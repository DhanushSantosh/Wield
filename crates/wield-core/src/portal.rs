//! The portal-execution seam. `wield-portal` implements this; `wield-core`
//! never depends on a portal library.

use crate::args::ArgMap;
use crate::outcome::ToolOutcome;
use tokio_util::sync::CancellationToken;

/// Runs a `Capability::Portal { adapter }` tool.
#[async_trait::async_trait]
pub trait PortalRunner: Send + Sync {
    /// Run the named portal adapter with validated arguments.
    async fn run(
        &self,
        adapter: &str,
        args: &ArgMap,
        cancel: CancellationToken,
    ) -> ToolOutcome;
}
