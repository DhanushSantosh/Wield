# M2a: video.convert + Real Progress + Multi-File Batch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `video.convert` (ffmpeg-backed) with real, live percentage progress and multi-file batch conversion — the first M2 (Converter suite) tool, and the milestone that proves the two mechanisms M2b–d reuse.

**Architecture:** Two already-existing-but-unwired extension points in `wield-core` get real implementations: `ProgressSpec`/`ProgressParser` gains an `FfmpegDuration` variant that parses ffmpeg's `-progress pipe:2` stderr output against its own startup `Duration:` banner (no `ffprobe` call needed); `ArgValue::Paths` (already accepted by validation, already renderable per-file by the *existing* single-file `render_argv`/`compute_output_path` once expanded) gets a small batch loop inside `Executor::run_command` that calls the *unmodified* single-file path once per selected file and aggregates a `ToolOutcome::Report`. The one genuinely missing link — `coerce_json_args` never had a branch turning a JSON array into `ArgValue::Paths` — is the actual reason batch has never worked end-to-end, not a deeper gap.

**Tech Stack:** Rust (`wield-core`, `wield-tools`, `wield-app` crates), `ffmpeg`/`ffprobe` (system binary, not bundled — see Global Constraints), Tokio async, existing Tauri IPC (`run_tool` command, unchanged signature).

**Spec:** `docs/superpowers/specs/2026-09-14-wield-m2a-video-convert-design.md`

## Global Constraints

- No Flatpak bundling of `ffmpeg` in this milestone (owner decision, 2026-09-14) — `video.convert` reports `Unavailable` inside the Flatpak build specifically, works fully on native installs via `$PATH`, matching `image.convert`'s existing precedent. Tracked in `docs/backlog.md`, not this plan's job to touch.
- No Rust dependency additions — progress parsing uses plain string parsing (`strip_prefix`/`split`), matching how `outcome.rs::hint_for_stderr` already does plain substring checks rather than a `regex` crate.
- Sequential batch execution, not concurrent — one file converts at a time; a later milestone can revisit throughput if it turns out to matter, not assumed now.
- One `CancellationToken` spans an entire batch — cancelling stops before the next file starts (or, if already inside a file's own `CommandRunner::execute`, cancels that file's process exactly like today's single-run cancel) and returns `ToolOutcome::Cancelled` for the whole batch, not a partial `Report`.
- `fmt`/`clippy -D warnings`/`cargo test --workspace`/`npm run check` must all be clean before every commit (this project's standing convention, unchanged from every prior milestone).
- Every new descriptor is authored with the typed builders (`DescriptorBuilder`/`ArgSpecBuilder`/`CommandSpecBuilder`), matching `image_convert.rs`'s existing pattern — never hand-built `Descriptor` structs outside tests.

---

### Task 1: `ProgressSpec::FfmpegDuration` — the parser, verified against real captured ffmpeg output

**Files:**
- Modify: `crates/wield-core/src/descriptor.rs` (add a `ProgressSpec` variant)
- Modify: `crates/wield-core/src/builder.rs` (add a `.progress()` setter to `CommandSpecBuilder` — it currently hardcodes `ProgressSpec::None` in `build()` with no way to override it)
- Modify: `crates/wield-core/src/command.rs` (the parser itself + a new `parser_for` match arm)
- Test: `crates/wield-core/tests/command_runner.rs` (new test, same file, same `support::write_stub_script` pattern every other test in it already uses)

**Interfaces:**
- Consumes: nothing from another task in this plan.
- Produces: `wield_core::descriptor::ProgressSpec::FfmpegDuration` (a unit variant, `pub`) and `CommandSpecBuilder::progress(self, spec: ProgressSpec) -> Self` (`pub`) — Task 4's `video.convert` descriptor calls `.progress(ProgressSpec::FfmpegDuration)` on the command builder.

- [ ] **Step 1: Add the new `ProgressSpec` variant**

In `crates/wield-core/src/descriptor.rs`, find:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProgressSpec {
    None,
}
```

Replace with:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProgressSpec {
    None,
    /// Parses ffmpeg's `-progress pipe:2 -nostats` stderr output against its
    /// own startup `Duration:` banner line - see `command.rs`'s
    /// `FfmpegDuration` for the parser. No `ffprobe` call needed; the total
    /// duration is already on the same stream.
    FfmpegDuration,
}
```

- [ ] **Step 2: Add a `.progress()` setter to `CommandSpecBuilder`**

In `crates/wield-core/src/builder.rs`, `CommandSpecBuilder` currently has no way
to set anything but the default `ProgressSpec::None` — its `build()` hardcodes
it. Find:

```rust
#[derive(Debug, Clone)]
pub struct CommandSpecBuilder {
    binary: String,
    args: Vec<CommandArg>,
    timeout: Duration,
}

impl CommandSpecBuilder {
    pub fn new(binary: &str) -> Self {
        Self {
            binary: binary.to_owned(),
            args: Vec::new(),
            timeout: Duration::from_secs(300),
        }
    }
```

Replace with (adds one field, initializes it in `new()`):

```rust
#[derive(Debug, Clone)]
pub struct CommandSpecBuilder {
    binary: String,
    args: Vec<CommandArg>,
    timeout: Duration,
    progress: ProgressSpec,
}

impl CommandSpecBuilder {
    pub fn new(binary: &str) -> Self {
        Self {
            binary: binary.to_owned(),
            args: Vec::new(),
            timeout: Duration::from_secs(300),
            progress: ProgressSpec::None,
        }
    }

    pub fn progress(mut self, progress: ProgressSpec) -> Self {
        self.progress = progress;
        self
    }
```

(The new `progress` method goes right after `new()`, before the existing `arg` method — exact position within the `impl` block doesn't matter, just keep it inside `impl CommandSpecBuilder`.)

Then find `build()`:

```rust
    pub fn build(self) -> CommandSpec {
        CommandSpec {
            binary: self.binary,
            args: self.args,
            progress: ProgressSpec::None,
            timeout: self.timeout,
            success: SuccessSpec::ExitZero,
        }
    }
```

Replace `progress: ProgressSpec::None,` with `progress: self.progress,`.

- [ ] **Step 3: Write the parser**

In `crates/wield-core/src/command.rs`, find:

```rust
pub(crate) fn parser_for(spec: &ProgressSpec) -> Option<Box<dyn ProgressParser + Send>> {
    match spec {
        ProgressSpec::None => None,
    }
}
```

Replace with:

```rust
pub(crate) fn parser_for(spec: &ProgressSpec) -> Option<Box<dyn ProgressParser + Send>> {
    match spec {
        ProgressSpec::None => None,
        ProgressSpec::FfmpegDuration => Some(Box::new(FfmpegDuration { total_secs: None })),
    }
}

/// Parses ffmpeg's `-progress pipe:2 -nostats` output into `Progress::Percent`.
///
/// Verified live against the actually-installed ffmpeg (n9.0) on the dev
/// machine, not assumed from docs: the startup banner
/// (`Duration: 00:00:03.00, start: 0.000000, bitrate: ...`) is printed once
/// on stderr before any progress blocks, regardless of `-nostats` - so no
/// separate `ffprobe` call is needed to learn the total duration, it's
/// already on the same stream this parser already sees. `out_time_us` is
/// the field trusted for elapsed time, not `out_time_ms` - confirmed live,
/// both fields report the *identical raw number* on this ffmpeg build
/// despite the name (a long-standing, documented ffmpeg quirk). Any line
/// that doesn't match either pattern (ffmpeg's own noise: the version/config
/// banner, per-codec stats, the one-time final `Lsize=` summary line that
/// prints even with `-nostats`) is silently ignored, not an error - this
/// parser is deliberately tolerant of ffmpeg's surrounding output.
struct FfmpegDuration {
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
        let out_time_us: u64 = line.strip_prefix("out_time_us=")?.trim().parse().ok()?;
        let total = self.total_secs?;
        if total <= 0.0 {
            return None;
        }
        let elapsed_secs = out_time_us as f64 / 1_000_000.0;
        let percent = (elapsed_secs / total * 100.0).clamp(0.0, 100.0);
        Some(Progress::Percent(percent.round() as u8))
    }
}

/// Parses ffmpeg's `HH:MM:SS.ss` timecode format (as seen in its `Duration:`
/// banner line) into total seconds. Returns `None` on anything malformed -
/// this parser only ever degrades to "no progress updates", never panics.
fn parse_timecode(text: &str) -> Option<f64> {
    let mut parts = text.trim().splitn(3, ':');
    let hours: f64 = parts.next()?.parse().ok()?;
    let minutes: f64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}
```

Add `use crate::outcome::Progress;` to `command.rs`'s imports if not already
present (it already is - `command.rs` already imports `crate::outcome::Progress`
at the top of the file for the existing `Progress::Started`/`Finished` sends).

- [ ] **Step 4: Run the existing tests to confirm nothing broke**

Run: `cargo test -p wield-core`
Expected: all existing tests still pass (this step only adds new code paths, doesn't touch existing ones).

- [ ] **Step 5: Write the failing test**

Add to `crates/wield-core/tests/command_runner.rs` (after the existing
`emits_started_then_finished_for_progress_none` test, same file - it already
has `mod support;` and the needed imports at the top):

```rust
#[tokio::test]
async fn ffmpeg_duration_parser_reports_percent_from_real_captured_output() {
    let dir = tempfile::tempdir().unwrap();
    // Exact lines captured live from a real `ffmpeg -progress pipe:2
    // -nostats` run during this feature's own design verification - not
    // synthesized, the real shape ffmpeg actually produces (including the
    // noise this parser must tolerate: the leftover `frame=  200 fps=0.0
    // ... Lsize=...` summary line `-nostats` does NOT suppress).
    let script = r#"#!/bin/sh
cat <<'EOF' 1>&2
ffmpeg version n9.0 Copyright (c) 2000-2026 the FFmpeg developers
Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'test-input.mp4':
  Duration: 00:00:08.00, start: 0.000000, bitrate: 47 kb/s
Stream mapping:
  Stream #0:0 -> #0:0 (h264 (native) -> h264 (libx264))
frame=98
fps=0.00
stream_0_0_q=28.0
bitrate=   0.1kbits/s
total_size=48
out_time_us=3840000
out_time_ms=3840000
out_time=00:00:03.840000
dup_frames=0
drop_frames=0
speed=7.65x
progress=continue
frame=  200 fps=0.0 q=-1.0 Lsize=      45KiB time=00:00:07.92 bitrate=  46.3kbits/s speed=9.97x elapsed=0:00:00.79
frame=200
fps=0.00
stream_0_0_q=-1.0
bitrate=  46.3kbits/s
total_size=45886
out_time_us=7920000
out_time_ms=7920000
out_time=00:00:07.920000
dup_frames=0
drop_frames=0
speed=9.97x
progress=end
EOF
echo done > "$2"
"#;
    let script = support::write_stub_script(dir.path(), "fake-ffmpeg", script);
    let plan = OutputPlan::for_final(dir.path().join("out.mp4"));
    let (tx, mut rx) = mpsc::channel(16);
    let argv = vec![
        "ignored".to_string(),
        plan.temp.to_string_lossy().into_owned(),
    ];
    let result = CommandRunner::execute(RunSpec {
        binary: &script,
        argv: &argv,
        cwd: None,
        output: Some(&plan),
        progress_spec: &ProgressSpec::FfmpegDuration,
        success: &SuccessSpec::ExitZero,
        timeout: Duration::from_secs(5),
        progress: tx,
        cancel: CancellationToken::new(),
    })
    .await;
    assert!(matches!(result, CommandResult::Success { .. }));

    let mut percents = vec![];
    while let Ok(progress) = rx.try_recv() {
        if let wield_core::outcome::Progress::Percent(p) = progress {
            percents.push(p);
        }
    }
    // 3.84s / 8.00s = 48%, 7.92s / 8.00s = 99% (rounds down from 99.0).
    assert_eq!(percents, vec![48, 99]);
}
```

- [ ] **Step 6: Run it to verify it fails**

Run: `cargo test -p wield-core ffmpeg_duration_parser_reports_percent_from_real_captured_output`
Expected: FAIL (percents is empty, or `ProgressSpec::FfmpegDuration` doesn't exist yet if Steps 1-3 weren't done first - run this after Steps 1-3, so it should fail only on the assertion, not a compile error).

- [ ] **Step 7: Run it to verify it passes**

Run: `cargo test -p wield-core ffmpeg_duration_parser_reports_percent_from_real_captured_output`
Expected: PASS.

- [ ] **Step 8: Run the full gate and commit**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace -- --test-threads=1`
Expected: all clean.

```bash
git add crates/wield-core/src/descriptor.rs crates/wield-core/src/builder.rs crates/wield-core/src/command.rs crates/wield-core/tests/command_runner.rs
git commit -m "feat(core): parse real ffmpeg progress via a new ProgressSpec variant"
```

---

### Task 2: Multi-file batch — `Executor::run_command`'s new batch path

**Files:**
- Modify: `crates/wield-core/src/executor.rs`
- Test: `crates/wield-core/tests/executor_pipeline.rs`

**Interfaces:**
- Consumes: `wield_core::args::ArgValue::Paths` (already exists), `wield_core::outcome::ToolOutcome::Report` (already exists) - nothing from Task 1.
- Produces: nothing a later task in this plan calls directly by name - `run_batch` is a private (`async fn`, no `pub`) method reached automatically through the existing public `Executor::run` whenever the effective args contain an `ArgValue::Paths` value. Task 4's `video.convert` descriptor sets `multiple: true` on its `input` arg, which is what makes this path reachable for that tool - no new public API surface for Task 4 to call.

- [ ] **Step 1: Write the failing test**

Add to `crates/wield-core/tests/executor_pipeline.rs` (after the existing
`built_descriptor_runs_through_registry_and_executor` test - the file already
imports everything this test needs):

```rust
#[tokio::test]
async fn batch_input_converts_each_file_and_reports_a_summary() {
    let dir = tempfile::tempdir().unwrap();
    support::write_stub_script(dir.path(), "batch-conv", "#!/bin/sh\ncat \"$1\" > \"$2\"\n");
    let first = dir.path().join("a.raw");
    let second = dir.path().join("b.raw");
    std::fs::write(&first, b"A").unwrap();
    std::fs::write(&second, b"B").unwrap();

    let mut descriptor = convert_descriptor("batch-conv");
    descriptor.args[0].arg_type = ArgType::File {
        filters: vec![],
        multiple: true,
    };

    let executor = Executor::new(BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]));
    let mut args = BTreeMap::new();
    args.insert(
        "input".to_string(),
        ArgValue::Paths(vec![first.clone(), second.clone()]),
    );
    let (tx, mut rx) = mpsc::channel(64);
    let outcome = executor
        .run(
            ExecutionRequest { descriptor, args },
            tx,
            CancellationToken::new(),
        )
        .await;

    match outcome {
        ToolOutcome::Report { title, lines } => {
            assert_eq!(title, "2 of 2 converted");
            assert_eq!(lines.len(), 2);
            assert!(lines[0].contains("a.raw"));
            assert!(lines[1].contains("b.raw"));
        }
        other => panic!("expected Report, got {other:?}"),
    }
    assert_eq!(std::fs::read(dir.path().join("a.out")).unwrap(), b"A");
    assert_eq!(std::fs::read(dir.path().join("b.out")).unwrap(), b"B");

    let mut messages = vec![];
    while let Ok(progress) = rx.try_recv() {
        if let wield_core::outcome::Progress::Message(m) = progress {
            messages.push(m);
        }
    }
    assert!(messages.iter().any(|m| m.contains("1 of 2")));
    assert!(messages.iter().any(|m| m.contains("2 of 2")));
}

#[tokio::test]
async fn batch_stops_before_the_next_file_once_cancelled() {
    let dir = tempfile::tempdir().unwrap();
    support::write_stub_script(dir.path(), "slow-conv", "#!/bin/sh\nsleep 30\n");
    let first = dir.path().join("a.raw");
    let second = dir.path().join("b.raw");
    std::fs::write(&first, b"A").unwrap();
    std::fs::write(&second, b"B").unwrap();

    let mut descriptor = convert_descriptor("slow-conv");
    descriptor.args[0].arg_type = ArgType::File {
        filters: vec![],
        multiple: true,
    };
    if let Capability::Command(spec) = &mut descriptor.capability {
        spec.timeout = Duration::from_secs(30);
    }

    let executor = Executor::new(BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]));
    let mut args = BTreeMap::new();
    args.insert("input".to_string(), ArgValue::Paths(vec![first, second]));
    let (tx, _rx) = mpsc::channel(64);
    let cancel = CancellationToken::new();
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(200)).await;
        cancel_clone.cancel();
    });

    let outcome = tokio::time::timeout(
        Duration::from_secs(10),
        executor.run(ExecutionRequest { descriptor, args }, tx, cancel),
    )
    .await
    .expect("cancellation should resolve well before the command timeout");
    assert!(matches!(outcome, ToolOutcome::Cancelled));
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p wield-core batch_input_converts_each_file_and_reports_a_summary batch_stops_before_the_next_file_once_cancelled`
Expected: FAIL — right now a `Paths` value reaches `render_argv`, which
silently renders it as an unresolved placeholder (`ArgValue::Paths(_) =>
Ok(None)`), so the command runs with a missing/malformed argv rather than
producing a `Report`.

- [ ] **Step 3: Implement the batch path**

In `crates/wield-core/src/executor.rs`, find `run_command`:

```rust
    async fn run_command(
        &self,
        descriptor: &Descriptor,
        command: &crate::descriptor::CommandSpec,
        effective: &ArgMap,
        progress: mpsc::Sender<Progress>,
        cancel: CancellationToken,
    ) -> ToolOutcome {
        let output_path = match compute_output_path(&descriptor.output, effective) {
```

Insert immediately after the opening `{` (before `let output_path = ...`):

```rust
        if let Some((arg_name, paths)) = batch_paths(effective) {
            return self
                .run_batch(descriptor, command, effective, arg_name, &paths, progress, cancel)
                .await;
        }

```

Then add two new items to the same `impl Executor` block (anywhere after
`run_command`, e.g. right after it, before the closing `}` of `impl
Executor`):

```rust
    async fn run_batch(
        &self,
        descriptor: &Descriptor,
        command: &crate::descriptor::CommandSpec,
        effective: &ArgMap,
        arg_name: &str,
        paths: &[std::path::PathBuf],
        progress: mpsc::Sender<Progress>,
        cancel: CancellationToken,
    ) -> ToolOutcome {
        let mut lines = Vec::with_capacity(paths.len());
        let mut succeeded = 0usize;
        for (index, path) in paths.iter().enumerate() {
            if cancel.is_cancelled() {
                return ToolOutcome::Cancelled;
            }
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.to_string_lossy().into_owned());
            let _ = progress.try_send(Progress::Message(format!(
                "Converting {} of {}: {name}",
                index + 1,
                paths.len()
            )));
            let mut single = effective.clone();
            single.insert(arg_name.to_owned(), ArgValue::Path(path.clone()));
            // `run_command`'s new batch check above only matches when
            // `effective` itself holds a `Paths` value - `single` never
            // does, so this recursive call takes the normal single-file
            // path, not another batch iteration.
            let outcome = Box::pin(self.run_command(
                descriptor,
                command,
                &single,
                progress.clone(),
                cancel.clone(),
            ))
            .await;
            match outcome {
                ToolOutcome::File { path } => {
                    succeeded += 1;
                    lines.push(format!("\u{2713} {name} \u{2192} {}", path.display()));
                }
                ToolOutcome::Cancelled => return ToolOutcome::Cancelled,
                other => lines.push(format!("\u{2717} {name}: {}", outcome_summary(&other))),
            }
        }
        ToolOutcome::Report {
            title: format!("{succeeded} of {} converted", paths.len()),
            lines,
        }
    }
```

```rust
fn batch_paths(effective: &ArgMap) -> Option<(&str, Vec<std::path::PathBuf>)> {
    effective.iter().find_map(|(name, value)| match value {
        ArgValue::Paths(paths) => Some((name.as_str(), paths.clone())),
        _ => None,
    })
}

fn outcome_summary(outcome: &ToolOutcome) -> String {
    match outcome {
        ToolOutcome::Failed { detail, .. } => detail.clone(),
        ToolOutcome::Unavailable { reason, .. } => reason.clone(),
        other => format!("{other:?}"),
    }
}
```

(`ToolOutcome` has no `OutputMissing`/`NonZeroExit`/`Timeout`/`SpawnFailed`
variant of its own — those are `CommandResult` variants that `run_command`'s
existing match already maps to `ToolOutcome::Failed` before `run_batch` ever
sees the result, so `Failed`/`Unavailable`/the catch-all `other` cover every
case `outcome_summary` actually receives.)

Both `batch_paths` and `outcome_summary` are free functions at module scope
in `executor.rs` (same level as the existing `failed`/`unavailable_binary`/
`join_errors` helpers at the bottom of the file) - not methods on `Executor`.

`Box::pin(...)` around the recursive `run_command` call is required: Rust
doesn't allow an `async fn` to call itself directly (the future type would be
infinitely sized) - `Box::pin` breaks the cycle by heap-allocating that one
frame. This is standard, not a workaround; every recursive async fn in Rust
needs it.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p wield-core batch_input_converts_each_file_and_reports_a_summary batch_stops_before_the_next_file_once_cancelled`
Expected: PASS.

- [ ] **Step 5: Run the full existing executor test suite to confirm no regression**

Run: `cargo test -p wield-core --test executor_pipeline`
Expected: all pass, including the pre-existing single-file tests (`run_command`'s new batch check is a no-op for any `ArgMap` with no `Paths` value, which is every existing test).

- [ ] **Step 6: Run the full gate and commit**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace -- --test-threads=1`
Expected: all clean.

```bash
git add crates/wield-core/src/executor.rs crates/wield-core/tests/executor_pipeline.rs
git commit -m "feat(core): batch-convert a multi-file arg through the existing single-file path"
```

---

### Task 3: `coerce_json_args` — the actual missing link for multi-file selection

**Files:**
- Modify: `apps/wield/src-tauri/src/commands.rs`

**Interfaces:**
- Consumes: `wield_core::ArgValue::Paths` (already exists, exercised end-to-end by Task 2).
- Produces: nothing a later task calls by name - this closes the gap between a multi-select file picker's JSON array and `ArgValue::Paths` reaching `Executor::run`. Task 4's `video.convert` (once registered) becomes reachable through this path automatically.

- [ ] **Step 1: Write the failing test**

Add to `apps/wield/src-tauri/src/commands.rs`'s existing `mod tests` block
(anywhere after the existing `run_tool_rejects_unknown_arguments` test - the
module already has everything this test needs in scope):

```rust
    #[tokio::test]
    async fn coerce_json_args_accepts_an_array_for_a_multi_file_arg() {
        let mut descriptor = wield_core::DescriptorBuilder::new(
            "test.batch",
            "Test batch",
            wield_core::Category::Convert,
        )
        .arg(
            wield_core::ArgSpecBuilder::new(
                "input",
                "Input",
                wield_core::ArgType::File {
                    filters: vec![],
                    multiple: true,
                },
            )
            .required(true)
            .build(),
        )
        .requires(wield_core::Requires::None)
        .output(wield_core::OutputSpec::Report)
        .command(wield_core::CommandSpecBuilder::new("true"))
        .build()
        .expect("descriptor should be valid");
        descriptor.id = wield_core::ToolId::parse("test.batch").unwrap();

        let args = serde_json::json!({ "input": ["/a.mp4", "/b.mp4"] });
        let result = coerce_json_args(&descriptor, &args).expect("array should coerce");
        assert_eq!(
            result.get("input"),
            Some(&ArgValue::Paths(vec!["/a.mp4".into(), "/b.mp4".into()]))
        );
    }

    #[tokio::test]
    async fn coerce_json_args_rejects_an_array_for_a_single_file_arg() {
        let state = AppState::build().await;
        let descriptor = state.registry.get("image.convert").unwrap();
        let args = serde_json::json!({ "input": ["/a.png"], "format": "png" });
        assert!(coerce_json_args(descriptor, &args).is_err());
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p wield-app coerce_json_args_accepts_an_array_for_a_multi_file_arg`
Expected: FAIL — `value.as_str()` returns `None` for a JSON array, so
`coerce_json_args` currently returns `Err("input must be a path string")`.

- [ ] **Step 3: Implement the new branch**

In `apps/wield/src-tauri/src/commands.rs`, find:

```rust
        let value = match &spec.arg_type {
            ArgType::File { .. } | ArgType::Dir => value
                .as_str()
                .map(|value| ArgValue::Path(value.into()))
                .ok_or_else(|| format!("{name} must be a path string"))?,
```

Replace with:

```rust
        let value = match &spec.arg_type {
            ArgType::File { multiple: true, .. } => {
                let items = value
                    .as_array()
                    .ok_or_else(|| format!("{name} must be an array of path strings"))?;
                let paths = items
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .map(std::path::PathBuf::from)
                            .ok_or_else(|| format!("{name} must be an array of path strings"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                ArgValue::Paths(paths)
            }
            ArgType::File { multiple: false, .. } | ArgType::Dir => value
                .as_str()
                .map(|value| ArgValue::Path(value.into()))
                .ok_or_else(|| format!("{name} must be a path string"))?,
```

(`ArgType::Dir` never has `multiple`, so it stays paired with the
`multiple: false` arm — a `Dir` value passed to this match still only ever
needs the single-path branch.)

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p wield-app coerce_json_args_accepts_an_array_for_a_multi_file_arg coerce_json_args_rejects_an_array_for_a_single_file_arg`
Expected: PASS.

- [ ] **Step 5: Run the full existing commands test suite to confirm no regression**

Run: `cargo test -p wield-app`
Expected: all pass.

- [ ] **Step 6: Run the full gate and commit**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace -- --test-threads=1`
Expected: all clean.

```bash
git add apps/wield/src-tauri/src/commands.rs
git commit -m "feat(app): coerce a multi-file arg's JSON array into ArgValue::Paths"
```

---

### Task 4: `video.convert` descriptor

**Files:**
- Create: `crates/wield-tools/src/video_convert.rs`
- Modify: `crates/wield-tools/src/lib.rs`
- Modify: `crates/wield-tools/tests/builtin_registry.rs`
- Modify: `crates/wield-tools/tests/snapshots/builtin_registry.json` (regenerated, not hand-edited — see Step 5)

**Interfaces:**
- Consumes: `ProgressSpec::FfmpegDuration` (Task 1), `CommandSpecBuilder::progress()` (Task 1), the batch path reached automatically via `multiple: true` (Task 2), the `coerce_json_args` array branch (Task 3) — this task is what actually exercises all three for the first time as one real tool.
- Produces: `wield_tools::video_convert::descriptor() -> wield_core::Descriptor`, registered in `wield_tools::builtin_registry()` as `"video.convert"`. Nothing later in this plan consumes it by name — this is the user-facing deliverable Task 5 verifies live.

- [ ] **Step 1: Write the descriptor**

Create `crates/wield-tools/src/video_convert.rs`:

```rust
//! The `video.convert` built-in — format / resolution / quality via ffmpeg.

use std::time::Duration;
use wield_core::{
    ArgSpecBuilder, ArgType, ArgValueLiteral, Category, CommandSpecBuilder, Descriptor,
    DescriptorBuilder, FileFilter, OutputDir, OutputSpec, ProgressSpec, Requires,
};

const VIDEO_EXTS: &[&str] = &["mp4", "mov", "mkv", "webm", "avi"];
const FORMATS: &[&str] = &["mp4", "webm", "mkv"];
const RESOLUTIONS: &[&str] = &["original", "1080p", "720p", "480p"];

/// Descriptor for `video.convert`. Runs `ffmpeg -y -i <input> [-vf
/// scale=-2:H] [-crf N] -progress pipe:2 -nostats <output>`. `resolution`
/// and `quality` are optional - absent options drop from argv, matching
/// `image.convert`'s established pattern. `input` accepts multiple files
/// (`multiple: true`) - `wield-core`'s executor converts each one through
/// the same single-file path and reports a summary; see
/// `docs/superpowers/specs/2026-09-14-wield-m2a-video-convert-design.md`.
pub fn descriptor() -> Descriptor {
    DescriptorBuilder::new("video.convert", "Convert video", Category::Convert)
        .keywords(&[
            "video", "convert", "ffmpeg", "mp4", "webm", "mkv", "resolution", "format",
        ])
        .arg(
            ArgSpecBuilder::new(
                "input",
                "Video",
                ArgType::File {
                    filters: vec![FileFilter {
                        label: "Videos".into(),
                        extensions: VIDEO_EXTS.iter().map(|ext| (*ext).to_owned()).collect(),
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
            .default(ArgValueLiteral::Str("mp4".into()))
            .build(),
        )
        .arg(
            ArgSpecBuilder::new(
                "resolution",
                "Resolution",
                ArgType::Enum {
                    options: RESOLUTIONS.iter().map(|r| (*r).to_owned()).collect(),
                },
            )
            .required(true)
            .default(ArgValueLiteral::Str("original".into()))
            .help("\"original\" keeps the source resolution unchanged.")
            .build(),
        )
        .arg(
            ArgSpecBuilder::new(
                "quality",
                "Quality (CRF)",
                ArgType::Int {
                    range: Some([0, 51]),
                    step: Some(1),
                },
            )
            .help(
                "Lower is higher quality (libx264's CRF scale - the opposite of most \"quality\" sliders). Leave blank for ffmpeg's own default.",
            )
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
                .arg_when("-vf", "resolution", vec![ArgValueLiteral::Str("1080p".into())])
                .arg_when(
                    "scale=-2:1080",
                    "resolution",
                    vec![ArgValueLiteral::Str("1080p".into())],
                )
                .arg_when("-vf", "resolution", vec![ArgValueLiteral::Str("720p".into())])
                .arg_when(
                    "scale=-2:720",
                    "resolution",
                    vec![ArgValueLiteral::Str("720p".into())],
                )
                .arg_when("-vf", "resolution", vec![ArgValueLiteral::Str("480p".into())])
                .arg_when(
                    "scale=-2:480",
                    "resolution",
                    vec![ArgValueLiteral::Str("480p".into())],
                )
                .arg_when_set("-crf", "quality")
                .arg_when_set("{quality}", "quality")
                .arg("-progress")
                .arg("pipe:2")
                .arg("-nostats")
                .arg("{output}")
                .progress(ProgressSpec::FfmpegDuration)
                .timeout(Duration::from_secs(1800)),
        )
        .build()
        .expect("video.convert descriptor is valid")
}
```

The three `arg_when` pairs above (one per resolution value) are the
mechanism: `arg_when` (not `arg_when_set`) gates a segment on a *specific*
value, which is exactly "only emit `-vf scale=-2:1080` when `resolution` is
`1080p` specifically" — not merely "when `resolution` is set at all" (it's
always set, since it has a default of `"original"`). Traced through
`render_argv`'s actual matching logic (read directly, not assumed): for any
given `resolution` value, at most one of the three `-vf`/`scale=...` pairs
has its `when` condition satisfied, so exactly zero or two segments get
emitted, never a mismatched pair.

- [ ] **Step 2: Register it in the built-in registry**

In `crates/wield-tools/src/lib.rs`, find:

```rust
pub mod color_pick;
pub mod image_convert;
```

Add a third line, alphabetically:

```rust
pub mod color_pick;
pub mod image_convert;
pub mod video_convert;
```

Then find:

```rust
    registry
        .register(image_convert::descriptor())
        .expect("image.convert registers");
    registry
```

Replace with:

```rust
    registry
        .register(image_convert::descriptor())
        .expect("image.convert registers");
    registry
        .register(video_convert::descriptor())
        .expect("video.convert registers");
    registry
```

- [ ] **Step 3: Update the registry-contents test**

In `crates/wield-tools/tests/builtin_registry.rs`, find:

```rust
#[test]
fn registry_has_exactly_the_expected_builtins() {
    let ids: Vec<_> = builtin_registry()
        .list()
        .iter()
        .map(|d| d.id.as_ref().to_string())
        .collect();
    assert_eq!(ids, vec!["color.pick", "image.convert"]);
}
```

Replace the `assert_eq!` line's expected vector:

```rust
    assert_eq!(ids, vec!["color.pick", "image.convert", "video.convert"]);
```

- [ ] **Step 4: Write an argv-rendering test, mirroring `image_convert`'s exactly**

Add to the same file, after the existing `image_convert_renders_magick_argv_for_present_and_absent_options` test:

```rust
#[test]
fn video_convert_renders_ffmpeg_argv_for_present_and_absent_options() {
    let tool = builtin_registry().get("video.convert").unwrap().clone();
    let Capability::Command(spec) = &tool.capability else {
        panic!("expected Command");
    };

    let mut minimal = BTreeMap::new();
    minimal.insert("input".to_string(), ArgValue::Path("/clips/a.mov".into()));
    minimal.insert("format".to_string(), ArgValue::Str("mp4".into()));
    minimal.insert("resolution".to_string(), ArgValue::Str("original".into()));
    let out = std::path::PathBuf::from("/clips/a.mp4");
    assert_eq!(
        render_argv(&spec.args, &minimal, Some(&out)).unwrap(),
        vec![
            "-y".to_string(),
            "-i".into(),
            "/clips/a.mov".into(),
            "-progress".into(),
            "pipe:2".into(),
            "-nostats".into(),
            "/clips/a.mp4".into(),
        ],
    );

    let mut full = minimal.clone();
    full.insert("resolution".to_string(), ArgValue::Str("720p".into()));
    full.insert("quality".to_string(), ArgValue::Int(23));
    assert_eq!(
        render_argv(&spec.args, &full, Some(&out)).unwrap(),
        vec![
            "-y".to_string(),
            "-i".into(),
            "/clips/a.mov".into(),
            "-vf".into(),
            "scale=-2:720".into(),
            "-crf".into(),
            "23".into(),
            "-progress".into(),
            "pipe:2".into(),
            "-nostats".into(),
            "/clips/a.mp4".into(),
        ],
    );
}
```

- [ ] **Step 5: Run the tests, regenerate the snapshot**

Run: `cargo test -p wield-tools`
Expected: `builtin_registry_matches_snapshot` FAILS (the registry now has a
third tool the committed snapshot doesn't know about) — every other test in
this step should pass.

Run: `UPDATE_SNAPSHOTS=1 cargo test -p wield-tools`
Expected: rewrites `crates/wield-tools/tests/snapshots/builtin_registry.json`.

Run: `cargo test -p wield-tools`
Expected: all pass, including `builtin_registry_matches_snapshot` now that
the snapshot reflects the new tool.

- [ ] **Step 6: Review the regenerated snapshot diff before committing**

Run: `git diff crates/wield-tools/tests/snapshots/builtin_registry.json`
Expected: only additions — a new `"video.convert"` object appended, `"color.pick"`
and `"image.convert"`'s existing entries byte-for-byte unchanged. If anything
about `image.convert`'s entry changed, stop and investigate before
proceeding - this task should not touch it.

- [ ] **Step 7: Run the full gate and commit**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace -- --test-threads=1 && npm run check`
Expected: all clean.

```bash
git add crates/wield-tools/src/video_convert.rs crates/wield-tools/src/lib.rs crates/wield-tools/tests/builtin_registry.rs crates/wield-tools/tests/snapshots/builtin_registry.json
git commit -m "feat(tools): add video.convert (ffmpeg, real progress, multi-file batch)"
```

---

### Task 5: Live verification and `docs/testing.md`

**Files:**
- Modify: `docs/testing.md`

No new code in this task — installing the build and verifying the actual
end-to-end behavior live, the only way this class of feature (a real
subprocess, real progress timing, real multi-file selection through the
native file picker) can be checked, matching this project's established
convention (every prior milestone's Command/Portal tools were verified this
same way).

- [ ] **Step 1: Confirm `ffmpeg` is on this machine (it already is, per the design spec's own live verification)**

Run: `which ffmpeg ffprobe`
Expected: both resolve (already confirmed present, `n9.0`, during this
plan's own design phase — re-confirming here is just making sure nothing
changed since).

- [ ] **Step 2: Full build and install**

```bash
npm run build -w apps/wield -- --no-bundle
pkill -x wield-app || true
cp target/release/wield-app ~/.local/bin/wield-app
nohup ~/.local/bin/wield-app > /tmp/wield-m2a-test.log 2>&1 &
disown
```

- [ ] **Step 3: Single-file conversion, verify live progress**

Show the palette, search "convert video", select `video.convert`, pick one
short real video file (or generate one: `ffmpeg -f lavfi -i
testsrc=duration=8:size=640x480:rate=25 -y /tmp/m2a-test-1.mp4`), pick a
target format, run it. Confirm: the percentage bar actually animates (not
stuck at 0% or jumping straight to 100%), the result card shows the
converted file's path, and the converted file plays / has a nonzero size
matching a real conversion (not the input just copied).

- [ ] **Step 4: Multi-file batch, verify Report card and per-file naming**

Generate 2-3 more short test clips (`ffmpeg -f lavfi -i
testsrc=duration=3:size=320x240:rate=25 -y /tmp/m2a-test-2.mp4`, `.../m2a-test-3.mp4`),
run `video.convert` again with all files selected in the picker at once
(multi-select). Confirm: the running view's message narrates "Converting N
of M: filename" and updates between files, the final Report card lists all
files with a per-file success line, and each output file actually exists
next to its input with the expected `{stem}.{format}` name.

- [ ] **Step 5: Cancellation mid-batch**

Start a batch of at least 3 files, press Escape partway through (while the
second or later file is converting). Confirm: the run stops promptly (not
hanging until the whole batch or the 30-minute timeout), the palette returns
to search, and no stray output files are left for whichever file was
in-flight at the moment of cancellation (the existing `OutputPlan`
temp-file-then-atomic-rename mechanism should already guarantee this, this
step is confirming it, not implementing anything new).

- [ ] **Step 6: `Unavailable` path**

Temporarily rename `/usr/bin/ffmpeg` out of the way (or use a scoped
`BinaryResolver` if easier via a debug build - whichever is faster to set up
live), show the palette, confirm `video.convert` renders as unavailable with
a reason naming `ffmpeg`, matching `image.convert`'s existing `Unavailable`
presentation. Restore `ffmpeg` afterward.

- [ ] **Step 7: Write up the results in `docs/testing.md`**

Add a new section following this project's existing convention (what was
tried, what was confirmed, any surprises or false leads along the way — not
just "it works"). Include the exact verification steps and outcomes from
Steps 3-6 above, and note the `ffmpeg` version this was verified against
(`n9.0`).

- [ ] **Step 8: Final full verification pass**

Run, from the repo root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
npm run check
```

Expected: all clean.

- [ ] **Step 9: Commit**

```bash
git add docs/testing.md
git commit -m "docs(testing): verify video.convert's progress and batch conversion live"
```

---

## After this plan

Push the branch and open a PR against `master` (PR description: summarize
the feature, link the spec, note live verification results — matching every
prior PR in this project's own established pattern) — **do not merge
without the project owner's explicit go-ahead**, matching how every previous
PR in this project has been handled.

M2b (`audio.extract`) follows the same shape as Task 4 here almost entirely
— a new descriptor reusing `ProgressSpec::FfmpegDuration` and `multiple:
true` as-is, no further `wield-core` changes expected. M2c
(`document.convert`, pandoc + libreoffice fallback) and M2d (`pdf.tools`,
qpdf/ghostscript) are separate, later plans — each backing binary needs its
own progress-format research the same way this plan researched ffmpeg's,
not assumed to be identical.
