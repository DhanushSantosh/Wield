# M2b: `audio.extract` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `audio.extract` — extract or convert an audio track via
ffmpeg — reusing M2a's progress-parsing and multi-file-batch infrastructure
completely unmodified, plus fix a real frontend gap this milestone's design
is the first to expose.

**Architecture:** One new tool descriptor (`crates/wield-tools/src/audio_extract.rs`)
mirroring `video_convert.rs`'s exact builder pattern: `Capability::Command`
running ffmpeg, `ProgressSpec::FfmpegDuration` (unmodified), `multiple: true`
input (batch is automatic via `Executor::run_batch`, already generic).
`format`'s enum value doubles as the output file extension (same mechanism
`video.convert` already relies on) — codec selection is `arg_when`-gated
per format, same pattern as `video.convert`'s resolution-gated `-vf`
segments. One small, necessary frontend fix precedes the descriptor: a
`when`-gated `ArgSpec` field's stale value currently survives being hidden
and gets submitted anyway — `quality`'s `.when(...)` gate (§4 of the spec)
is the first thing in this codebase to make that consequential.

**Tech Stack:** Rust (`wield-core`, `wield-tools`), TypeScript/React
(`apps/wield/src`), ffmpeg (already a project dependency since M2a).

**Spec:** `docs/superpowers/specs/2026-09-14-wield-m2b-audio-extract-design.md`

## Global Constraints

- No new Rust or npm dependencies.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace -- --test-threads=1`, and `npm run check` (run
  from the repo root — it covers lint + typecheck + frontend tests +
  `cargo test` again via `scripts/run-cargo.js`) must all be clean before
  every commit.
- Typed builders only (`DescriptorBuilder`/`ArgSpecBuilder`/`CommandSpecBuilder`)
  — no hand-built `Descriptor`/`ArgSpec`/`CommandSpec` struct literals.
- `format`'s enum value is the output file extension, not necessarily the
  ffmpeg codec name — they differ for one entry (`m4a` → codec `aac`).
  Never rename `format`'s enum options to codec names.
- This branch (`feat/m2b-audio-extract`) is stacked on
  `feat/m2a-video-convert` (not master) — already created, spec commits
  (`25cdce4`, `110d1b9`) already on it. Do not rebase onto master or
  attempt to reconcile with master's state; work from this branch's
  current tip exactly as it is.

---

### Task 1: Fix `ArgForm`'s submit to exclude hidden fields' stale values

**Files:**
- Modify: `apps/wield/src/palette/ArgForm.tsx`
- Test: `apps/wield/src/palette/ArgForm.test.tsx`

**Interfaces:**
- Consumes: `isVisible` from `apps/wield/src/palette/argVisibility.ts`
  (already imported in `ArgForm.tsx`; signature
  `isVisible(spec: ArgSpec, values: Record<string, unknown>, allSpecs: ArgSpec[]): boolean`
  — unchanged, not touched by this task).
- Produces: nothing new — this task changes `ArgForm`'s existing `onSubmit`
  call to filter its payload. `ArgForm`'s public props (`ArgFormProps`) are
  unchanged; Task 2 consumes nothing new from this task beyond "the fix is
  in place before `quality`'s `.when(...)` gate ships".

- [ ] **Step 1: Write the failing test**

Add this test to `apps/wield/src/palette/ArgForm.test.tsx` (the file
already imports `render`, `screen`, `userEvent`, `expect`, `test`, `vi`,
`ArgSpec`, `ToolSummary`, `ArgForm`, `isVisible` — no new imports needed):

```tsx
test("hiding a field via its when-gate excludes its value from submit, even if it was set while visible", async () => {
  const tool: ToolSummary = {
    id: "audio.extract",
    title: "Extract audio",
    keywords: [],
    category: "Convert",
    available: true,
    reason: null,
    args: [
      {
        name: "format",
        label: "Output format",
        help: null,
        arg_type: { Enum: { options: ["mp3", "flac"] } },
        default: { Str: "mp3" },
        required: true,
        when: null,
      },
      {
        name: "quality",
        label: "Bitrate",
        help: null,
        arg_type: { Enum: { options: ["192k", "320k"] } },
        default: null,
        required: false,
        when: { arg: "format", in: [{ Str: "mp3" }] },
      },
    ],
  };
  const onSubmit = vi.fn();
  render(<ArgForm tool={tool} initialValues={{}} onSubmit={onSubmit} onEscape={vi.fn()} />);
  // quality is visible while format=mp3 (the default) - set it.
  await userEvent.selectOptions(screen.getByLabelText("Bitrate"), "320k");
  // Switching format to flac hides quality's field. Its value stays in
  // React state (ArgForm never clears it on a visibility change) - this
  // is exactly the scenario the fix must handle at submit time.
  await userEvent.selectOptions(screen.getByLabelText("Output format"), "flac");
  expect(screen.queryByLabelText("Bitrate")).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole("button", { name: "Extract audio" }));
  expect(onSubmit).toHaveBeenCalledWith({ format: "flac" });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `npm --prefix apps/wield test -- ArgForm.test.tsx`
Expected: FAIL — `onSubmit` is called with
`{ format: "flac", quality: "320k" }` (the stale value still present),
not `{ format: "flac" }`.

- [ ] **Step 3: Fix `ArgForm.tsx`**

In `apps/wield/src/palette/ArgForm.tsx`, find:

```tsx
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    onSubmit(values);
  };
```

Replace with:

```tsx
  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const visibleValues = Object.fromEntries(
      visible.flatMap((spec) => (spec.name in values ? [[spec.name, values[spec.name]]] : [])),
    );
    onSubmit(visibleValues);
  };
```

(`visible` is already computed earlier in the component —
`const visible = tool.args.filter((spec) => isVisible(spec, values, tool.args));`
— and already in scope inside `submit`'s closure. No other change needed.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `npm --prefix apps/wield test -- ArgForm.test.tsx`
Expected: PASS — all tests in the file, including the two pre-existing
ones (`"renders only visible fields and submits their default values"` and
`"initialValues pre-fills the form for Run again"`), which this change
does not affect (neither exercises a hidden field with a stale value).

- [ ] **Step 5: Full gate**

```bash
npm run check
```

Expected: clean (run from the repo root, not `apps/wield` — the root
`package.json`'s `check` script is `lint && typecheck && test`, and `test`
also re-runs `cargo test --workspace` via `scripts/run-cargo.js`).

- [ ] **Step 6: Commit**

```bash
git add apps/wield/src/palette/ArgForm.tsx apps/wield/src/palette/ArgForm.test.tsx
git commit -m "fix(app): exclude a when-hidden field's stale value from ArgForm's submit

ArgForm's submit handler sent the whole form-state object, not just the
currently-visible args - so a field hidden by its own when-gate (e.g.
audio.extract's quality, hidden once format is a lossless value) could
still leak its last-set value into the submitted args if it had been
visible and set earlier in the same form session. No current descriptor
had triggered this (video.convert's optional fields are always visible),
so it was latent until audio.extract's quality field, the first to use
.when() on a Command tool's optional arg, made it reachable.

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 2: `audio.extract` descriptor, registry, and tests

**Files:**
- Create: `crates/wield-tools/src/audio_extract.rs`
- Modify: `crates/wield-tools/src/lib.rs`
- Modify: `crates/wield-tools/tests/builtin_registry.rs`
- Modify: `crates/wield-tools/tests/snapshots/builtin_registry.json` (regenerated, not hand-edited)
- Modify: `apps/wield/src-tauri/src/commands.rs` (mechanical ripple — registration-order test)
- Modify: `apps/wield/src-tauri/tests/commands.rs` (mechanical ripple — two tool-count assertions)

**Interfaces:**
- Consumes: `ProgressSpec::FfmpegDuration` and `CommandSpecBuilder::progress()`
  (`wield-core`, unmodified since M2a); `Executor::run_batch`/`batch_paths`
  (unmodified, triggered automatically by `ArgType::File { multiple: true, .. }`);
  the fixed `ArgForm.tsx` from Task 1 (this task's `quality` field relies on
  it — Task 1 must land first).
- Produces: `audio_extract::descriptor() -> Descriptor`, registered in
  `builtin_registry()` as `"audio.extract"`. Nothing downstream in this
  plan consumes it beyond Task 3's live verification.

- [ ] **Step 1: Write the descriptor**

Create `crates/wield-tools/src/audio_extract.rs`:

```rust
//! The `audio.extract` built-in — extract or convert an audio track via ffmpeg.

use std::time::Duration;
use wield_core::{
    ArgSpecBuilder, ArgType, ArgValueLiteral, Category, CommandSpecBuilder, Descriptor,
    DescriptorBuilder, FileFilter, OutputDir, OutputSpec, ProgressSpec, Requires,
};

const INPUT_EXTS: &[&str] = &[
    "mp4", "mov", "mkv", "webm", "avi", "mp3", "wav", "flac", "m4a", "aac",
];
const FORMATS: &[&str] = &["mp3", "m4a", "flac", "wav"];
const BITRATES: &[&str] = &["128k", "192k", "256k", "320k"];

/// Descriptor for `audio.extract`. Runs `ffmpeg -y -i <input> -vn
/// -codec:a <codec> [-b:a <bitrate>] -progress pipe:2 -nostats <output>`.
/// `codec` is picked per `format` via `arg_when` (libmp3lame/aac/flac/
/// pcm_s16le) - `format`'s enum value is the output *extension* (it feeds
/// `{format}` in the output name template, same mechanism
/// `video.convert` uses), which is why the m4a option's codec is `aac`
/// (two different strings, kept deliberately separate - a literal `.aac`
/// extension would select ffmpeg's ADTS muxer instead of the more
/// compatible, more precise MP4-family `.m4a` container; verified live,
/// see the design spec §3-4). `quality` (a bitrate preset) only applies
/// to the lossy formats (mp3/m4a) and is hidden for flac/wav via its own
/// `when` gate - the same mechanism `video.convert`'s resolution-gated
/// `-vf`/`-crf` segments use. `-vn` is a safe no-op on an audio-only
/// input (verified live), so this one descriptor covers both "extract
/// audio from a video" and "convert between audio formats". See
/// `docs/superpowers/specs/2026-09-14-wield-m2b-audio-extract-design.md`.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("audio.extract", "Extract audio", Category::Convert)
        .keywords(&[
            "audio", "extract", "convert", "ffmpeg", "mp3", "m4a", "flac", "wav", "track",
        ])
        .arg(
            ArgSpecBuilder::new(
                "input",
                "Video or audio",
                ArgType::File {
                    filters: vec![FileFilter {
                        label: "Video & audio".into(),
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
            .default(ArgValueLiteral::Str("mp3".into()))
            .build(),
        )
        .arg(
            ArgSpecBuilder::new(
                "quality",
                "Bitrate",
                ArgType::Enum {
                    options: BITRATES.iter().map(|b| (*b).to_owned()).collect(),
                },
            )
            .when(
                "format",
                vec![
                    ArgValueLiteral::Str("mp3".into()),
                    ArgValueLiteral::Str("m4a".into()),
                ],
            )
            .help("Only applies to mp3/m4a - flac and wav are lossless.")
            .build(),
        )
        .requires(Requires::Binary("ffmpeg".into()))
        .output(OutputSpec::File {
            name: "{input_stem}.{format}".into(),
            dir: OutputDir::SameAsInput,
        })
        .command(
            CommandSpecBuilder::new("ffmpeg")
                .arg("-y")
                .arg("-i")
                .arg("{input}")
                .arg("-vn")
                .arg_when("-codec:a", "format", vec![ArgValueLiteral::Str("mp3".into())])
                .arg_when(
                    "libmp3lame",
                    "format",
                    vec![ArgValueLiteral::Str("mp3".into())],
                )
                .arg_when("-codec:a", "format", vec![ArgValueLiteral::Str("m4a".into())])
                .arg_when("aac", "format", vec![ArgValueLiteral::Str("m4a".into())])
                .arg_when("-codec:a", "format", vec![ArgValueLiteral::Str("flac".into())])
                .arg_when("flac", "format", vec![ArgValueLiteral::Str("flac".into())])
                .arg_when("-codec:a", "format", vec![ArgValueLiteral::Str("wav".into())])
                .arg_when(
                    "pcm_s16le",
                    "format",
                    vec![ArgValueLiteral::Str("wav".into())],
                )
                .arg_when_set("-b:a", "quality")
                .arg_when_set("{quality}", "quality")
                .arg("-progress")
                .arg("pipe:2")
                .arg("-nostats")
                .arg("{output}")
                .progress(ProgressSpec::FfmpegDuration)
                .timeout(Duration::from_secs(300)),
        )
        .build()
        .expect("audio.extract descriptor is valid")
}
```

- [ ] **Step 2: Register in the registry**

In `crates/wield-tools/src/lib.rs`, the module declarations currently read:

```rust
pub mod color_pick;
pub mod image_convert;
pub mod video_convert;
```

Replace with (alphabetical, `audio_extract` sorts first):

```rust
pub mod audio_extract;
pub mod color_pick;
pub mod image_convert;
pub mod video_convert;
```

And `builtin_registry()` currently reads:

```rust
pub fn builtin_registry() -> Registry {
    let mut registry = Registry::new();
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

Replace with (register `audio.extract` first, matching the module order —
`Registry::list()` returns registration order, not a sorted order, so this
is what actually determines the order later tests assert against):

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

- [ ] **Step 3: Write the render_argv tests**

In `crates/wield-tools/tests/builtin_registry.rs`, add two new tests
(place them near the existing `video_convert_renders_ffmpeg_argv_for_present_and_absent_options`,
which this mirrors — the file already imports `Capability`, `ArgValue`,
`BTreeMap`, `render_argv`, `builtin_registry`, no new imports needed):

```rust
#[test]
fn audio_extract_renders_ffmpeg_argv_for_present_and_absent_options() {
    let tool = builtin_registry().get("audio.extract").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };

    let mut minimal = BTreeMap::new();
    minimal.insert("input".to_string(), ArgValue::Path("/clips/a.mp4".into()));
    minimal.insert("format".to_string(), ArgValue::Str("mp3".into()));
    let out = std::path::PathBuf::from("/clips/a.mp3");
    assert_eq!(
        render_argv(&spec.args, &minimal, Some(&out)).unwrap(),
        vec![
            "-y".to_string(),
            "-i".into(),
            "/clips/a.mp4".into(),
            "-vn".into(),
            "-codec:a".into(),
            "libmp3lame".into(),
            "-progress".into(),
            "pipe:2".into(),
            "-nostats".into(),
            "/clips/a.mp3".into(),
        ],
    );

    let mut full = minimal.clone();
    full.insert("quality".to_string(), ArgValue::Str("256k".into()));
    assert_eq!(
        render_argv(&spec.args, &full, Some(&out)).unwrap(),
        vec![
            "-y".to_string(),
            "-i".into(),
            "/clips/a.mp4".into(),
            "-vn".into(),
            "-codec:a".into(),
            "libmp3lame".into(),
            "-b:a".into(),
            "256k".into(),
            "-progress".into(),
            "pipe:2".into(),
            "-nostats".into(),
            "/clips/a.mp3".into(),
        ],
    );
}

#[test]
fn audio_extract_picks_the_right_codec_for_each_format() {
    let tool = builtin_registry().get("audio.extract").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };
    let out = std::path::PathBuf::from("/clips/a.out");

    for (format, codec) in [
        ("mp3", "libmp3lame"),
        ("m4a", "aac"),
        ("flac", "flac"),
        ("wav", "pcm_s16le"),
    ] {
        let mut args = BTreeMap::new();
        args.insert("input".to_string(), ArgValue::Path("/clips/a.mp4".into()));
        args.insert("format".to_string(), ArgValue::Str(format.to_string()));
        let argv = render_argv(&spec.args, &args, Some(&out)).unwrap();
        assert!(
            argv.windows(2).any(|pair| pair == ["-codec:a", codec]),
            "format {format}: expected -codec:a {codec} in {argv:?}"
        );
        for (_, other_codec) in [
            ("mp3", "libmp3lame"),
            ("m4a", "aac"),
            ("flac", "flac"),
            ("wav", "pcm_s16le"),
        ] {
            if other_codec == codec {
                continue;
            }
            assert!(
                !argv.iter().any(|segment| segment == other_codec),
                "format {format}: unexpected codec {other_codec} leaked into {argv:?}"
            );
        }
    }
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
    assert_eq!(ids, vec!["color.pick", "image.convert", "video.convert"]);
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
        vec!["audio.extract", "color.pick", "image.convert", "video.convert"]
    );
}
```

- [ ] **Step 4: Run tests to verify the new ones fail correctly**

Run: `cargo test -p wield-tools`
Expected: `audio_extract_renders_ffmpeg_argv_for_present_and_absent_options`,
`audio_extract_picks_the_right_codec_for_each_format`, and
`registry_has_exactly_the_expected_builtins` all FAIL to compile/run
(`audio_extract` module and `audio.extract` id don't exist yet) — this
confirms Step 1-3 wired the new pieces together correctly once Step 1's
descriptor exists; if Step 1 was already written before Step 3, these
should instead fail only on `builtin_registry_matches_snapshot` (stale
snapshot) and pass everywhere else. Either outcome is fine — the point is
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

Expected: a pure insertion of one new `audio.extract` object — confirm
`color.pick`, `image.convert`, and `video.convert`'s existing entries are
byte-for-byte unchanged in the diff (no lines removed from any of them,
only lines added for the new entry).

- [ ] **Step 7: Fix the mechanical ripple in the app crate**

Two files outside `wield-tools` assert on the exact registered-tool count
or order, and both need updating for the fourth built-in — same class of
change M2a's `video.convert` addition made to these same two files.

In `apps/wield/src-tauri/src/commands.rs`, find the test
`list_tools_with_no_query_returns_registration_order` and its expected
vector:

```rust
vec!["color.pick", "image.convert", "video.convert"]
```

Replace with:

```rust
vec!["audio.extract", "color.pick", "image.convert", "video.convert"]
```

In `apps/wield/src-tauri/tests/commands.rs`, find both `assert_eq!(...len(), 3)`
lines (in `capabilities_lists_every_builtin_with_availability` and
`list_tools_returns_both_builtins_with_arg_schemas`) and change both `3`s
to `4`.

- [ ] **Step 8: Full workspace gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
npm run check
```

All four must be clean.

- [ ] **Step 9: Commit**

```bash
git add crates/wield-tools/src/audio_extract.rs \
  crates/wield-tools/src/lib.rs \
  crates/wield-tools/tests/builtin_registry.rs \
  crates/wield-tools/tests/snapshots/builtin_registry.json \
  apps/wield/src-tauri/src/commands.rs \
  apps/wield/src-tauri/tests/commands.rs
git commit -m "feat(tools): add audio.extract (ffmpeg, reuses M2a's progress/batch infra)

No wield-core changes - FfmpegDuration and Executor::run_batch are
consumed exactly as M2a built them. format's enum value is the output
extension (m4a, not aac) to get the more compatible, more precise
MP4-family container rather than a raw ADTS stream (verified live, see
the design spec). quality is a bitrate preset (Enum), not a raw numeric
range, and is the first ArgSpec in this codebase to use .when() on a
Command tool's optional field - safe now that ArgForm's stale-hidden-
value gap is fixed (previous commit).

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 3: Live verification and `docs/testing.md` write-up

**No subagent dispatch for this task** — matches M2a's own Task 5
precedent: this class of work needs live desktop tooling (a real file
picker, real ffmpeg runs, `ffprobe`-independent confirmation) that only
the orchestrator session driving this plan has access to. Whoever is
executing this plan (GEON, per the confirmed collaboration model, or a
future reader of this plan) does this task directly, not via a dispatched
implementer.

**Files:**
- Modify: `docs/testing.md`

**Interfaces:**
- Consumes: the real, installed `audio.extract` tool via the real desktop
  UI (search → arg form → run → result card) — everything built in
  Tasks 1-2.
- Produces: nothing further in this plan consumes this task's output; it's
  the plan's final deliverable.

- [ ] **Step 1: Build and install a fresh binary**

```bash
npm run build -w apps/wield -- --no-bundle
```

(Never bare `cargo build` for anything meant to actually run — it skips
Tauri's `custom-protocol` feature and the app fails to find its frontend
assets. This project's established build step, unchanged since M1.)

- [ ] **Step 2: Single-file extraction from a real video**

Search "audio" in the palette, select `Extract audio`, pick a real test
video file (one with both video and audio streams — reuse or regenerate
one via `ffmpeg -f lavfi -i testsrc=... -f lavfi -i sine=... ...` if no
real sample is handy) through the native file picker, run with default
options (`format=mp3`). Confirm the result card shows a real
`ToolOutcome::File` success. Verify independently via `ffprobe` (not just
the UI): the output is a valid audio file, its duration matches the
source's, and it's genuinely encoded (not a copy) — e.g. compare file size
against a raw PCM-equivalent estimate, or confirm the codec via
`ffprobe -show_entries stream=codec_name`.

- [ ] **Step 3: Audio-to-audio conversion**

Run again on a source that is itself audio (e.g. a `.wav` file), converting
to a different format (e.g. `flac`). Confirm this works identically to the
video-source case — this is the part of the design (§1 of the spec) that
distinguishes `audio.extract` from a video-only tool, so it needs its own
explicit live confirmation, not just an inference from the video case.

- [ ] **Step 4: Multi-file batch**

Select 2-3 files at once through the native multi-select picker, run the
conversion. Confirm the result card shows a real `ToolOutcome::Report`
(`"N of N converted"`), and independently `ffprobe`-verify each output.

- [ ] **Step 5: `quality` field visibility**

In the arg form, confirm live that selecting `format=mp3` shows the
`Bitrate` field, and switching to `format=flac` or `format=wav` hides it —
and, since Task 1's fix is what makes this safe, confirm a run made after
switching away from mp3 does NOT include a stale `-b:a` flag (this can be
checked by comparing the produced file's properties against a `-b:a`-free
manual `ffmpeg` run, or more directly by checking Wield's own tracing log
at `$XDG_STATE_HOME/wield/logs/` for the actual rendered argv if it's
logged there — check what M2a's live verification used for this kind of
confirmation and reuse the same method).

- [ ] **Step 6: Write up `docs/testing.md`**

Add a new section following the exact pattern of the existing "M2a:
`video.convert`" section (read it first for the tone/structure to match):
what was verified and how, honest gaps if anything couldn't be reproduced
live (state the reason, same standard as M2a's — testing-tool friction is
an acceptable, honestly-documented reason; skipping verification silently
is not), and explicit confirmation that Task 1's `ArgForm` fix behaves
correctly live, not just in its unit test.

- [ ] **Step 7: Full gate one more time**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
npm run check
```

- [ ] **Step 8: Commit**

```bash
git add docs/testing.md
git commit -m "docs(testing): verify audio.extract live — video source, audio source, batch

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```
