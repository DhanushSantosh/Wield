mod support;

use std::time::Duration;
use tokio_util::sync::CancellationToken;
use wield_core::{ToolOutcome, ValueKind};

#[tokio::test]
async fn pick_color_through_a_fake_portal_yields_a_value() {
    let Some(bus) = support::PrivateBus::launch() else {
        return;
    };
    let server = bus.connect().await;
    support::serve_fake_pick_color(
        &server,
        (0.0, 0.5019608, 1.0),
        Duration::from_millis(20),
    )
    .await;

    std::env::set_var("DBUS_SESSION_BUS_ADDRESS", bus.address());
    let output = wield_portal::adapters::pick_color::pick_color(
        &Default::default(),
        CancellationToken::new(),
    )
    .await;

    match output {
        ToolOutcome::Value {
            kind: ValueKind::Color,
            data,
        } => assert!(data.contains("#0080ff")),
        other => panic!("expected Value, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "requires a real desktop portal; run manually"]
async fn pick_color_against_the_real_portal() {
    let output = wield_portal::adapters::pick_color::pick_color(
        &Default::default(),
        CancellationToken::new(),
    )
    .await;
    assert!(matches!(
        output,
        ToolOutcome::Value { .. } | ToolOutcome::Cancelled
    ));
}
