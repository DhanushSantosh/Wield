//! `screen.ocr` — Screenshot portal capture, then `tesseract` recognition.
//!
//! Capture and recognition are each behind their own small async function so
//! tests can substitute a fake for either independently, mirroring
//! `wield-portal/src/adapters/pick_color.rs`'s `pick_color_with`.

use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wield_core::args::ArgMap;
use wield_core::command::{CommandResult, CommandRunner, RunSpec};
use wield_core::descriptor::{ProgressSpec, SuccessSpec};
use wield_core::outcome::{Stage, ToolOutcome};
use wield_core::ValueKind;
use wield_portal::PortalError;

const TESSERACT_TIMEOUT_SECS: u64 = 30;

/// Entry point wired into `NativeToolRunner`'s dispatch (Task 5).
pub async fn run(_args: &ArgMap, cancel: CancellationToken) -> ToolOutcome {
    run_with(capture_screenshot(), recognize_text, cancel).await
}

async fn run_with<C, R, RFut>(capture: C, recognize: R, cancel: CancellationToken) -> ToolOutcome
where
    C: std::future::Future<Output = Result<PathBuf, PortalError>>,
    R: FnOnce(PathBuf, CancellationToken) -> RFut,
    RFut: std::future::Future<Output = ToolOutcome>,
{
    let screenshot_path = tokio::select! {
        biased;
        _ = cancel.cancelled() => return ToolOutcome::Cancelled,
        result = capture => match result {
            Ok(path) => path,
            Err(error) => return error.into_outcome(),
        },
    };

    let outcome = recognize(screenshot_path.clone(), cancel).await;
    // Best-effort: the portal owns this file, not us. A leftover temp
    // screenshot the user never sees is not worth failing the run over.
    let _ = std::fs::remove_file(&screenshot_path);
    outcome
}

async fn capture_screenshot() -> Result<PathBuf, PortalError> {
    let request = ashpd::desktop::screenshot::Screenshot::request()
        .interactive(true)
        .modal(true)
        .send()
        .await
        .map_err(map_ashpd_error)?;
    let screenshot = request.response().map_err(map_ashpd_error)?;
    let url = url::Url::parse(screenshot.uri().as_str())
        .map_err(|error| PortalError::BadResponse(error.to_string()))?;
    url.to_file_path()
        .map_err(|_| PortalError::BadResponse(format!("unexpected screenshot URI: {url}")))
}

fn map_ashpd_error(error: ashpd::Error) -> PortalError {
    match error {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled) => PortalError::Cancelled,
        other => PortalError::Transport(other.to_string()),
    }
}

async fn recognize_text(image: PathBuf, cancel: CancellationToken) -> ToolOutcome {
    let scratch = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(error) => {
            return ToolOutcome::Failed {
                stage: Stage::Native,
                detail: format!("could not create a scratch directory: {error}"),
                hint: None,
            }
        }
    };
    // tesseract appends `.txt` to whatever outputbase it's given.
    let output_base = scratch.path().join("ocr");
    let argv = vec![
        image.display().to_string(),
        output_base.display().to_string(),
        "-l".to_string(),
        "eng".to_string(),
    ];
    let (progress_tx, _progress_rx) = mpsc::channel(1);
    let result = CommandRunner::execute(RunSpec {
        binary: Path::new("tesseract"),
        argv: &argv,
        cwd: None,
        output: None,
        progress_spec: &ProgressSpec::None,
        success: &SuccessSpec::ExitZero,
        timeout: Duration::from_secs(TESSERACT_TIMEOUT_SECS),
        progress: progress_tx,
        cancel,
    })
    .await;

    match result {
        CommandResult::Success { .. } => {
            let text_path = output_base.with_extension("txt");
            let text = std::fs::read_to_string(&text_path).unwrap_or_default();
            let _ = std::fs::remove_file(&text_path);
            text_outcome(&text)
        }
        CommandResult::SpawnFailed { detail } => ToolOutcome::Unavailable {
            reason: format!("tesseract could not be started: {detail}"),
            fix: Some("install tesseract or add it to your PATH".to_owned()),
        },
        CommandResult::NonZeroExit { code, stderr_tail } => ToolOutcome::Failed {
            stage: Stage::Native,
            detail: format!("tesseract exited with {code:?}: {stderr_tail}"),
            hint: None,
        },
        CommandResult::Timeout => ToolOutcome::Failed {
            stage: Stage::Native,
            detail: "tesseract timed out".to_owned(),
            hint: None,
        },
        CommandResult::Cancelled => ToolOutcome::Cancelled,
        CommandResult::OutputMissing => ToolOutcome::Failed {
            stage: Stage::Native,
            detail: "tesseract did not produce a text file".to_owned(),
            hint: None,
        },
    }
}

/// Maps raw (untrimmed) tesseract stdout-file content to the final outcome.
/// A blank result is a clear failure, not a silent empty-string success —
/// a "Copy" button that copies nothing is worse than an explicit message.
fn text_outcome(raw: &str) -> ToolOutcome {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        ToolOutcome::Failed {
            stage: Stage::Native,
            detail: "no text was found in the selected region".to_owned(),
            hint: Some("try a region with clearer text or more contrast".to_owned()),
        }
    } else {
        ToolOutcome::Value {
            kind: ValueKind::Text,
            data: trimmed.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wield_core::{Stage, ToolOutcome, ValueKind};
    use wield_portal::PortalError;

    #[tokio::test]
    async fn cancel_token_wins_over_a_pending_capture() {
        let token = CancellationToken::new();
        token.cancel();
        let outcome = run_with(
            std::future::pending::<Result<PathBuf, PortalError>>(),
            |_path, _cancel| async { unreachable!("recognize must not run") },
            token,
        )
        .await;
        assert!(matches!(outcome, ToolOutcome::Cancelled));
    }

    #[tokio::test]
    async fn dismissed_portal_dialog_is_cancelled() {
        let outcome = run_with(
            async { Err(PortalError::Cancelled) },
            |_path, _cancel| async { unreachable!("recognize must not run") },
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(outcome, ToolOutcome::Cancelled));
    }

    #[tokio::test]
    async fn portal_transport_error_is_failed_native() {
        let outcome = run_with(
            async { Err(PortalError::Transport("boom".into())) },
            |_path, _cancel| async { unreachable!("recognize must not run") },
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(
            outcome,
            ToolOutcome::Failed {
                stage: Stage::Portal,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn happy_path_trims_and_returns_the_recognized_text() {
        let outcome = run_with(
            async { Ok(PathBuf::from("/tmp/does-not-need-to-exist.png")) },
            |_path, _cancel| async {
                ToolOutcome::Value {
                    kind: ValueKind::Text,
                    data: "recognized text".into(),
                }
            },
            CancellationToken::new(),
        )
        .await;
        match outcome {
            ToolOutcome::Value {
                kind: ValueKind::Text,
                data,
            } => {
                assert_eq!(data, "recognized text");
            }
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[test]
    fn empty_stdout_is_a_clear_failure_not_a_silent_empty_value() {
        let outcome = text_outcome("   \n  ");
        assert!(matches!(
            outcome,
            ToolOutcome::Failed {
                stage: Stage::Native,
                ..
            }
        ));
    }

    #[test]
    fn non_empty_stdout_is_trimmed_into_a_value() {
        let outcome = text_outcome("  hello world  \n");
        match outcome {
            ToolOutcome::Value {
                kind: ValueKind::Text,
                data,
            } => assert_eq!(data, "hello world"),
            other => panic!("expected Value, got {other:?}"),
        }
    }
}
