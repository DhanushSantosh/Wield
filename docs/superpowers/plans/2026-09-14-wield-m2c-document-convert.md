# M2c: `document.convert` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `document.convert` — convert between document formats via
`pandoc`, with PDF output routed through a `soffice --headless` wrapper —
reusing every existing Wield mechanism, with zero `wield-core` changes.

**Architecture:** One new tool descriptor
(`crates/wield-tools/src/document_convert.rs`). The `Capability::Command`
runs `sh -c '<script>' sh {input} {output} {format}` instead of invoking
`pandoc`/`soffice` directly — the script branches on the `format`
positional argument: `pandoc --standalone {input} -o {output}` for every
format except `pdf`; for `pdf`, it copies the input into `{output}`'s own
temp directory under a matching stem (so `soffice`'s forced
input-stem-derived naming produces exactly `{output}`'s path), runs
`soffice --headless --convert-to pdf`, and cleans up the intermediate
copy via `trap ... EXIT`. `{input}`/`{output}`/`{format}` are all
already-supported template placeholders — no new ones needed. Batch
(`multiple: true`) is automatic, same as every M2 tool since `audio.extract`.

**Tech Stack:** Rust (`wield-core`, `wield-tools`), `pandoc-cli` 3.10.2,
LibreOffice 26.8.0 (already installed on the target dev machine for this
plan's live-verification task).

**Spec:** `docs/superpowers/specs/2026-09-14-wield-m2c-document-convert-design.md`

## Global Constraints

- No new Rust or npm dependencies.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace -- --test-threads=1`, and `npm run check` (run
  from the repo root) must all be clean before every commit.
- Typed builders only (`DescriptorBuilder`/`ArgSpecBuilder`/`CommandSpecBuilder`).
- The shell script embedded in the descriptor must contain **zero literal
  `{` or `}` characters** — `wield-core`'s template engine
  (`crates/wield-core/src/template.rs::placeholders`) scans the *entire*
  text of every command argv element for `{name}` patterns, including
  ones that are just shell syntax (e.g. `${var}`), and will fail
  descriptor validation on anything it can't resolve. Use `$(...)`
  command substitution and external tools (`sed`, `basename`, `dirname`)
  instead of `${...}` parameter expansion anywhere in the script. This
  plan's script has already been verified against this exact constraint
  — do not "simplify" it back to `${in##*.}`-style syntax.
- This branch (`feat/m2c-document-convert`) is forked from `master`
  (already created; both M2a and M2b are merged, nothing to stack on
  right now). The spec commit (`660ec29`) is already on it.

---

### Task 1: `document.convert` descriptor, registry, tests, and backlog update

**Files:**
- Create: `crates/wield-tools/src/document_convert.rs`
- Modify: `crates/wield-tools/src/lib.rs`
- Modify: `crates/wield-tools/tests/builtin_registry.rs`
- Modify: `crates/wield-tools/tests/snapshots/builtin_registry.json` (regenerated, not hand-edited)
- Modify: `apps/wield/src-tauri/src/commands.rs` (mechanical ripple — registration-order test)
- Modify: `apps/wield/src-tauri/tests/commands.rs` (mechanical ripple — two tool-count assertions)
- Modify: `docs/backlog.md`

**Interfaces:**
- Consumes: nothing new from `wield-core` — every mechanism
  (`{input}`/`{output}`/`{format}` template resolution, `arg_when`-free
  plain `.arg(...)` segments, batch via `multiple: true`) already exists
  and is unmodified.
- Produces: `document_convert::descriptor() -> Descriptor`, registered in
  `builtin_registry()` as `"document.convert"`. Nothing downstream in
  this plan consumes it beyond Task 2's live verification.

- [ ] **Step 1: Write the descriptor**

Create `crates/wield-tools/src/document_convert.rs`:

```rust
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
            "document", "convert", "pandoc", "libreoffice", "pdf", "docx", "odt", "markdown",
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
```

- [ ] **Step 2: Register in the registry**

In `crates/wield-tools/src/lib.rs`, the module declarations currently read:

```rust
pub mod audio_extract;
pub mod color_pick;
pub mod image_convert;
pub mod video_convert;
```

Replace with (alphabetical, `document_convert` sorts between `color_pick`
and `image_convert`):

```rust
pub mod audio_extract;
pub mod color_pick;
pub mod document_convert;
pub mod image_convert;
pub mod video_convert;
```

And `builtin_registry()` currently reads:

```rust
pub fn builtin_registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register(audio_extract::descriptor())
        .expect("audio.extract registers");
    registry
        .register(color_pick::descriptor())
        .expect("color.pick registers");
    registry
        .register(image_convert::descriptor())
        .expect("image.convert registers");
    registry
        .register(video_convert::descriptor())
        .expect("video.convert registers");
    registry
}
```

Replace with (register `document.convert` between `color.pick` and
`image.convert`, matching the module order — `Registry::list()` returns
registration order, not a sorted order):

```rust
pub fn builtin_registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register(audio_extract::descriptor())
        .expect("audio.extract registers");
    registry
        .register(color_pick::descriptor())
        .expect("color.pick registers");
    registry
        .register(document_convert::descriptor())
        .expect("document.convert registers");
    registry
        .register(image_convert::descriptor())
        .expect("image.convert registers");
    registry
        .register(video_convert::descriptor())
        .expect("video.convert registers");
    registry
}
```

- [ ] **Step 3: Write the render_argv tests**

In `crates/wield-tools/tests/builtin_registry.rs`, add two new tests
(place them near the existing `audio_extract_*` tests, which this
mirrors — the file already imports `Capability`, `ArgValue`, `BTreeMap`,
`render_argv`, `builtin_registry`, no new imports needed). The expected
script text below is the **exact** `CONVERT_SCRIPT` constant from Step 1
— copy it verbatim, character for character, including the raw-string
delimiters:

```rust
const EXPECTED_SCRIPT: &str = r#"set -e
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

#[test]
fn document_convert_renders_the_pandoc_direct_branch() {
    let tool = builtin_registry().get("document.convert").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };

    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/docs/a.md".into()));
    args.insert("format".to_string(), ArgValue::Str("docx".into()));
    let out = std::path::PathBuf::from("/docs/a.docx");
    assert_eq!(
        render_argv(&spec.args, &args, Some(&out)).unwrap(),
        vec![
            "-c".to_string(),
            EXPECTED_SCRIPT.to_string(),
            "sh".into(),
            "/docs/a.md".into(),
            "/docs/a.docx".into(),
            "docx".into(),
        ],
    );
}

#[test]
fn document_convert_renders_the_pdf_wrapper_branch() {
    let tool = builtin_registry().get("document.convert").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };

    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Path("/docs/a.md".into()));
    args.insert("format".to_string(), ArgValue::Str("pdf".into()));
    let out = std::path::PathBuf::from("/docs/a.pdf");
    assert_eq!(
        render_argv(&spec.args, &args, Some(&out)).unwrap(),
        vec![
            "-c".to_string(),
            EXPECTED_SCRIPT.to_string(),
            "sh".into(),
            "/docs/a.md".into(),
            "/docs/a.pdf".into(),
            "pdf".into(),
        ],
    );
}
```

Also update `registry_has_exactly_the_expected_builtins` (already in this
file):

```rust
#[test]
fn registry_has_exactly_the_expected_builtins() {
    let ids: Vec<_> = builtin_registry()
        .list()
        .iter()
        .map(|d| d.id.as_ref().to_string())
        .collect();
    assert_eq!(
        ids,
        vec!["audio.extract", "color.pick", "image.convert", "video.convert"]
    );
}
```

to:

```rust
#[test]
fn registry_has_exactly_the_expected_builtins() {
    let ids: Vec<_> = builtin_registry()
        .list()
        .iter()
        .map(|d| d.id.as_ref().to_string())
        .collect();
    assert_eq!(
        ids,
        vec![
            "audio.extract",
            "color.pick",
            "document.convert",
            "image.convert",
            "video.convert"
        ]
    );
}
```

- [ ] **Step 4: Run tests to verify the new ones fail correctly**

Run: `cargo test -p wield-tools`
Expected: `document_convert_renders_the_pandoc_direct_branch`,
`document_convert_renders_the_pdf_wrapper_branch`, and
`registry_has_exactly_the_expected_builtins` all fail (module/id don't
exist yet) if Step 1 wasn't written first, or `builtin_registry_matches_snapshot`
fails on a stale snapshot if it was — either is fine, the point is
confirming nothing passes by accident before the snapshot is regenerated.

- [ ] **Step 5: Regenerate the snapshot**

```bash
UPDATE_SNAPSHOTS=1 cargo test -p wield-tools
```

Then run `cargo test -p wield-tools` again (without the env var) to
confirm `builtin_registry_matches_snapshot` now passes.

- [ ] **Step 6: Review the snapshot diff**

```bash
git diff crates/wield-tools/tests/snapshots/builtin_registry.json
```

Expected: a pure insertion of one new `document.convert` object — confirm
`audio.extract`, `color.pick`, `image.convert`, and `video.convert`'s
existing entries are byte-for-byte unchanged (only lines added, none
removed from any of them).

- [ ] **Step 7: Fix the mechanical ripple in the app crate**

In `apps/wield/src-tauri/src/commands.rs`, find the test
`list_tools_with_no_query_returns_registration_order` and its expected
vector:

```rust
vec![
    "audio.extract",
    "color.pick",
    "image.convert",
    "video.convert"
]
```

Replace with:

```rust
vec![
    "audio.extract",
    "color.pick",
    "document.convert",
    "image.convert",
    "video.convert"
]
```

In `apps/wield/src-tauri/tests/commands.rs`, find both `assert_eq!(...len(), 4)`
lines (in `capabilities_lists_every_builtin_with_availability` and
`list_tools_returns_both_builtins_with_arg_schemas`) and change both `4`s
to `5`.

- [ ] **Step 8: Update `docs/backlog.md`**

Find the "Bundled converter binaries" bullet under `## Packaging` (it
currently lists ImageMagick, pandoc, qpdf, Ghostscript, Tesseract+`eng`,
and the `ffmpeg-full` runtime extension). Add `libreoffice-fresh`
explicitly to that same bullet's list of unbundled binaries — it's not
just "one more item", it's dramatically larger than everything already
named there, worth being able to spot at a glance rather than folded in
silently.

Then add a new bullet directly after it, still under `## Packaging` (or
under a new `## Converter tooling (M2)` section if one already exists
from an earlier milestone — check the file first):

```markdown
- **`document.convert` can't read legacy binary Office formats
  (`.doc`/`.ppt`/`.xls`).** `pandoc` has no reader for them at all
  (confirmed via `pandoc --list-input-formats` - only the modern XML-based
  `docx`/`pptx`/`xlsx` are supported). Reading them would need the
  engine-selection logic to also branch on the *input*'s format, not just
  the requested output format - real added complexity, deliberately
  deferred rather than built into M2c's first version. `libreoffice`
  itself can read these formats fine; only the routing logic is missing.
```

- [ ] **Step 9: Full workspace gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
npm run check
```

All four must be clean.

- [ ] **Step 10: Commit**

```bash
git add crates/wield-tools/src/document_convert.rs \
  crates/wield-tools/src/lib.rs \
  crates/wield-tools/tests/builtin_registry.rs \
  crates/wield-tools/tests/snapshots/builtin_registry.json \
  apps/wield/src-tauri/src/commands.rs \
  apps/wield/src-tauri/tests/commands.rs \
  docs/backlog.md
git commit -m "feat(tools): add document.convert (pandoc, soffice wrapper for PDF)

No wield-core changes - the sh -c wrapper script achieves everything
within the existing Command capability model. pandoc takes an arbitrary
output path directly, same as ffmpeg/magick; soffice --convert-to
cannot (outdir + input-stem-derived naming only), so the pdf branch
copies the input into the output's own temp directory under a matching
stem first, letting soffice's forced naming land exactly at {output}.
Verified live for both branches at the exact argv level (no shell
re-quoting layer) before writing this down - see the design spec.

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 2: Live verification and `docs/testing.md` write-up

**No subagent dispatch for this task** — matches M2a's Task 5 and M2b's
Task 3 precedent: this class of work needs live desktop tooling (a real
file picker, real `pandoc`/`soffice` runs, independent confirmation of
each output) that only the orchestrator session driving this plan has
access to.

**Files:**
- Modify: `docs/testing.md`

**Interfaces:**
- Consumes: the real, installed `document.convert` tool via the real
  desktop UI — everything built in Task 1.
- Produces: nothing further in this plan consumes this task's output;
  it's the plan's final deliverable.

- [ ] **Step 1: Build and install a fresh binary**

```bash
npm run build -w apps/wield -- --no-bundle
```

- [ ] **Step 2: A pandoc-direct conversion**

Search "document" in the palette, select `Convert document`, pick a real
test document (a `.md` file is simplest to prepare) through the native
file picker, convert to `docx` (or another non-`pdf` format). Confirm the
result card shows a real success outcome. Verify independently — not just
trusting the UI — that the output is a genuinely different, valid file of
the target format (e.g. `file <output>` reports the right type, and its
content is readable/openable, not just present).

- [ ] **Step 3: The PDF/`soffice` wrapper path specifically**

This is the one exercising real new logic (the temp-directory-matching
trick and the cleanup trap), so it needs its own explicit confirmation,
not an inference from Step 2. Convert the same or a different document to
`pdf`. Confirm the result is a valid PDF (`file <output>` reports `PDF
document`), and confirm no stray intermediate file (the copied
`<stem>.<original-ext>` used to drive `soffice`'s naming) was left behind
in the output directory.

- [ ] **Step 4: Multi-file batch**

Select 2-3 files at once through the native multi-select picker, run the
conversion. Confirm the result card shows a real `ToolOutcome::Report`
(`"N of N converted"`) and independently verify each output.

- [ ] **Step 5: Write up `docs/testing.md`**

Add a new section following the exact pattern of the existing M2a/M2b
sections (read at least one of them first for the tone/structure to
match): what was verified and how, and any honest gaps if something
couldn't be reproduced live (state the reason — this project's
established standard, not a new one for this task).

- [ ] **Step 6: Full gate one more time**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
npm run check
```

- [ ] **Step 7: Commit**

```bash
git add docs/testing.md
git commit -m "docs(testing): verify document.convert live — pandoc path, pdf wrapper, batch

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```
