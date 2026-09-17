//! `inhibit.toggle` — toggles the `org.freedesktop.portal.Inhibit` session
//! that prevents idle/suspend. State (the currently-held `Request<()>`, if
//! any) lives in a module-level static, not `AppState` — `keep.awake` runs
//! through the same generic `Executor`/`PortalRunner` pipeline every other
//! tool uses, which has no channel into `AppState` at all.

use crate::error::PortalError;
use ashpd::desktop::inhibit::{InhibitFlags, InhibitOptions, InhibitProxy};
use ashpd::desktop::Request;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use wield_core::{ArgMap, ToolOutcome};

static HELD: OnceLock<Mutex<Option<Request<()>>>> = OnceLock::new();

fn held() -> &'static Mutex<Option<Request<()>>> {
    HELD.get_or_init(|| Mutex::new(None))
}

/// Synchronous state query - the tray uses this to initialize and refresh
/// its checkmark without going through the async toggle path. `try_lock`
/// is deliberate: this only needs to be right when nothing is mid-toggle,
/// true the overwhelming majority of the time it's called - falling back
/// to "not active" on the rare contended case is an acceptable
/// display-only race, not a correctness issue worth a real lock wait for.
pub fn is_active() -> bool {
    held().try_lock().map(|guard| guard.is_some()).unwrap_or(false)
}

pub async fn toggle(_args: &ArgMap, cancel: CancellationToken) -> ToolOutcome {
    let mut guard = held().lock().await;
    toggle_with(&mut guard, start_inhibit(), cancel).await
}

/// Closes the held inhibit if one is active, without flipping anything on.
/// Used only by the tray's Quit handler - `toggle()` always flips state,
/// which would be wrong here if keep-awake happened to already be off.
pub async fn close_if_active() {
    if let Some(request) = held().lock().await.take() {
        let _ = request.close().await;
    }
}

/// The testable core: takes the held-state slot and the "start a new
/// inhibit" future as parameters so tests can substitute a fake for the
/// latter without touching the real portal or the shared static.
async fn toggle_with<F>(held: &mut Option<Request<()>>, start: F, cancel: CancellationToken) -> ToolOutcome
where
    F: std::future::Future<Output = Result<Request<()>, PortalError>>,
{
    if let Some(request) = held.take() {
        // Turning off. Best-effort: even if the close call itself fails,
        // clearing our own handle is still correct - we can't meaningfully
        // hold or retry a handle we've already decided to drop.
        let _ = request.close().await;
        return ToolOutcome::Report {
            title: "Keep awake".to_owned(),
            lines: vec![
                "Turned off - normal sleep and screensaver behavior is restored.".to_owned(),
            ],
        };
    }

    let result = tokio::select! {
        biased;
        _ = cancel.cancelled() => return ToolOutcome::Cancelled,
        result = start => result,
    };
    match result {
        Ok(request) => {
            *held = Some(request);
            ToolOutcome::Report {
                title: "Keep awake".to_owned(),
                lines: vec![
                    "Turned on - your screen won't sleep or lock until you toggle this off again."
                        .to_owned(),
                ],
            }
        }
        Err(error) => error.into_outcome(),
    }
}

async fn start_inhibit() -> Result<Request<()>, PortalError> {
    let proxy = InhibitProxy::new().await.map_err(PortalError::from)?;
    proxy
        .inhibit(
            None,
            InhibitFlags::Idle | InhibitFlags::Suspend,
            InhibitOptions::default().set_reason("Wield: keep awake"),
        )
        .await
        .map_err(PortalError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancel_wins_over_a_pending_start() {
        let mut held: Option<ashpd::desktop::Request<()>> = None;
        let token = CancellationToken::new();
        token.cancel();
        let outcome = toggle_with(
            &mut held,
            std::future::pending::<Result<ashpd::desktop::Request<()>, PortalError>>(),
            token,
        )
        .await;
        assert!(matches!(outcome, ToolOutcome::Cancelled));
        assert!(held.is_none());
    }

    #[tokio::test]
    async fn a_dismissed_start_is_cancelled() {
        let mut held: Option<ashpd::desktop::Request<()>> = None;
        let outcome = toggle_with(
            &mut held,
            async { Err(PortalError::Cancelled) },
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(outcome, ToolOutcome::Cancelled));
        assert!(held.is_none());
    }

    #[tokio::test]
    async fn a_transport_failure_is_failed_portal() {
        let mut held: Option<ashpd::desktop::Request<()>> = None;
        let outcome = toggle_with(
            &mut held,
            async { Err(PortalError::Transport("boom".into())) },
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(
            outcome,
            ToolOutcome::Failed {
                stage: wield_core::Stage::Portal,
                ..
            }
        ));
        assert!(held.is_none());
    }

    #[test]
    fn is_active_is_false_when_nothing_is_held() {
        // A fresh static, never toggled on in this test binary's run so
        // far, must report inactive - this is the only branch of
        // `is_active()` unit-testable without a real ashpd::Request (see
        // the plan's Global Constraints).
        assert!(!is_active());
    }

    #[tokio::test]
    async fn close_if_active_is_a_no_op_when_nothing_is_held() {
        // Exercises the "nothing held" branch directly against the real
        // static (safe: it's a no-op either way) - the "something held"
        // branch has the same untestable-without-a-real-Request
        // constraint as toggle_with's own off-path (see Global
        // Constraints).
        close_if_active().await;
        assert!(!is_active());
    }
}
