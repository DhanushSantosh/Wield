//! Dispatches a descriptor's `NativeId` to its implementation. Mirrors
//! `wield-portal/src/adapters/mod.rs`'s `dispatch` function exactly.

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use wield_core::{ArgMap, NativeRunner, Stage, ToolOutcome};

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeToolRunner;

#[async_trait]
impl NativeRunner for NativeToolRunner {
    async fn run(&self, id: &str, args: &ArgMap, cancel: CancellationToken) -> ToolOutcome {
        match id {
            other => ToolOutcome::Failed {
                stage: Stage::Native,
                detail: format!("unknown native tool: {other}"),
                hint: Some("this is a bug in the tool descriptor".to_owned()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unknown_id_is_a_clear_failure() {
        let runner = NativeToolRunner;
        let args = std::collections::BTreeMap::new();
        let outcome = runner
            .run("not.a.real.tool", &args, CancellationToken::new())
            .await;
        assert!(matches!(
            outcome,
            ToolOutcome::Failed {
                stage: Stage::Native,
                ..
            }
        ));
    }
}
