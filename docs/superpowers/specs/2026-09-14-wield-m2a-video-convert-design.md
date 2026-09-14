# Wield — M2a: `video.convert` + Real Progress + Multi-File Batch Design

## 1. Why this exists

M1 shipped `image.convert` as the proof of the `Command` capability path, but it
proved only the simplest slice of it: ImageMagick's `convert` is near-instant,
so `ProgressSpec` only ever needed its one stub variant (`None`), and every
tool has always run against exactly one input file. M2 (the "Converter suite"
milestone — `video.convert`, `audio.extract`, `document.convert`, `pdf.tools`,
per `docs/superpowers/specs/2026-09-10-wield-design.md` §6) needs two things
`image.convert` never exercised: **real, live progress** for
genuinely-long-running conversions, and **multi-file batch**, since converting
one file at a time is a real friction point for exactly the kind of tool this
milestone adds.

This is M2a specifically: build both of those cross-cutting mechanisms, proven
through one real tool (`video.convert`, backed by `ffmpeg`) rather than
designing them speculatively against four tools' worth of guessed
requirements. `audio.extract`/`document.convert`/`pdf.tools` (M2b–d) then
reuse this plumbing, adding only their own descriptor + backing-binary
specifics.

## 2. What's already there, verified against the actual code and a live run

Both mechanisms this milestone needs turn out to already have real,
purpose-built extension points in `wield-core` — not something to build from
scratch:

- **`ProgressSpec`/`ProgressParser`** (`command.rs`): `RunSpec.progress_spec`
  is handed to `parser_for()`, which returns an `Option<Box<dyn ProgressParser
  + Send>>`; `drain_stderr` already calls `parser.parse_line(line)` per stderr
  line and forwards any `Some(Progress)` it returns over the same mpsc channel
  `Progress::Started`/`Finished` already use. `ProgressSpec` currently has
  exactly one variant, `None` — a stub, not a limitation of the mechanism
  itself.
- **`ArgValue::Paths(Vec<PathBuf>)`** (`args.rs`) and `ArgType::File {
  multiple: bool }` (`descriptor.rs`) both already exist, and
  `value_satisfies_type` already accepts a `Paths` value for a `multiple:
  true` File arg. The frontend's `FormField.tsx` already branches on
  `Array.isArray(value)` for a multi-select file field. None of this is
  wired end-to-end today: `template.rs::render_argv` explicitly renders
  `ArgValue::Paths(_) => Ok(None)` (silently drops it from argv, doesn't
  error), and — the actual reason multi-file has never worked at all —
  `commands.rs::coerce_json_args` has no branch for a `multiple: true` File
  arg receiving a JSON array; it unconditionally calls `value.as_str()`,
  which fails outright on an array.
- **`RunningView.tsx`** already renders a live percentage bar whenever
  `Progress::Percent(n)` arrives, and a status line whenever
  `Progress::Message(s)` arrives (`{toolTitle}…` otherwise). **Zero frontend
  changes are needed for either progress or batch-status narration** — both
  already render correctly given the right `Progress` values from the
  backend.
- **`ToolOutcome::Report { title, lines }`** already exists and already has a
  result card (`ResultView`'s "5 result kinds", built in P6a). This is the
  natural batch-completion summary — no new outcome type needed.
- **`OutputDir::SameAsInput` + `"{input_stem}.{format}"` name templating**
  (`OutputSpec::File`) already compute a fresh, correct per-file output path
  from whatever `ArgValue::Path` is in the effective args at render time —
  applies automatically per-iteration in a batch loop with no changes.

So M2a's real, new work is narrow: one new `ProgressSpec` variant + parser,
one new branch in `coerce_json_args`, one new orchestration path in
`Executor::run`, and the `video.convert` descriptor itself.

## 3. Progress parsing: `FfmpegDuration`

Verified live against the actually-installed `ffmpeg` (`n9.0`) on this dev
machine, not assumed from docs alone — ran real test conversions with
`ffmpeg -progress pipe:2 -nostats`:

- `-progress pipe:2` puts one `key=value` line per field on **stderr**
  (matching the existing single-stream `drain_stderr` design — no second
  pipe to plumb through `RunSpec`).
- `-nostats` suppresses ffmpeg's default `\r`-overwriting per-frame status
  line (which would otherwise confuse line-based reading), but **not** the
  one-time final `frame=N fps=... Lsize=...` summary line printed at the end
  regardless — confirmed live, this line appears once, interleaved with the
  tail of the progress blocks. It doesn't match any pattern the parser looks
  for, so it's silently ignored — no special-casing required; the parser
  design is inherently robust to ffmpeg's surrounding noise.
- The startup banner (`Duration: 00:00:03.00, start: 0.000000, bitrate: ...`)
  is still printed, once, before any progress blocks, regardless of
  `-nostats`. **No separate `ffprobe` call is needed** to learn the total
  duration — it's already on the same stream, in a stable, long-documented
  format.
- `out_time_us` is the field to trust for elapsed time, not `out_time_ms` —
  confirmed live, both fields report the *identical raw number* on this
  ffmpeg build despite the name (`out_time_ms=3840000` alongside
  `out_time_us=3840000` for the same instant) — a known, long-standing
  ffmpeg quirk. Using the field whose name actually matches its unit avoids
  depending on that historical inconsistency.
- `progress=continue` / `progress=end` mark each block (verified live: it's
  `continue`, not `continuing` as at least one secondary source paraphrases
  it) — not needed by this parser, since `Progress::Finished` is already
  sent unconditionally by `CommandRunner::execute` once the process exits,
  independent of what the parser itself does.

`FfmpegDuration` is a small, stateful `ProgressParser`:

```rust
pub(crate) struct FfmpegDuration {
    total_secs: Option<f64>,
}

impl ProgressParser for FfmpegDuration {
    fn parse_line(&mut self, line: &str) -> Option<Progress> {
        if self.total_secs.is_none() {
            if let Some(rest) = line.trim_start().strip_prefix("Duration: ") {
                let timecode = rest.split(',').next()?;
                self.total_secs = parse_timecode(timecode);
            }
            return None;
        }
        let out_time_us = line.strip_prefix("out_time_us=")?.trim().parse::<u64>().ok()?;
        let total = self.total_secs?;
        if total <= 0.0 {
            return None;
        }
        let percent = ((out_time_us as f64 / 1_000_000.0) / total * 100.0).clamp(0.0, 100.0);
        Some(Progress::Percent(percent.round() as u8))
    }
}

fn parse_timecode(text: &str) -> Option<f64> {
    let mut parts = text.trim().splitn(3, ':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}
```

Plain string parsing (`strip_prefix`/`split`), matching how `hint_for_stderr`
already does plain substring checks rather than reaching for a `regex`
dependency this workspace doesn't otherwise need. Defensive throughout:
a malformed or unexpected line just returns `None` (no panic, no update) —
the same graceful-degradation the rest of this parser leans on for ffmpeg's
other noise.

`ProgressSpec` gains one new variant:

```rust
pub enum ProgressSpec {
    None,
    FfmpegDuration,
}
```

`parser_for` gains one new match arm constructing `FfmpegDuration { total_secs: None }`.

## 4. `video.convert` descriptor

Same shape as `image.convert`, same builders, same conventions:

- `input`: `ArgType::File { filters: [common video extensions: mp4, mov, mkv,
  webm, avi], multiple: true }`, required. **The only descriptor-level
  difference from `image.convert`'s `input` that enables batch.**
- `format`: `ArgType::Enum { options: [mp4, webm, mkv] }`, required, default
  `mp4`.
- `resolution`: optional `ArgType::Enum` (e.g. `["original", "1080p", "720p",
  "480p"]`) — mapped to an ffmpeg `-vf scale=...` filter only `when` set to
  something other than `original`, reusing `arg_when_set`'s existing
  presence-gating.
- `quality`: optional `ArgType::Int { range: [0, 51] }` (libx264 CRF scale —
  lower is higher quality; documented in the arg's `help` text since the
  direction is the opposite of "quality" intuition), mapped to `-crf
  {quality}` only when set.
- `Requires::Binary("ffmpeg")`.
- `output: OutputSpec::File { name: "{input_stem}.{format}", dir:
  OutputDir::SameAsInput }` — reused verbatim.
- `command: CommandSpecBuilder::new("ffmpeg").arg("-y").arg("-i").arg("{input}")
  .arg_when_set("-vf", "resolution").arg_when_set("scale=-2:{resolution_px}",
  "resolution").arg_when_set("-crf", "quality").arg_when_set("{quality}",
  "quality").arg("-progress").arg("pipe:2").arg("-nostats").arg("{output}")
  .progress(ProgressSpec::FfmpegDuration).timeout(Duration::from_secs(1800))`
  — timeout at 30 minutes, generous for a single video vs. `image.convert`'s
  120s; exact value is a judgment call, not load-bearing, easy to revisit.
  `-y` (overwrite without prompting) is required since Wield always targets
  a fresh temp path (`OutputPlan`) — without it, ffmpeg would block waiting
  for interactive stdin confirmation that never comes.

The `resolution` → filter mapping needs one concrete decision at
implementation time: mapping `"1080p"`/`"720p"`/`"480p"` to actual pixel
heights for the `scale=-2:H` filter (the `-2` keeps width even and
aspect-correct) is mechanical, not architectural — left to the plan/impl
rather than spelled out symbolically here.

## 5. Multi-file batch

### 5.1 The missing link, precisely

Three touch points, each small:

1. **`commands.rs::coerce_json_args`** — new branch: when `spec.arg_type` is
   `ArgType::File { multiple: true, .. }` and the JSON value is an array,
   map each element through the existing single-path coercion and collect
   into `ArgValue::Paths(Vec<PathBuf>)`. The existing `value.as_str()`
   branch keeps handling `multiple: false` unchanged.
2. **`executor.rs::Executor::run`** — after `validate_args` produces
   `effective`, check whether any entry is `ArgValue::Paths`. If so, hand off
   to a new `run_batch` path instead of the existing single-file
   `Capability::Command`/`Portal`/`Native` dispatch (batch is Command-only in
   practice — `Portal`/`Native` capabilities have no multi-file arg in this
   milestone's scope; `run_batch` only needs to handle `Capability::Command`,
   returning a `Failed` outcome if a `Paths` value somehow reaches a
   non-Command capability, which validation should already prevent given
   only File args can be `Paths` and no current Portal/Native tool has one).
3. **`video.convert`'s descriptor** — `multiple: true` on `input`, as in §4.

### 5.2 `run_batch`

```rust
async fn run_batch(
    &self,
    descriptor: &Descriptor,
    command: &CommandSpec,
    effective: &ArgMap,
    multi_arg: &str,
    paths: &[PathBuf],
    progress: mpsc::Sender<Progress>,
    cancel: CancellationToken,
) -> ToolOutcome {
    let mut lines = Vec::new();
    let mut succeeded = 0usize;
    for (index, path) in paths.iter().enumerate() {
        if cancel.is_cancelled() {
            return ToolOutcome::Cancelled;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        let _ = progress.try_send(Progress::Message(
            format!("Converting {} of {}: {name}", index + 1, paths.len()),
        ));
        let mut single = effective.clone();
        single.insert(multi_arg.to_owned(), ArgValue::Path(path.clone()));
        let outcome = self.run_command(descriptor, command, &single, progress.clone(), cancel.clone()).await;
        match &outcome {
            ToolOutcome::File { path } => {
                succeeded += 1;
                lines.push(format!("✓ {name} → {}", path.display()));
            }
            ToolOutcome::Cancelled => return ToolOutcome::Cancelled,
            other => lines.push(format!("✗ {name}: {}", outcome_summary(other))),
        }
    }
    ToolOutcome::Report {
        title: format!("{succeeded} of {} converted", paths.len()),
        lines,
    }
}
```

(`outcome_summary` is a small new helper turning `Failed`/`Unavailable` into
one readable line — exact wording is an implementation detail, not
architectural.)

Sequential, not concurrent — deliberately: N parallel `ffmpeg` processes
would contend for CPU/IO in a way that makes each one's own progress
reporting misleading (and this milestone doesn't need the throughput), and
sequential execution is what makes "one cancellation token cancels the whole
batch, cleanly, between files" trivial to reason about. Worth revisiting only
if batch throughput turns out to matter in practice — not assumed now.

One cancellation-token span covers the whole batch: pressing Escape while a
batch is running behaves exactly like cancelling today's single run,
just scoped to "this batch" as the one logical operation — matching the
existing UX (`RunningView`'s "esc to cancel" hint needs no wording change).

### 5.3 Frontend

No new components. `FormField.tsx` already renders a multi-select picker and
already stores an array once `multiple: true` is set on the arg (confirmed by
reading its existing `Array.isArray(value)` branch) — flipping that one
descriptor flag is expected to be sufficient, but the plan should verify
the exact value shape sent over `invoke("run_tool", ...)` matches what
`coerce_json_args`'s new branch expects (an array of path strings) as a real
implementation-time check, not assumed correct from reading alone.

## 6. Risks and honest gaps

1. **`-nostats`'s one leftover summary line was found empirically, not
   documented anywhere searched.** If a *different* installed ffmpeg build
   or version prints something that happens to collide with the
   `Duration: `/`out_time_us=` patterns this parser matches, it could emit a
   spurious progress update. Low risk (both prefixes are specific enough
   that a collision is unlikely) and low cost if wrong (a single wrong
   percent reading, self-correcting on the next real update) — not chased
   further with defensive-format-validation that would add real complexity
   for a hazard this narrow.
2. **`resolution`'s exact enum-to-pixel-height mapping isn't pinned in this
   spec.** Mechanical, not risky — flagged as a plan-time decision rather
   than guessed here.
3. **Batch progress narration ("file 2 of 4") replaces per-file percent
   granularity at the batch level** — there's no "38% through the whole
   batch" figure, only "38% through the current file, which is file 2 of
   4." Matches what the existing `Progress` model can express without
   inventing a new type; judged sufficient for this milestone.
4. **No Flatpak bundling of `ffmpeg` in this milestone** — matches
   `image.convert`'s own precedent (`magick` unbundled) and the owner's
   2026-09-14 decision to keep deferring binary bundling; `video.convert`
   will report `Unavailable` inside the Flatpak build specifically, same as
   `image.convert` does today. Tracked in `docs/backlog.md`.

## 7. Testing strategy

- **`FfmpegDuration`**: pure unit tests feeding real captured line sequences
  (the exact banner + progress-block text captured during this spec's own
  live verification) — no live ffmpeg process needed, matching how
  `command_runner`'s existing tests already avoid spawning real binaries
  where the logic under test doesn't require it.
- **`video.convert` descriptor**: argv-rendering tests for present/absent
  optional args, mirroring `image_convert.rs`'s existing test shape exactly.
- **`run_batch`**: unit tests against a fake/mock command path (not real
  ffmpeg) verifying: Report aggregation with mixed success/failure,
  mid-batch cancellation stopping before the next file, per-file output
  naming.
- **Live verification** (this project's established convention for
  anything that ultimately depends on a real subprocess/UI, per
  `docs/testing.md`'s whole existing pattern): a real multi-file
  `video.convert` batch run against real short test clips, confirming the
  percentage bar actually animates, the "file N of M" message narrates
  correctly, and the final Report card lists real per-file results.

## 8. Repo artifacts

- `crates/wield-core/src/command.rs`: `FfmpegDuration` parser + new
  `ProgressSpec::FfmpegDuration` match arm in `parser_for`.
- `crates/wield-core/src/descriptor.rs`: new `ProgressSpec` variant.
- `crates/wield-core/src/executor.rs`: batch detection + `run_batch`.
- `crates/wield-tools/src/video_convert.rs` (new): the descriptor.
- `crates/wield-tools/src/lib.rs`: register `video.convert` in
  `builtin_registry()`; snapshot update.
- `apps/wield/src-tauri/src/commands.rs`: `coerce_json_args`'s new
  multi-file branch.
- `apps/wield/src/lib/wield.ts`: TS type update if the current `ArgValue`
  mirror doesn't already accept an array for a File arg's value.
- `docs/testing.md`: new section for this milestone's live verification,
  matching the project's established convention.
