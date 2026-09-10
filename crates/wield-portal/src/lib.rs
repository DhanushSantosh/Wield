//! `wield-portal` — XDG Desktop Portal access and startup capability probe.
//!
//! Stub in plan P1. The `ashpd` wrapper and capability probe land in P3.

/// Placeholder for the startup capability probe. Always `true` until P3.
pub fn probe_stub() -> bool {
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn probe_stub_is_true() {
        assert!(super::probe_stub());
    }
}
