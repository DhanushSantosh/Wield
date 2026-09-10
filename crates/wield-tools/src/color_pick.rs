//! The `color.pick` built-in — pick a screen colour via the Screenshot portal.

use wield_core::{Category, Descriptor, DescriptorBuilder, OutputSpec, Requires, ValueKind};

/// Descriptor for `color.pick`. Portal capability, no arguments; the result is a
/// colour value the UI can copy as hex / rgb / hsl.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("color.pick", "Pick a colour", Category::Capture)
        .keywords(&["colour", "color", "eyedropper", "pixel", "hex", "rgb"])
        .requires(Requires::Portal {
            iface: "Screenshot".into(),
            min_ver: 2,
        })
        .output(OutputSpec::Value(ValueKind::Color))
        .portal("screenshot.pick_color")
        .build()
        .expect("color.pick descriptor is valid")
}
