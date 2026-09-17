//! `wield-native` — the `Native`-capability tool implementations. Implements
//! `wield_core::NativeRunner`, the same shape `wield_portal::PortalRunner`
//! uses for `Portal`-capability tools.

pub mod runner;
pub mod tools;

pub use runner::NativeToolRunner;
