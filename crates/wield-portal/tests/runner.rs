use std::collections::BTreeMap;
use tokio_util::sync::CancellationToken;
use wield_core::{PortalRunner, Stage, ToolOutcome};
use wield_portal::PortalAdapterRunner;

#[tokio::test]
async fn unknown_adapter_is_failed_portal() {
    let output = PortalAdapterRunner
        .run(
            "nope.nope",
            &BTreeMap::new(),
            CancellationToken::new(),
        )
        .await;
    assert!(matches!(
        output,
        ToolOutcome::Failed {
            stage: Stage::Portal,
            ..
        }
    ));
}

#[tokio::test]
async fn known_adapter_with_immediate_cancel_is_cancelled() {
    let token = CancellationToken::new();
    token.cancel();
    let output = PortalAdapterRunner
        .run("screenshot.pick_color", &BTreeMap::new(), token)
        .await;
    assert!(matches!(output, ToolOutcome::Cancelled));
}
