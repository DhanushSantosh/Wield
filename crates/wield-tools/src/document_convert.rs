//! The `document.convert` built-in — pandoc, with a soffice fallback for PDF.

use std::time::Duration;
use wield_core::{
    ArgSpecBuilder, ArgType, ArgValueLiteral, Category, CommandSpecBuilder, Descriptor,
    DescriptorBuilder, FileFilter, OutputDir, OutputSpec, ProgressSpec, Requires,
};

const INPUT_EXTS: &[&str] = &["md", "html", "docx", "odt", "rst"];
const FORMATS: &[&str] = &["md", "html", "docx", "odt", "pdf"];

/// The command run for every conversion. `sh` receives this as its `-c`
/// script, with `sh {input} {output} {format}` giving it `$0`/`$1`/`$2`/`$3`.
///
/// `pandoc` takes an arbitrary output path directly (`-o {output}`), same
/// as `ffmpeg`/`magick` - fits the normal single-command model. `soffice
/// --convert-to` does not: it only accepts an output *directory* and
/// always names its result `<input-stem>.<ext>`, with no override. The
/// `pdf` branch works around this without any `wield-core` change: it
/// copies the real input into `{output}`'s own directory under a
/// filename whose stem matches `{output}`'s stem (keeping the input's
/// real extension, since `soffice` needs that to detect the source
/// format), so `soffice`'s forced naming produces exactly `{output}`'s
/// path. `trap ... EXIT` cleans up that intermediate copy whether the
/// command succeeds or fails.
///
/// IMPORTANT: this script must contain zero literal `{`/`}` characters -
/// `wield-core`'s template engine treats ANY `{...}` in a command argv
/// element as a Wield placeholder to resolve, including ones that are
/// just shell syntax. `${in##*.}`-style parameter expansion would break
/// descriptor validation; the `sed` line below extracts the extension
/// instead, verified live to behave identically.
///
/// Verified live end-to-end (`docs/superpowers/specs/2026-09-14-wield-m2c-document-convert-design.md`
/// §3): both branches, multiple source formats, exact-argv-level
/// invocation (no shell re-quoting layer), zero leftover files.
const CONVERT_SCRIPT: &str = r#"set -e
in="$1"; out="$2"; fmt="$3"
case "$fmt" in
  pdf)
    dir=$(dirname "$out")
    stem=$(basename "$out" ".$fmt")
    ext=$(printf "%s" "$in" | sed "s/^.*\.//")
    tmp="$dir/$stem.$ext"
    trap "rm -f \"$tmp\"" EXIT
    cp "$in" "$tmp"
    soffice --headless --convert-to pdf --outdir "$dir" "$tmp" 1>&2
    ;;
  *)
    pandoc --standalone "$in" -o "$out"
    ;;
esac
"#;

/// Descriptor for `document.convert`. `format`'s enum value doubles as
/// the output extension - `pandoc` auto-detects both reader and writer
/// from file extensions (verified live, no `-f`/`-t` flags needed), so
/// there's no separate codec-name indirection needed here (unlike
/// `audio.extract`'s `m4a`/`aac` split). No progress reporting - neither
/// `pandoc` nor `soffice --headless` exposes any incremental progress
/// protocol (verified live: both complete in well under 3 seconds on
/// realistic test documents, with no periodic stderr output to parse).
/// See `docs/superpowers/specs/2026-09-14-wield-m2c-document-convert-design.md`.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("document.convert", "Convert document", Category::Convert)
        .keywords(&[
            "document",
            "convert",
            "pandoc",
            "libreoffice",
            "pdf",
            "docx",
            "odt",
            "markdown",
        ])
        .arg(
            ArgSpecBuilder::new(
                "input",
                "Document",
                ArgType::File {
                    filters: vec![FileFilter {
                        label: "Documents".into(),
                        extensions: INPUT_EXTS.iter().map(|ext| (*ext).to_owned()).collect(),
                    }],
                    multiple: true,
                },
            )
            .required(true)
            .build(),
        )
        .arg(
            ArgSpecBuilder::new(
                "format",
                "Output format",
                ArgType::Enum {
                    options: FORMATS.iter().map(|fmt| (*fmt).to_owned()).collect(),
                },
            )
            .required(true)
            .default(ArgValueLiteral::Str("md".into()))
            .build(),
        )
        .requires(Requires::Binary("pandoc".into()))
        .output(OutputSpec::File {
            name: "{input_stem}.{format}".into(),
            dir: OutputDir::SameAsInput,
        })
        .command(
            CommandSpecBuilder::new("sh")
                .arg("-c")
                .arg(CONVERT_SCRIPT)
                .arg("sh")
                .arg("{input}")
                .arg("{output}")
                .arg("{format}")
                .progress(ProgressSpec::None)
                .timeout(Duration::from_secs(300)),
        )
        .build()
        .expect("document.convert descriptor is valid")
}
