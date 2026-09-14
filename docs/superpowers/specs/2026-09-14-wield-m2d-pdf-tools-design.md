# Wield — M2d: `pdf.tools` Design

## 1. Why this exists, and a real naming decision

M2d is the fourth and last M2 (Converter suite) tool, per the design
spec's milestone table (`docs/superpowers/specs/2026-09-10-wield-design.md`
§6): "`pdf.tools` | Convert | Command | qpdf / ghostscript | compress /
merge / split".

**This ships as three separate tool IDs — `pdf.compress`, `pdf.merge`,
`pdf.split` — not one `pdf.tools` descriptor with an operation selector.**
Traced why directly: `document.convert` (M2c) could branch between two
engines inside one descriptor because both branches shared the same arg
shape (one input, one output) and the same `Capability`/`OutputSpec`
combination — only the *command* varied. Compress/merge/split don't share
that: compress takes one input and produces one output (`OutputSpec::File`,
same as every other M2 tool); merge takes *multiple* inputs and produces
*one* output (needs a new dispatch mode, §4); split takes *one* input and
produces an *unknown-until-runtime number* of outputs (needs a new
`OutputSpec` variant, §6). A `Descriptor` has exactly one `Capability` and
one `OutputSpec` — there's no way to make one descriptor branch across
these three genuinely different shapes without a far messier "operation
selector that switches the whole capability" design. Three descriptors
under a shared `pdf.` prefix matches this project's own existing
convention (`video.convert`/`audio.extract`/`document.convert` are each
their own ID under the "M2" milestone *label*, which was never itself a
tool ID) more than it deviates from it.

The owner explicitly chose to build all three now, including the new
`wield-core` mechanisms merge and split need, rather than shipping
compress alone and deferring the rest (2026-09-14, asked directly given
the real difference in scope).

## 2. What's already there, reused unmodified

`CommandRunner::execute`'s entire progress/timeout/cancellation/stderr
machinery (`crates/wield-core/src/command.rs`) — untouched by every
change below. `ProgressSpec::None` for all three (verified live: neither
`qpdf` nor `ghostscript` exposes incremental progress on realistic
inputs, same finding as M2c). The typed builders. `ToolOutcome::Report`
(from M2a's batch work) — reused for split's result, not a new outcome
type.

## 3. `pdf.compress` — fits the existing model exactly, zero new mechanism

One input, one output — identical shape to `image.convert`/
`video.convert`. Backed by `ghostscript`, not `qpdf`: verified live that
`gs -sDEVICE=pdfwrite -dPDFSETTINGS=/ebook` (real downsampling/
recompression, the actual "shrink my PDF" behavior most people mean) is
the meaningfully different operation between the two libraries here;
`qpdf`'s own `--optimize-images --compress-streams=y` recompresses
streams losslessly but showed no real size win on the (small, text-only)
test documents available, and using both would just be two code paths
for no real product difference. `quality` exposed as an `Enum` of the
three standard Ghostscript PDF presets (`screen`/`ebook`/`printer` —
smallest/medium/best-quality-largest), not a free-form value, avoiding
the CRF-style footgun class M2a/M2b already learned from.

## 4. New mechanism: "combine" mode — for `pdf.merge` (N inputs → 1 output)

Traced directly, not assumed: `Executor::run_command`
(`crates/wield-core/src/executor.rs`) unconditionally routes *any*
`ArgValue::Paths` value through `run_batch`'s per-file loop
(`if let Some((arg_name, paths)) = batch_paths(effective) { return
self.run_batch(...) }` — this check has no way to be told "no, treat this
as one combined operation"). Separately, `render_argv`'s per-segment
resolution (`crates/wield-core/src/template.rs`) silently drops any
segment whose value resolves to `ArgValue::Paths` — `resolve()` returns
`Ok(None)` for it unconditionally, and an unresolved segment is just
skipped, not erred. Neither fits "pass the whole file list into one
`qpdf` invocation" — confirmed by testing `qpdf --empty --pages
file1.pdf file2.pdf file3.pdf -- output.pdf` live: each input file is its
own separate argv element, in the order given.

**The fix, two small additions:**

1. `CommandSpec` gets a new field, `combine_inputs: bool` (default
   `false`, a new `CommandSpecBuilder::combine_inputs()` setter). When
   `true`, `run_command`'s existing batch-dispatch check is skipped
   entirely — the descriptor's own `Capability::Command` flow runs
   directly, the same path `pdf.compress` uses. This is the only change
   to `run_command`'s control flow; every other descriptor (all of
   which leave this `false`) is unaffected.
2. `render_argv` gets one new branch, checked *before* its existing
   per-token resolution: if a segment's `template` is *exactly* one bare
   placeholder (e.g. `"{input}"`, nothing else in the string) and that
   arg's raw value is `ArgValue::Paths(paths)`, push each path as its own
   argv element instead of attempting single-value substitution. This
   only ever fires when a raw `Paths` value reaches `render_argv` at
   all — which, by construction, only happens when `combine_inputs` is
   `true` (otherwise `run_batch` already intercepted it upstream) — so
   it's inert for every existing descriptor, confirmed by tracing the
   only caller path.

**Output naming needs one more small change.** `merge` has no single
"the input" — `resolve()`'s existing `input`/`input_stem`/`input_dir`
special-casing requires `ArgValue::Path` and returns `Ok(None)` (silently
unresolved) for anything else, including `Paths`. Extended to fall back
to the *first* selected file when the value is `ArgValue::Paths` and
non-empty — the merged output lands next to the first file selected,
named after it (`"{input_stem}-merged.pdf"`). A predictable, documented
convention, not a surprising one.

No `validate.rs` changes needed: `check_template`'s existing rule for
`{input}`-family placeholders already accepts `ArgType::File { .. }`
regardless of `multiple` (M2a's ruling-accepted relaxation) — merge's
`input` arg is `multiple: true`, already covered.

## 5. `pdf.merge` descriptor

`input`: `File { multiple: true, filters: [pdf] }`, required. No
`format` arg — merge output is always a PDF. Command:
`qpdf --empty --pages {input} -- {output}` — `{input}` here is the
combine-mode spread placeholder from §4 (expands to N argv elements, one
per selected file, in selection order); `--` before `{output}` is
`qpdf`'s own required separator between the `--pages` file list and the
rest of its arguments — verified live both ways: with `--`, exit 0, a
correct 3-page merge; without it, `qpdf` tries to parse the output
filename as a page-range specifier and fails outright (`qpdf: error at *
in numeric range *merged.pdf: invalid range syntax`, exit 2).
`.combine_inputs(true)`. Output: `OutputSpec::File { name:
"{input_stem}-merged.pdf", dir: OutputDir::SameAsInput }` (§4's
first-file fallback makes `{input_stem}` resolve here).

## 6. New mechanism: `OutputSpec::Directory` — for `pdf.split` (1 input → N outputs)

`qpdf --split-pages <input> <dir>/<pattern>-%d.pdf` produces a number of
files that isn't known until the command actually runs (it depends on
the input's page count) — verified live with a 3-page merged test PDF,
producing exactly 3 files, sequentially numbered from 1. This breaks
`OutputPlan`'s entire model: it computes one pre-known temp path, and
`finish_output` (`crates/wield-core/src/command.rs`) explicitly checks
`plan.temp.is_file()` — a single, specific path, not "however many files
appeared in a directory".

**The fix reuses far more of the existing machinery than it first looks
like it needs to.** `CommandRunner::execute`'s `RunSpec.output` field is
already `Option<&OutputPlan>` — and `finish_output` already has a branch
for `None`: `let Some(plan) = output else { return
CommandResult::Success { output_file: None } }`. That's *already* the
exact behavior split needs at the `command.rs` level: run the command
with its full progress/timeout/cancellation handling completely
unchanged, skip the single-file temp-then-rename dance entirely. **Zero
changes to `command.rs`.** Every change lives in `executor.rs`, the same
place M2a's `run_batch` lives:

1. New `OutputSpec::Directory { name: String, dir: OutputDir }` variant
   (`crates/wield-core/src/descriptor.rs`) — `name` is a template
   (resolved via the existing `render_output_name`, so `{input_stem}`
   etc. work identically to `OutputSpec::File`) giving the *prefix* each
   discovered file's final name is built from — not a full filename,
   since the actual per-file numbering is an executor-level concern, not
   a template concept (see step 4). `validate.rs::check_capability_and_output`
   gets one new match arm: `(Capability::Command(command),
   OutputSpec::Directory { name, .. })` runs the same
   `check_command_templates`-style checks `OutputSpec::File` already
   gets, just without the "must not contain `{output}`" rule that
   applies to a filename (a directory-output descriptor's command
   references `{output_dir}` instead — see step 2).
2. A new template placeholder, `{output_dir}`, resolved in
   `template.rs`'s `resolve()` next to the existing `output` case — it
   needs a *directory* path, not `OutputPlan`'s single-file temp path, so
   `Executor::run_command` computes it differently for this branch (a
   fresh, real, empty temp directory created via `std::fs::create_dir`
   next to the final destination directory, named
   `.wield-tmp-{uuid}-split/` — same dotfile-prefix convention
   `OutputPlan::for_final` already uses for its temp files, just a
   directory instead of a file. No new dependency: plain `Uuid::new_v4()`
   + `std::fs`, exactly what `OutputPlan` already uses).
3. `Executor::run_command` gets one new branch at the top (parallel to
   the existing `batch_paths` check): if `descriptor.output` is
   `OutputSpec::Directory`, dispatch to a new `run_split`-shaped method
   instead of the normal single-file flow.
4. That method: creates the temp directory, renders argv with
   `{output_dir}` substituted, calls `CommandRunner::execute` with
   `output: None` (per point 1's realization), and on
   `CommandResult::Success { output_file: None }`, lists the temp
   directory's contents, sorts them (alphabetically — see §8's honest
   gap on sort order at page counts ≥ 10), and for each one: resolves
   the descriptor's `OutputSpec::Directory.name` template once to get the
   prefix, moves the file to `{dir}/{prefix}-{n}.pdf` (`n` is a plain
   1-indexed loop counter, not a template placeholder — this numbering
   is executor logic, not something `template.rs`'s general arg
   resolution needs to know about). Aggregates into the *existing*
   `ToolOutcome::Report { title, lines }` — the exact type M2a's batch
   summary already uses, rendered by the exact same, unmodified
   `ReportResult.tsx`. Any non-`Success` `CommandResult` (failure,
   timeout, cancellation) cleans up the temp directory and maps to the
   matching `ToolOutcome`, mirroring `run_batch`'s existing pattern for
   the equivalent cases.

## 7. `pdf.split` descriptor

`input`: `File { multiple: false, filters: [pdf] }`, required — split
takes exactly one PDF (batch-of-splits, i.e. splitting several PDFs at
once, is out of scope for v1; see §8). No `format` arg. Command: `qpdf
--split-pages {input} {output_dir}/page-%d.pdf`. Output:
`OutputSpec::Directory { name: "{input_stem}", dir: OutputDir::SameAsInput }`
— each discovered page lands as `{input_stem}-{n}.pdf` next to the
source file.

## 8. Risks and honest gaps

- **Split's discovered-file sort is alphabetical, not numeric.** At 10+
  pages, `qpdf`'s own `%d`-numbered output (`page-1.pdf`, `page-10.pdf`,
  `page-2.pdf`, ...) sorts lexicographically wrong before the executor
  ever renumbers them — the *content* of each split file is still
  correct (each is exactly one real page), but which page ends up
  labeled `-2` vs `-10` in the final filename could be off for
  larger documents. Not fixed for v1: a numeric-aware sort is a small,
  legitimate follow-up, not core-mechanism work; tracked in
  `docs/backlog.md`.
- **No batch support for any of the three.** `pdf.compress` could
  reasonably support `multiple: true` (independent per-file compression,
  exactly `run_batch`'s existing shape) — deliberately deferred to keep
  this milestone's real scope (two new core mechanisms) from also
  absorbing a third, unrelated addition. `pdf.merge`'s and `pdf.split`'s
  own cardinalities don't compose with batch at all (merge already
  consumes a `multiple: true` arg for its *own* purpose; split producing
  N outputs from M inputs would need yet another mechanism).
- **`pdf.merge`'s output naming convention** (`{first-file-stem}-merged.pdf`)
  is a real, deliberate choice, not an oversight — there's no
  established Wield pattern for "let the user name the output" (every
  tool so far derives the name from input), and inventing one is
  explicitly out of scope for this milestone.
- **No Flatpak bundling**, same as every Command-backed converter tool.
  `qpdf` and `ghostscript` both already named in `docs/backlog.md`'s
  existing "Bundled converter binaries" entry from the original M1
  packaging research — no new entry needed.
- **`Requires::Binary`** — each descriptor gates on its own real binary
  (`ghostscript` for compress, `qpdf` for merge/split) — no
  composite-requirement gap this time, since each tool genuinely needs
  exactly one binary, unlike M2c's `document.convert`.

## 9. Testing strategy

For the two new mechanisms specifically (not just the three descriptors):
unit tests for `render_argv`'s new spread branch (a `Paths` value under
`combine_inputs`, confirming N argv elements in order, and confirming a
segment that ISN'T a bare `{input}` placeholder — e.g. `{input_stem}` —
still resolves via the existing single-value path against the
first-file fallback); a `validate.rs` test confirming `OutputSpec::Directory`
validates correctly; `Executor`-level integration tests for `run_split`
covering the success case (N files discovered, correctly renamed and
reported), the zero-files case (should this be a failure? — decide in
the plan, not left implicit), and cancellation/timeout cleanup (temp
directory removed, matching `run_batch`'s existing cancellation test
shape). Plus the same `render_argv` + registry + live-verification
pattern every M2 tool has had. Live verification: real multi-page PDFs,
`qpdf --show-npages`/`pdfinfo` independently confirming each operation's
actual output, not just trusting the result card.

## 10. Repo artifacts

- This spec: `docs/superpowers/specs/2026-09-14-wield-m2d-pdf-tools-design.md`
- Plan (next): `docs/superpowers/plans/2026-09-14-wield-m2d-pdf-tools.md`
- Modified (`wield-core`): `crates/wield-core/src/descriptor.rs`
  (`OutputSpec::Directory`, `CommandSpec.combine_inputs`),
  `crates/wield-core/src/builder.rs` (`CommandSpecBuilder::combine_inputs()`),
  `crates/wield-core/src/template.rs` (spread resolution, `{output_dir}`,
  `input`/`input_stem`/`input_dir` `Paths` fallback),
  `crates/wield-core/src/executor.rs` (`combine_inputs` dispatch skip,
  new `run_split`), `crates/wield-core/src/validate.rs`
  (`OutputSpec::Directory` arm).
- New: `crates/wield-tools/src/pdf_compress.rs`,
  `crates/wield-tools/src/pdf_merge.rs`, `crates/wield-tools/src/pdf_split.rs`.
- Modified: `crates/wield-tools/src/lib.rs`,
  `crates/wield-tools/tests/builtin_registry.rs`,
  `crates/wield-tools/tests/snapshots/builtin_registry.json`, and the
  same mechanical registration-count ripple every prior M2 tool has hit
  (`apps/wield/src-tauri/src/commands.rs`'s registration-order test,
  `apps/wield/src-tauri/tests/commands.rs`'s two tool-count assertions —
  `5` becomes `8`, since this milestone adds three tools at once).
- `docs/backlog.md`: add the split numeric-sort gap and the deferred
  compress-batch gap.
- Branch: `feat/m2d-pdf-tools`, forked from `master` (M2a/b/c all merged).
