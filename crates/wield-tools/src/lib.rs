//! `wield-tools` — the built-in Wield tool descriptors.
//!
//! Each built-in is a [`wield_core::Descriptor`] constructed with the typed
//! builders and validated on build. [`builtin_registry`] is the single source
//! every surface (palette, tray, CLI) reads from.

pub mod color_pick;

use wield_core::Registry;

/// The registry of every built-in tool, validated. Panics only if a built-in
/// descriptor is malformed — a programming error the snapshot test catches.
pub fn builtin_registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register(color_pick::descriptor())
        .expect("color.pick registers");
    registry
}
