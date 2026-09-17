//! `Native` capability runner — dispatches a descriptor's `NativeId` to its
//! Rust implementation. Mirrors `portal.rs`'s `PortalRunner` exactly: same
//! injection shape into `Executor`, same string-keyed dispatch pattern.

use crate::args::ArgMap;
use crate::outcome::ToolOutcome;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

#[async_trait]
pub trait NativeRunner: Send + Sync {
    async fn run(&self, id: &str, args: &ArgMap, cancel: CancellationToken) -> ToolOutcome;
}
