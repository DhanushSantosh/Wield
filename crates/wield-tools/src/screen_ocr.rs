//! The `screen.ocr` built-in — capture a screen region via the Screenshot
//! portal, then recognize text in it via `tesseract`. `Native` capability:
//! the first built-in that isn't `Command` or `Portal`.

use wield_core::{Category, Descriptor, DescriptorBuilder, OutputSpec, Requires, ValueKind};

/// Descriptor for `screen.ocr`. Needs both the Screenshot portal (capture)
/// and the `tesseract` binary (recognition) — `Requires::All` lets the
/// palette grey this out proactively if either is missing, not just one.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("screen.ocr", "Extract text from screen", Category::Capture)
        .keywords(&["ocr", "text", "extract", "screenshot", "recognize"])
        .requires(Requires::All(vec![
            Requires::Portal {
                iface: "Screenshot".into(),
                min_ver: 2,
            },
            Requires::Binary("tesseract".into()),
        ]))
        .output(OutputSpec::Value(ValueKind::Text))
        .native("screen.ocr")
        .build()
        .expect("screen.ocr descriptor is valid")
}
