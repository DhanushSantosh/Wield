//! The `pdf.compress` built-in — shrink a PDF's file size via Ghostscript.

use std::time::Duration;
use wield_core::{
    ArgSpecBuilder, ArgType, ArgValueLiteral, Category, CommandSpecBuilder, Descriptor,
    DescriptorBuilder, FileFilter, OutputDir, OutputSpec, ProgressSpec, Requires,
};

const QUALITY_PRESETS: &[&str] = &["screen", "ebook", "printer"];

/// Descriptor for `pdf.compress`. Runs `gs -sDEVICE=pdfwrite
/// -dCompatibilityLevel=1.4 -dPDFSETTINGS=/<preset> -dNOPAUSE -dBATCH
/// -sOutputFile=<output> <input>`. `quality` is one of Ghostscript's
/// three standard presets (`screen` smallest/lowest quality, `ebook`
/// medium, `printer` largest/highest quality) - verified live that all
/// three work; a real Enum of presets rather than a raw numeric range,
/// same reasoning as `audio.extract`'s bitrate choice. The output name
/// appends `-compressed` rather than reusing `{input_stem}.{format}` -
/// unlike every format-conversion tool so far, compress doesn't change
/// the extension, so there's nothing to naturally distinguish the
/// output from the source; overwriting the source PDF in place would be
/// a real, surprising data-loss risk for a "shrink my file" tool.
/// `ghostscript` is the package name; the binary itself is `gs`.
/// See `docs/superpowers/specs/2026-09-14-wield-m2d-pdf-tools-design.md`.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("pdf.compress", "Compress PDF", Category::Convert)
        .keywords(&["pdf", "compress", "shrink", "ghostscript", "size"])
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
        .arg(
            ArgSpecBuilder::new(
                "quality",
                "Quality",
                ArgType::Enum {
                    options: QUALITY_PRESETS.iter().map(|q| (*q).to_owned()).collect(),
                },
            )
            .required(true)
            .default(ArgValueLiteral::Str("ebook".into()))
            .help("screen: smallest, lowest quality. ebook: balanced. printer: largest, best quality.")
            .build(),
        )
        .requires(Requires::Binary("gs".into()))
        .output(OutputSpec::File {
            name: "{input_stem}-compressed.pdf".into(),
            dir: OutputDir::SameAsInput,
        })
        .command(
            CommandSpecBuilder::new("gs")
                .arg("-sDEVICE=pdfwrite")
                .arg("-dCompatibilityLevel=1.4")
                .arg("-dPDFSETTINGS=/{quality}")
                .arg("-dNOPAUSE")
                .arg("-dBATCH")
                .arg("-sOutputFile={output}")
                .arg("{input}")
                .progress(ProgressSpec::None)
                .timeout(Duration::from_secs(300)),
        )
        .build()
        .expect("pdf.compress descriptor is valid")
}
