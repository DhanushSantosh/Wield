//! `wield-portal` — the startup capability probe and the XDG Desktop Portal
//! adapters Wield uses. Implements `wield_core::PortalRunner`.

pub mod adapters;
pub mod color;
pub mod error;
pub mod global_shortcuts;
pub mod probe;
pub mod runner;

pub use error::PortalError;
pub use probe::{probe, PortalMap};
pub use runner::PortalAdapterRunner;
