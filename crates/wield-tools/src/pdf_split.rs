//! The `pdf.split` built-in — one PDF's pages become one file each, via qpdf.

use std::time::Duration;
use wield_core::{
    ArgSpecBuilder, ArgType, Category, CommandSpecBuilder, Descriptor, DescriptorBuilder,
    FileFilter, OutputDir, OutputSpec, ProgressSpec, Requires,
};

/// Descriptor for `pdf.split`. Runs `qpdf --split-pages <input>
/// <output_dir>/page-%d.pdf` - the number of files this produces isn't
/// known until it actually runs (it depends on the input's page count),
/// so this uses `OutputSpec::Directory` rather than `OutputSpec::File`:
/// `{output_dir}` resolves to a fresh, empty scratch directory instead
/// of one exact path, and `Executor::run_split` discovers whatever
/// files appeared afterward, renaming each to `{input_stem}-N.pdf` next
/// to the source. `qpdf`'s own `%d` is plain `printf`-style numbering,
/// not a Wield placeholder - it has no braces, so the template engine's
/// brace-scanning never touches it. The `{output_dir}/page-%d.pdf`
/// segment is NOT a bare placeholder (there's a suffix beyond just
/// `{output_dir}`), so it resolves through the normal single-value path
/// `resolve()` already handles for `"output_dir"`, not the spread path.
/// See `docs/superpowers/specs/2026-09-14-wield-m2d-pdf-tools-design.md`.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("pdf.split", "Split PDF", Category::Convert)
        .keywords(&["pdf", "split", "pages", "extract", "qpdf"])
        .arg(
            ArgSpecBuilder::new(
                "input",
                "PDF",
                ArgType::File {
                    filters: vec![FileFilter {
                        label: "PDF".into(),
                        extensions: vec!["pdf".into()],
                    }],
                    multiple: false,
                },
            )
            .required(true)
            .build(),
        )
        .requires(Requires::Binary("qpdf".into()))
        .output(OutputSpec::Directory {
            name: "{input_stem}".into(),
            dir: OutputDir::SameAsInput,
        })
        .command(
            CommandSpecBuilder::new("qpdf")
                .arg("--split-pages")
                .arg("{input}")
                .arg("{output_dir}/page-%d.pdf")
                .progress(ProgressSpec::None)
                .timeout(Duration::from_secs(300)),
        )
        .build()
        .expect("pdf.split descriptor is valid")
}
