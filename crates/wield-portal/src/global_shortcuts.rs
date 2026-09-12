//! The `GlobalShortcuts` portal: bind a single "show palette" shortcut and
//! forward activations to a callback.
//!
//! `ashpd` 0.13 does not expose a session-restore token for this portal (only
//! `InputCapture` / `RemoteDesktop` / `ScreenCast` have one in this version),
//! so every bind re-shows the desktop environment's shortcut-confirmation
//! dialog. That is a real, documented limitation of the current dependency,
//! not a bug in this module — see `docs/testing.md`.

use std::sync::Arc;

use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use ashpd::desktop::Session;
use futures_util::StreamExt;

/// The one shortcut Wield registers.
pub const SHORTCUT_ID: &str = "show-palette";
/// Preferred trigger hint; the desktop environment's own binding UI is
/// authoritative and may offer the user something else.
pub const PREFERRED_TRIGGER: &str = "<Super>w";
/// Shown to the user when the portal is unavailable, to bind in their DE's
/// own keyboard-shortcut settings. There is no dedicated "show palette" CLI
/// verb (`wield-cli` only runs one-shot tools by id) — relaunching the shell
/// binary is the mechanism that actually works today: the single-instance
/// guard (P5a) detects the already-running instance and relays `ShowPalette`
/// to it instead of starting a second shell. Revisit if a packaged install
/// exposes a friendlier launcher name (Flatpak/AUR naming lands in P7).
pub const FALLBACK_COMMAND: &str = "wield-app";

/// The result of trying to bind the shortcut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindOutcome {
    /// The shortcut is bound; activations invoke the callback for the rest of
    /// the process's life.
    Bound,
    /// The portal is absent, too old, or binding failed for another reason.
    Unavailable { fallback_command: String },
}

/// The live portal session behind a successful [`BindOutcome::Bound`].
/// Cloning is cheap (an `Arc` handle) — the same session is shared between
/// the activation-listening task and whoever holds this to request
/// reconfiguration later (e.g. a "Change shortcut" button in Preferences).
#[derive(Clone)]
pub struct HotkeyController {
    inner: Arc<BoundSession>,
}

struct BoundSession {
    portal: GlobalShortcuts,
    session: Session<GlobalShortcuts>,
}

impl HotkeyController {
    /// Asks the portal to show the desktop environment's own shortcut
    /// configuration UI for this session — the standard, DE-native way for
    /// a user to rebind (or unbind) the shortcut. Requires portal interface
    /// version 2+; older backends return an error, which the caller should
    /// treat the same as "not available here" rather than a fatal error.
    pub async fn configure(&self) -> ashpd::Result<()> {
        self.inner
            .portal
            .configure_shortcuts(&self.inner.session, None, Default::default())
            .await
    }
}

/// Create a `GlobalShortcuts` session, bind [`SHORTCUT_ID`], and spawn a task
/// that calls `on_activated` for every matching `Activated` signal. Returns
/// once the bind attempt resolves; the forwarding task (on success) runs for
/// the process lifetime. Never blocks startup — any portal failure is logged
/// and reported as [`BindOutcome::Unavailable`]. On success, also returns a
/// [`HotkeyController`] for later reconfiguration; `None` when unavailable
/// (nothing to reconfigure).
pub async fn bind_show_palette(
    on_activated: impl Fn() + Send + Sync + 'static,
) -> (BindOutcome, Option<HotkeyController>) {
    match try_bind(on_activated).await {
        Ok(controller) => (BindOutcome::Bound, Some(controller)),
        Err(error) => {
            tracing::warn!(%error, "GlobalShortcuts portal unavailable");
            (
                BindOutcome::Unavailable {
                    fallback_command: FALLBACK_COMMAND.to_owned(),
                },
                None,
            )
        }
    }
}

async fn try_bind(
    on_activated: impl Fn() + Send + Sync + 'static,
) -> ashpd::Result<HotkeyController> {
    let portal = GlobalShortcuts::new().await?;
    let session = portal.create_session(Default::default()).await?;

    let shortcut = NewShortcut::new(SHORTCUT_ID, "Show the Wield palette")
        .preferred_trigger(PREFERRED_TRIGGER);
    portal
        .bind_shortcuts(&session, &[shortcut], None, Default::default())
        .await?
        .response()?;

    let bound = Arc::new(BoundSession { portal, session });
    let mut activations = bound.portal.receive_activated().await?;

    // Clone the Arc (cheap) into the listening task so the portal
    // connection and session stay alive for as long as we're listening,
    // while the original stays with the returned controller.
    let listener = bound.clone();
    tokio::spawn(async move {
        let _keep_alive = listener;
        while let Some(activated) = activations.next().await {
            if activated.shortcut_id() == SHORTCUT_ID {
                on_activated();
            }
        }
        tracing::warn!("GlobalShortcuts activation stream ended");
    });

    Ok(HotkeyController { inner: bound })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_command_relaunches_the_shell_binary() {
        assert_eq!(FALLBACK_COMMAND, "wield-app");
    }

    #[tokio::test]
    #[ignore = "requires a compositor with the GlobalShortcuts portal; run manually"]
    async fn binds_against_a_real_portal() {
        let (outcome, controller) = bind_show_palette(|| {}).await;
        assert!(matches!(
            outcome,
            BindOutcome::Bound | BindOutcome::Unavailable { .. }
        ));
        assert_eq!(controller.is_some(), outcome == BindOutcome::Bound);
    }

    #[tokio::test]
    #[ignore = "requires a compositor with the GlobalShortcuts portal v2+; run manually"]
    async fn configure_opens_the_desktop_environments_rebind_ui() {
        let (outcome, controller) = bind_show_palette(|| {}).await;
        let BindOutcome::Bound = outcome else {
            panic!("expected the portal to be available in this manual test run");
        };
        controller
            .expect("Bound implies a controller")
            .configure()
            .await
            .expect("configure_shortcuts should be accepted by a v2+ backend");
    }
}
