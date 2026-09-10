//! Screenshot portal colour picker.

use crate::color::Rgb;
use crate::error::PortalError;
use tokio_util::sync::CancellationToken;
use wield_core::{ArgMap, ToolOutcome, ValueKind};

/// Pick a colour through the Screenshot desktop portal.
pub async fn pick_color(_args: &ArgMap, cancel: CancellationToken) -> ToolOutcome {
    pick_color_with(fetch_color_via_ashpd(), cancel).await
}

async fn pick_color_with<F>(fetch: F, cancel: CancellationToken) -> ToolOutcome
where
    F: std::future::Future<Output = Result<Rgb, PortalError>>,
{
    tokio::select! {
        biased;
        _ = cancel.cancelled() => ToolOutcome::Cancelled,
        result = fetch => match result {
            Ok(rgb) => ToolOutcome::Value {
                kind: ValueKind::Color,
                data: rgb.value_payload(),
            },
            Err(error) => error.into_outcome(),
        },
    }
}

async fn fetch_color_via_ashpd() -> Result<Rgb, PortalError> {
    let request = ashpd::desktop::Color::pick()
        .send()
        .await
        .map_err(map_ashpd_error)?;
    let color = request.response().map_err(map_ashpd_error)?;
    Ok(Rgb::from_unit(color.red(), color.green(), color.blue()))
}

fn map_ashpd_error(error: ashpd::Error) -> PortalError {
    match error {
        ashpd::Error::Response(ashpd::desktop::ResponseError::Cancelled) => PortalError::Cancelled,
        other => PortalError::Transport(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgb;
    use crate::error::PortalError;
    use tokio_util::sync::CancellationToken;
    use wield_core::{ToolOutcome, ValueKind};

    #[tokio::test]
    async fn maps_a_picked_colour_to_a_value_outcome() {
        let output = pick_color_with(
            async { Ok(Rgb { r: 10, g: 20, b: 30 }) },
            CancellationToken::new(),
        )
        .await;
        match output {
            ToolOutcome::Value {
                kind: ValueKind::Color,
                data,
            } => assert!(data.contains("#0a141e")),
            other => panic!("expected Value, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn dismissed_dialog_is_silent_cancelled() {
        let output = pick_color_with(
            async { Err(PortalError::Cancelled) },
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(output, ToolOutcome::Cancelled));
    }

    #[tokio::test]
    async fn transport_error_is_failed_portal() {
        let output = pick_color_with(
            async { Err(PortalError::Transport("boom".into())) },
            CancellationToken::new(),
        )
        .await;
        assert!(matches!(
            output,
            ToolOutcome::Failed {
                stage: wield_core::Stage::Portal,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn cancel_token_wins_over_a_pending_pick() {
        let token = CancellationToken::new();
        token.cancel();
        let output = pick_color_with(
            std::future::pending::<Result<Rgb, PortalError>>(),
            token,
        )
        .await;
        assert!(matches!(output, ToolOutcome::Cancelled));
    }
}
