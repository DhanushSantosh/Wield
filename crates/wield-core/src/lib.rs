//! `wield-core` — the Wield tool descriptor model, argument validation,
//! argv-template rendering, the `Command`-capability executor, and the tool
//! registry.
//!
//! Portal and Native execution are stubbed here; they land in P3 and P4.

pub mod args;
pub mod builder;
pub mod command;
pub mod descriptor;
pub mod error;
pub mod executor;
pub mod outcome;
pub mod registry;
pub mod template;
pub mod validate;

/// The `wield-core` crate version, from Cargo.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_non_empty() {
        assert!(!super::version().is_empty());
    }
}
