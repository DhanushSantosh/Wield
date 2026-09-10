//! `wield-core` — descriptor model, executor, and tool registry for Wield.
//!
//! Stub in plan P1. The descriptor/executor model lands in P2.

/// The `wield-core` crate version, from Cargo.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty() {
        assert!(!version().is_empty());
    }
}
