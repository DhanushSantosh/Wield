//! Portal adapter runner.

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use wield_core::{ArgMap, PortalRunner, ToolOutcome};

/// Dispatches named desktop portal adapters.
#[derive(Debug, Default, Clone, Copy)]
pub struct PortalAdapterRunner;

#[async_trait]
impl PortalRunner for PortalAdapterRunner {
    async fn run(&self, adapter: &str, args: &ArgMap, cancel: CancellationToken) -> ToolOutcome {
        crate::adapters::dispatch(adapter, args, cancel).await
    }
}
