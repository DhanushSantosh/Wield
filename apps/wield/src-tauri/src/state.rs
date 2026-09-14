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

/// Whether the `GlobalShortcuts` portal binding succeeded, and if not, what
/// to tell the user to bind by hand in their desktop environment.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state")]
pub enum HotkeyState {
    /// The bind attempt hasn't completed yet (briefly, at startup).
    Pending,
    /// The shortcut is bound; activating it shows the palette.
    Registered,
    /// The portal is absent or binding failed. `fallback_command` is a
    /// working, copyable command the user can bind themselves.
    Unavailable { fallback_command: String },
}

/// Registry, executor, capability data, and cancellation state held by Tauri.
pub struct AppState {
    pub registry: Registry,
    pub executor: Executor,
    pub availability: AvailabilityView,
    runs: Mutex<HashMap<RunId, CancellationToken>>,
    hotkey: Mutex<HotkeyState>,
    /// Set once the startup `GlobalShortcuts` bind resolves to `Bound`; `None`
    /// otherwise (nothing to reconfigure, or the bind hasn't finished yet).
    hotkey_controller: Mutex<Option<wield_portal::global_shortcuts::HotkeyController>>,
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
            hotkey: Mutex::new(HotkeyState::Pending),
            hotkey_controller: Mutex::new(None),
        }
    }

    pub fn hotkey_state(&self) -> HotkeyState {
        self.hotkey.lock().expect("hotkey lock").clone()
    }

    pub fn set_hotkey_state(&self, state: HotkeyState) {
        *self.hotkey.lock().expect("hotkey lock") = state;
    }

    pub fn set_hotkey_controller(
        &self,
        controller: Option<wield_portal::global_shortcuts::HotkeyController>,
    ) {
        *self
            .hotkey_controller
            .lock()
            .expect("hotkey controller lock") = controller;
    }

    /// Opens the desktop environment's shortcut-configuration UI so the user
    /// can rebind the palette shortcut. Errors when there is no active
    /// session to reconfigure (the bind never succeeded, or hasn't resolved
    /// yet) or when the portal backend rejects the request (e.g. older than
    /// the required v2 interface).
    pub async fn configure_hotkey(&self) -> Result<(), String> {
        let controller = self
            .hotkey_controller
            .lock()
            .expect("hotkey controller lock")
            .clone();
        match controller {
            Some(controller) => controller
                .configure()
                .await
                .map_err(|error| error.to_string()),
            None => Err("no active hotkey session to reconfigure".to_owned()),
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

    #[cfg(test)]
    pub(crate) async fn for_test(registry: Registry, resolver: BinaryResolver) -> Self {
        let availability = AvailabilityView::probe_binaries(&resolver, registry.list());
        let executor = Executor::new(resolver)
            .with_portal(Arc::new(wield_portal::PortalAdapterRunner))
            .with_availability(availability.clone());
        Self {
            registry,
            executor,
            availability,
            runs: Mutex::new(HashMap::new()),
            hotkey: Mutex::new(HotkeyState::Pending),
            hotkey_controller: Mutex::new(None),
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test_sync() -> Self {
        let mut state = tauri::async_runtime::block_on(Self::for_test(
            wield_tools::builtin_registry(),
            wield_core::BinaryResolver::with_dirs(vec![]),
        ));
        state.availability.binaries.clear();
        state
    }

    #[cfg(test)]
    pub(crate) fn in_flight_ids(&self) -> Vec<RunId> {
        self.runs
            .lock()
            .expect("runs lock")
            .keys()
            .cloned()
            .collect()
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

    fn empty_state() -> AppState {
        AppState {
            registry: wield_core::Registry::new(),
            executor: wield_core::Executor::new(wield_core::BinaryResolver::from_env()),
            availability: Default::default(),
            runs: std::sync::Mutex::new(std::collections::HashMap::new()),
            hotkey: std::sync::Mutex::new(HotkeyState::Pending),
            hotkey_controller: std::sync::Mutex::new(None),
        }
    }

    #[test]
    fn cancel_run_toggles_a_registered_token() {
        let state = empty_state();
        let id = RunId::new();
        let token = CancellationToken::new();
        state.register_run(id.clone(), token.clone());
        assert!(state.cancel_run(&id));
        assert!(token.is_cancelled());
        assert!(state.take_run(&id).is_some());
        assert!(!state.cancel_run(&RunId::new()));
    }

    #[tokio::test]
    async fn configure_hotkey_errors_with_no_active_session() {
        let state = empty_state();
        let error = state.configure_hotkey().await.unwrap_err();
        assert_eq!(error, "no active hotkey session to reconfigure");
    }

    #[test]
    fn hotkey_state_defaults_pending_and_is_settable() {
        let state = empty_state();
        assert_eq!(state.hotkey_state(), HotkeyState::Pending);
        state.set_hotkey_state(HotkeyState::Registered);
        assert_eq!(state.hotkey_state(), HotkeyState::Registered);
        state.set_hotkey_state(HotkeyState::Unavailable {
            fallback_command: "wield-app".into(),
        });
        assert_eq!(
            state.hotkey_state(),
            HotkeyState::Unavailable {
                fallback_command: "wield-app".into()
            }
        );
    }
}
