//! The `keep.awake` built-in — toggle preventing idle/suspend via the
//! `Inhibit` desktop portal. `Portal` capability, same shape as
//! `color.pick`, but a stateful toggle rather than a one-shot result.

use wield_core::{Category, Descriptor, DescriptorBuilder, OutputSpec, Requires};

/// Descriptor for `keep.awake`. No arguments - selecting it in the palette
/// runs it immediately, exactly like `color.pick` already does.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("keep.awake", "Keep awake", Category::Capture)
        .keywords(&["awake", "sleep", "screensaver", "caffeine", "inhibit"])
        .requires(Requires::Portal {
            iface: "Inhibit".into(),
            min_ver: 1,
        })
        .output(OutputSpec::Report)
        .portal("inhibit.toggle")
        .build()
        .expect("keep.awake descriptor is valid")
}
