# Wield — M2c: `document.convert` Design

## 1. Why this exists

M2c is the third M2 (Converter suite) tool, per the design spec's own
milestone table (`docs/superpowers/specs/2026-09-10-wield-design.md` §6):
"`document.convert` | Convert | Command | pandoc (+ libreoffice headless
fallback) | md / docx / html / rst / …".

Unlike M2b (`audio.extract`), this milestone needed genuinely new
verification: neither `pandoc` nor `libreoffice` was installed on this
machine, and neither had ever been exercised by anything in this codebase.
Both are now installed (`pandoc-cli` 3.10.2, `LibreOffice` 26.8.0) and
everything below is verified against the real binaries, not assumed from
documentation.

## 2. What's already there, and what isn't

Reused unmodified from M1/M2a/M2b: `Executor::run_batch` (batch is
automatic for any `multiple: true` File arg), the typed builders, the
`OutputPlan::for_final` temp-path-then-atomic-rename model
(`crates/wield-core/src/command.rs:70-88` — computes
`.wield-tmp-{uuid}-{final_name}` next to the final path, substitutes it
into `{output}`, renames it to the real final path only on success).

**Not reused: progress.** Both `pandoc` and `soffice --headless
--convert-to` are fast (sub-3-second on realistic test documents,
including `soffice`'s cold start on a machine where it had never run
before) and neither exposes any incremental progress protocol — no
periodic stderr output like ffmpeg's `-progress pipe:2`. `ProgressSpec::None`
is correct here, matching `image.convert`'s original pattern — this is a
verified absence, not an assumption.

## 3. The real architectural problem: `soffice` can't take an arbitrary output path

`pandoc <input> -o <output>` writes to exactly the path given — fits
Wield's existing model perfectly, the same way `ffmpeg`/`magick` do.

`soffice --headless --convert-to <ext> --outdir <dir> <input>` does not
have an equivalent. Confirmed via `soffice --help` (no output-filename
flag exists for `--convert-to`) and empirically: it always names its
result `<input-stem>.<ext>` inside `--outdir`, with no way to override
that. This is incompatible with `OutputPlan`'s model, which requires the
command to write to one specific, pre-computed path.

**The fix — verified working end-to-end, no new `wield-core` mechanism
needed:** run the whole command through `sh -c '<script>'` instead of
invoking `pandoc`/`soffice` directly. The script takes `{input}`,
`{output}`, and `{format}` as positional arguments (all three are already
valid template placeholders — `{format}` substitutes any arg's value
generically, the same mechanism `video.convert`'s `{quality}` already
relies on) and branches:

```sh
set -e
in="$1"; out="$2"; fmt="$3"
case "$fmt" in
  pdf)
    dir=$(dirname "$out")
    stem=$(basename "$out" ".$fmt")
    ext="${in##*.}"
    tmp="$dir/$stem.$ext"
    trap "rm -f \"$tmp\"" EXIT
    cp "$in" "$tmp"
    soffice --headless --convert-to pdf --outdir "$dir" "$tmp" 1>&2
    ;;
  *)
    pandoc --standalone "$in" -o "$out"
    ;;
esac
```

For the `pdf` branch: copy the real input into the *same directory* as
`{output}` (which is `OutputPlan`'s own temp directory — invisible to the
user, already Wield-owned scratch space), under a filename whose **stem
matches `{output}`'s own stem** but keeps the input's real extension (so
`soffice`'s format auto-detection still works correctly). `soffice` then
writes `<dir>/<stem>.pdf` — which **is** `{output}`, exactly, because we
chose the intermediate copy's stem to match on purpose. `trap ... EXIT`
cleans up the intermediate copy on both success and failure (verified: a
plain end-of-script `rm -f` would be skipped by `set -e` on a `soffice`
failure, leaving the intermediate file behind — `trap` fixes this).

Verified live, both branches, multiple source formats: `md → docx` via
pandoc, `md → pdf` and `odt → pdf` via the `soffice` wrapper, and a
`docx → md` round-trip (pandoc reading its own `docx` output back). Every
case produced a valid file at the exact expected temp path with nothing
else left behind.

**Why one script instead of two descriptors or a new `OutputSpec`
variant:** a `Descriptor`'s `Capability::Command` has exactly one
`binary` field — it can't conditionally be `pandoc` for some formats and
`sh` for others within one `CommandSpec`. Splitting into two separate
tools (`document.convert` + something like `document.export_pdf`) would
work too, but surfaces as two palette entries for what a user experiences
as one action ("convert my document") — worse UX for an implementation
detail they shouldn't need to know about. A new `OutputSpec` variant
("discover whatever file appeared in a directory") was considered and
rejected: it's less precise (fragile if a directory ever contains more
than the expected one file) and is real new `wield-core` surface for a
need the `sh -c` wrapper already meets with zero core changes.

**`--standalone` is required for `pandoc`'s direct path.** Verified: `md →
html` without `--standalone` produces a bare content fragment (no
`<html>`/`<head>`), not something a user expects when they ask to
"convert to HTML". With `--standalone`, every format gets a complete,
openable document. Harmless no-op for formats that are inherently
self-contained anyway (`docx`, `odt`).

## 4. `document.convert` descriptor

- **`input`** — `File { multiple: true, filters: [md, html, docx, odt,
  rst] }` (pandoc-readable formats only — see §5's scope cut).
- **`format`** — `Enum { options: [md, html, docx, odt, pdf] }`, default
  `md`. Pandoc auto-detects both reader and writer from file extensions
  (verified: no `-f`/`-t` flags needed anywhere in testing), so the enum
  value doubles directly as the output extension with no separate
  codec-name indirection needed (unlike `audio.extract`'s `m4a`/`aac`
  split) — `pandoc`'s writer name and the file extension are the same
  string for every format on this list.
- **`Requires::Binary("pandoc".into())`** — gates the tool's overall
  availability. **Known, documented gap**: `Requires` supports exactly
  one binary name, no composite "needs A and B" check exists in
  `wield-core` today. `pandoc` covers 4 of 5 formats; gating on it alone
  means a machine with `pandoc` but not `soffice` shows the tool as
  available, and a `pdf` conversion specifically would fail at runtime
  (the `sh -c` script's `soffice` invocation fails) rather than being
  caught ahead of time as `Unavailable`. Accepted for v1 rather than
  building a composite-requirement mechanism for one tool; worth
  reconsidering if a future tool needs the same shape.
- **Output**: `OutputSpec::File { name: "{input_stem}.{format}", dir:
  OutputDir::SameAsInput }` — same established naming convention as
  every other M1/M2 tool.
- **Command** (`Capability::Command`): `binary: "sh"`, args
  `["-c", <the script above>, "sh", "{input}", "{output}", "{format}"]`.
  `ProgressSpec::None`. Timeout: 300s (matches `audio.extract`'s — both
  engines are fast; generous margin for a large or complex document).

## 5. Risks and honest gaps

- **Legacy binary input formats (`.doc`/`.ppt`/`.xls`) are out of scope
  for v1.** `pandoc` cannot read them at all (confirmed — not in
  `pandoc --list-input-formats`'s output). Supporting them would need the
  engine-selection branch to also consider the *input*'s format, not just
  the requested output format — real added complexity (two-dimensional
  branching instead of one), deliberately deferred rather than built now.
  Worth its own follow-up plan if real demand shows up; track in
  `docs/backlog.md`.
- **The single-`Requires::Binary` gap** (§4) — `pandoc`-only availability
  gating means a `pdf` conversion can fail at runtime on a machine
  missing `soffice`, without the tool having shown `Unavailable` ahead of
  time. Documented, not fixed — see §4.
- **No Flatpak bundling.** Same pattern as every other Command-backed
  converter tool. `libreoffice-fresh` is dramatically larger than any
  binary named in `docs/backlog.md`'s existing "Bundled converter
  binaries" entry so far (`ImageMagick`, `pandoc`, `qpdf`, `Ghostscript`,
  `Tesseract`, `ffmpeg-full`) — worth naming explicitly there once this
  lands, not silently lumped in.
- **The `sh -c` wrapper is a new pattern for this codebase** — every
  prior Command-backed tool invokes its real binary directly. Worth a
  comment at the point of use (the plan's descriptor code) explaining why,
  so a future reader doesn't mistake it for an accidental complexity.
- **300s timeout is a judgment call**, not derived from a worst-case
  (e.g. a very large or image-heavy document). Both engines measured
  well under 3 seconds on realistic test documents; generous margin.

## 6. Testing strategy

`render_argv` unit tests confirming the exact rendered `sh -c` script and
positional arguments for both branches (a non-`pdf` format and `pdf`
specifically) — since the whole command is one large templated string,
the test asserts the complete rendered script text matches exactly what
was verified live in §3, not just that *some* script was produced. A
builtin-registry snapshot update. Live verification on the real desktop:
at least one pandoc-path conversion (e.g. `md → docx`), the `pdf` path
specifically (since it's the one exercising the wrapper's temp/cleanup
logic for real), and a multi-file batch — all `file`/checksum-independent
confirmed, not just trusting the result card. Written up in
`docs/testing.md`, same format as M2a/M2b's sections.

## 7. Repo artifacts

- This spec: `docs/superpowers/specs/2026-09-14-wield-m2c-document-convert-design.md`
- Plan (next): `docs/superpowers/plans/2026-09-14-wield-m2c-document-convert.md`
- New: `crates/wield-tools/src/document_convert.rs`
- Modified: `crates/wield-tools/src/lib.rs`,
  `crates/wield-tools/tests/builtin_registry.rs`,
  `crates/wield-tools/tests/snapshots/builtin_registry.json`, and the same
  mechanical registration-count ripple every prior M2 tool has hit
  (`apps/wield/src-tauri/src/commands.rs`'s registration-order test,
  `apps/wield/src-tauri/tests/commands.rs`'s two tool-count assertions —
  `4` becomes `5`).
- `docs/backlog.md`: add `libreoffice-fresh` explicitly to the "Bundled
  converter binaries" entry; add the deferred legacy-input-formats gap.
- Branch: `feat/m2c-document-convert`, forked from `master` (both M2a and
  M2b are merged — nothing currently unmerged to stack on top of, so this
  is a normal fork, not a stacked one; the stacked-PR convention applies
  whenever there *is* something open to stack on).
