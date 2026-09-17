//! Named portal adapters.

pub mod inhibit_toggle;
pub mod pick_color;

use tokio_util::sync::CancellationToken;
use wield_core::{ArgMap, Stage, ToolOutcome};

/// Route a descriptor's adapter key to its implementation.
pub(crate) async fn dispatch(
    adapter: &str,
    args: &ArgMap,
    cancel: CancellationToken,
) -> ToolOutcome {
    match adapter {
        "inhibit.toggle" => inhibit_toggle::toggle(args, cancel).await,
        "screenshot.pick_color" => pick_color::pick_color(args, cancel).await,
        other => ToolOutcome::Failed {
            stage: Stage::Portal,
            detail: format!("unknown portal adapter: {other}"),
            hint: Some("this is a bug in the tool descriptor".to_owned()),
        },
    }
}
