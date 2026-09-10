//! `wield-tools` — built-in tool descriptors and native tool implementations.
//!
//! Stub in plan P1. The `color.pick` and `image.convert` descriptors land in P4.

/// Number of built-in descriptors. Zero until P4.
pub fn descriptor_count() -> usize {
    0
}

#[cfg(test)]
mod tests {
    #[test]
    fn no_descriptors_yet() {
        assert_eq!(super::descriptor_count(), 0);
    }
}
