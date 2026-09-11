//! Long-lived shell state.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;
use wield_core::{AvailabilityView, BinaryResolver, Executor, Registry};

/// Stable identifier for one in-flight execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct RunId(pub String);

impl RunId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for RunId {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry, executor, capability data, and cancellation state held by Tauri.
pub struct AppState {
    pub registry: Registry,
    pub executor: Executor,
    pub availability: AvailabilityView,
    runs: Mutex<HashMap<RunId, CancellationToken>>,
}

impl AppState {
    /// Build all shell state. Portal probing degrades to an empty map on failure.
    pub async fn build() -> Self {
        let registry = wield_tools::builtin_registry();
        let resolver = BinaryResolver::from_env();
        let mut availability = AvailabilityView::probe_binaries(&resolver, registry.list());
        wield_portal::probe().await.apply_to(&mut availability);
        let executor = Executor::new(resolver)
            .with_portal(Arc::new(wield_portal::PortalAdapterRunner))
            .with_availability(availability.clone());
        Self {
            registry,
            executor,
            availability,
            runs: Mutex::new(HashMap::new()),
        }
    }

    pub fn register_run(&self, id: RunId, token: CancellationToken) {
        self.runs.lock().expect("runs lock").insert(id, token);
    }

    pub fn take_run(&self, id: &RunId) -> Option<CancellationToken> {
        self.runs.lock().expect("runs lock").remove(id)
    }

    pub fn cancel_run(&self, id: &RunId) -> bool {
        match self.runs.lock().expect("runs lock").get(id) {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn build_produces_the_builtin_registry() {
        let state = AppState::build().await;
        assert!(state.registry.get("image.convert").is_some());
        assert!(state.registry.get("color.pick").is_some());
    }

    #[test]
    fn cancel_run_toggles_a_registered_token() {
        let state = AppState {
            registry: wield_core::Registry::new(),
            executor: wield_core::Executor::new(wield_core::BinaryResolver::from_env()),
            availability: Default::default(),
            runs: std::sync::Mutex::new(std::collections::HashMap::new()),
        };
        let id = RunId::new();
        let token = CancellationToken::new();
        state.register_run(id.clone(), token.clone());
        assert!(state.cancel_run(&id));
        assert!(token.is_cancelled());
        assert!(state.take_run(&id).is_some());
        assert!(!state.cancel_run(&RunId::new()));
    }
}
