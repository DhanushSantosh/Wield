//! The `pdf.merge` built-in — combine several PDFs into one via qpdf.

use std::time::Duration;
use wield_core::{
    ArgSpecBuilder, ArgType, Category, CommandSpecBuilder, Descriptor, DescriptorBuilder,
    FileFilter, OutputDir, OutputSpec, ProgressSpec, Requires,
};

/// Descriptor for `pdf.merge`. Runs `qpdf --empty --pages <file1>
/// <file2> ... -- <output>` - `{input}` is a bare placeholder over a
/// `multiple: true` arg, so `combine_inputs(true)` makes it spread into
/// one argv element per selected file (see
/// `crates/wield-core/src/template.rs::render_argv`'s spread branch),
/// in one `qpdf` invocation, rather than `run_batch`'s usual per-file
/// loop. The `--` before `{output}` is required - verified live that
/// omitting it makes `qpdf` try to parse the output filename as a page
/// range and fail outright. There's no established Wield convention for
/// letting a user name a combined output, so the result is named after
/// the *first* selected file (`{input_stem}` now falls back to the
/// first path in a `Paths` value, from Task 1) with a `-merged` suffix.
/// See `docs/superpowers/specs/2026-09-14-wield-m2d-pdf-tools-design.md`.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("pdf.merge", "Merge PDFs", Category::Convert)
        .keywords(&["pdf", "merge", "combine", "join", "qpdf"])
        .arg(
            ArgSpecBuilder::new(
                "input",
                "PDFs",
                ArgType::File {
                    filters: vec![FileFilter {
                        label: "PDF".into(),
                        extensions: vec!["pdf".into()],
                    }],
                    multiple: true,
                },
            )
            .required(true)
            .build(),
        )
        .requires(Requires::Binary("qpdf".into()))
        .output(OutputSpec::File {
            name: "{input_stem}-merged.pdf".into(),
            dir: OutputDir::SameAsInput,
        })
        .command(
            CommandSpecBuilder::new("qpdf")
                .arg("--empty")
                .arg("--pages")
                .arg("{input}")
                .arg("--")
                .arg("{output}")
                .combine_inputs(true)
                .progress(ProgressSpec::None)
                .timeout(Duration::from_secs(300)),
        )
        .build()
        .expect("pdf.merge descriptor is valid")
}
