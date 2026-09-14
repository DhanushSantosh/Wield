# Wield — M2b: `audio.extract` Design

## 1. Why this exists

M2a shipped the two mechanisms M2 needed as reusable infrastructure: real
subprocess progress (`ProgressSpec`/`ProgressParser`, with `FfmpegDuration`
as the first concrete parser) and multi-file batch conversion
(`Executor::run_batch`, generic over any `ArgValue::Paths` argument). M2b
is the first test of that reuse claim: `audio.extract`, per the design
spec's own milestone table (`docs/superpowers/specs/2026-09-10-wield-design.md`
§6) — "extract / convert audio track", backed by ffmpeg like `video.convert`.

Concretely: pull the audio track out of a video file (the common "extract
audio" case), or transcode between audio formats (a video file isn't
required — an audio file converting to another audio format is the same
operation to ffmpeg, since `-vn` is a no-op when there's no video stream to
begin with). One descriptor covers both, verified live rather than assumed
(§2).

This branch (`feat/m2b-audio-extract`) is stacked on top of the
still-unmerged `feat/m2a-video-convert` (PR #13) rather than forked from
master — per the new project convention (2026-09-14): new implementation
work stacks on the current unmerged branch instead of waiting for a merge,
with PRs opened base-to-base and GitHub auto-retargeting once the base
merges and its branch is deleted.

## 2. What's already there, verified against the actual code and a live run

Nothing new needs to be built at the mechanism level. Confirmed by reading
the actual M2a code (not re-derived from memory) and by running real
ffmpeg (`n9.0.1`) against a synthetic test video and a synthetic test audio
file (both generated via `ffmpeg -f lavfi`, no external assets needed):

- **`FfmpegDuration`** (`crates/wield-core/src/command.rs`) needs zero
  changes. It parses the `Duration: HH:MM:SS.ss, start: ..., bitrate: ...`
  banner once and then `out_time_us=` from each `-progress pipe:2` block —
  confirmed live that this exact banner and progress-block shape appears
  identically for an audio-only `ffmpeg -vn ...` run, whether the input has
  a video stream (`source.mp4`, extracting to `.mp3`) or is itself
  audio-only (`audio-only.wav`, transcoding to `.flac`). No audio-specific
  quirk was found.
- **`Executor::run_batch`/`batch_paths`** (`crates/wield-core/src/executor.rs`)
  needs zero changes. Any descriptor with one `ArgType::File { multiple:
  true, .. }` arg gets batch support automatically — this is exactly what
  M2a built it to do.
- **The descriptor pattern** (`crates/wield-tools/src/video_convert.rs`) is
  the direct template: `arg_when`-gated command segments for format-
  dependent options, `Requires::Binary`, `OutputSpec::File` with
  `"{input_stem}.{format}"` naming and `OutputDir::SameAsInput`,
  `.progress(ProgressSpec::FfmpegDuration)`.
- **The frontend** (`RunningView`, `ArgForm`'s `when`-based field
  visibility, the native multi-select file picker) needs zero changes —
  all of it is already generic over any descriptor's `when`/`multiple`
  shape, confirmed by M2a's own live verification and unchanged since.

The only genuinely new work is the `audio.extract` descriptor itself and
verifying the exact ffmpeg argv shape for audio-only output, which had not
been exercised by anything in this codebase before.

## 3. ffmpeg argv for audio extraction/conversion, verified live

Tested directly (not assumed from ffmpeg documentation alone):

```
ffmpeg -y -i <input> -vn -codec:a <codec> [-b:a <bitrate>] out.<ext>
```

- `-vn` ("no video") is safe on every input tried, including one with no
  video stream at all — it's simply a no-op there, confirmed by exit 0 and
  a correct audio-only output on `audio-only.wav -> out.flac`.
- No explicit `-map` is needed: ffmpeg selects the input's audio stream on
  its own once `-vn` rules out video. Multi-audio-track selection (e.g. a
  video with several language tracks) is out of scope — YAGNI, matches
  this project's existing bias toward minimal exposed surface area (e.g.
  `video.convert` doesn't expose stream mapping either).
- Codec-per-format, all confirmed working end-to-end (`ffprobe`-verified
  output, correct duration, exit 0). **`format`'s enum value is the output
  *extension*, not the ffmpeg codec name** — they differ for one entry:
  - `mp3` → `-codec:a libmp3lame`
  - `m4a` → `-codec:a aac` (a literal `.aac` output extension was tried
    first and rejected: it makes ffmpeg select the ADTS muxer, which
    reports a measurably imprecise duration — `4.950561` instead of the
    exact `5.0` a `.m4a`/MP4-family container reports for an identical
    5s source, confirmed via `ffprobe`. `.m4a` is also the far more
    broadly compatible container for AAC audio in practice.)
  - `flac` → `-codec:a flac`
  - `wav` → `-codec:a pcm_s16le`
- The output container is inferred entirely from the output file's
  extension, same as `video.convert`'s temp-name-suffix mechanism
  (`OutputPlan::for_final` preserves the final extension on the temp path)
  — no `-f <format>` flag needed.
- **Speed**: audio extraction is dramatically faster than video
  re-encoding — observed 28x-547x realtime across the formats tested on a
  5s source. `video.convert`'s 1800s (30 min) timeout, sized for video
  re-encoding, would be wildly oversized here; **300s (5 min)** is the
  chosen default — generous even for an hour-long podcast or album side,
  while still bounding a genuinely stuck process.

## 4. `audio.extract` descriptor

`crates/wield-tools/src/audio_extract.rs`, registered in
`crates/wield-tools/src/lib.rs` (alphabetically after `audio_extract`,
before `color_pick`) and `builtin_registry()`.

- **`input`** — `File { multiple: true, filters: [mp4, mov, mkv, webm,
  avi, mp3, wav, flac, m4a, aac] }`, required. Both video-container and
  audio-file extensions are accepted, since either is a valid input to
  this tool (§1). Batch (`multiple: true`) is included from the start —
  it's now a zero-marginal-cost default for any Command-capability tool,
  not a scope decision to revisit per tool the way it was for M2a.
- **`format`** — `Enum { options: [mp3, m4a, flac, wav] }`, default `mp3`.
  **Corrected from an initial draft of this spec that used `aac` as the
  option** (verified live, after writing that draft, that this project's
  `"{input_stem}.{format}"` output-naming template feeds the enum's raw
  string directly into the output file's extension — the same mechanism
  `video.convert`'s `format` options already rely on. A literal `.aac`
  extension makes ffmpeg select the ADTS muxer, which is not only a less
  broadly compatible container than MP4/`.m4a` but measurably lossier on
  metadata: a live test produced `duration=4.950561` instead of the exact
  `5.0` `.m4a` reports for an identical 5s source. `format`'s enum value
  must be the desired *extension* (`m4a`), independent of the ffmpeg
  *codec* name (`aac`) used to produce it — those are two different
  strings and the command spec keeps them separate (§4).
- **`quality`** — optional `Enum { options: ["128k", "192k", "256k",
  "320k"] }`, gated via `arg_when` to `format in [mp3, m4a]` (flac and wav
  are lossless — a bitrate knob on them is meaningless, so the field
  doesn't apply and `ArgForm`'s existing `when`-based visibility hides it
  automatically, same mechanism `video.convert`'s resolution-gated
  `-vf`/`-crf` segments already rely on). **Deliberately an `Enum` of
  presets, not a raw `Int` range like `video.convert`'s CRF** — the M2a
  final review flagged a real footgun in that shape (an emptied numeric
  field silently coerces to `0` via `FormField.tsx`'s `Number("")===0`
  behavior, and `0` happened to be a meaningful, extreme value on that
  range). A closed set of bitrate strings has no empty-field failure mode:
  there's nothing to accidentally clear to a dangerous default.
- **`Requires::Binary("ffmpeg")`** — already a project dependency (M2a).
  No new packaging work; no new `docs/backlog.md` entry needed.
- **Command** (`Capability::Command`):
  ```
  ffmpeg -y -i {input} -vn
    -codec:a libmp3lame   (arg_when format in [mp3])
    -codec:a aac          (arg_when format in [m4a])
    -codec:a flac         (arg_when format in [flac])
    -codec:a pcm_s16le    (arg_when format in [wav])
    -b:a {quality}         (arg_when_set quality, presence form — only
                             emitted when quality is actually set, which
                             per the arg's own `when` can only happen for
                             mp3/m4a anyway)
    -progress pipe:2 -nostats
    {output}
  ```
  `.progress(ProgressSpec::FfmpegDuration)`, `.timeout(Duration::from_secs(300))`,
  `SuccessSpec::ExitZero` — same shape as `video.convert`'s command spec.
- **Output** — `OutputSpec::File { name: "{input_stem}.{format}", dir:
  OutputDir::SameAsInput }` — verbatim reuse of `video.convert`'s pattern.
  Same pre-existing, non-defect behavior applies: converting into the same
  format as the input (e.g. `mp3 -> mp3`) overwrites the source in place,
  inherited from `image.convert`/`video.convert`, not new here.

No new `ProgressSpec` variant, no new `ArgType`, no `wield-core` changes of
any kind — everything this descriptor needs already exists.

## 5. Risks and honest gaps

- **Multi-track audio inputs.** A video with multiple audio tracks (e.g.
  dubbed language tracks) will get whichever track ffmpeg's own default
  stream selection picks — not user-selectable. Scoped out deliberately
  (§3); revisit only if a real need surfaces.
- **A real, previously-latent frontend gap, found by this verification
  pass, not by inspection.** `quality` is the first `ArgSpec` in this
  codebase to use `.when(...)` (frontend-visibility gating) on a
  Command-tool's optional field — `video.convert`'s `resolution`/`quality`
  are both always-visible, so this interaction has never been exercised
  end-to-end before. Reading `ArgForm.tsx` directly: its `submit` handler
  calls `onSubmit(values)` with the *entire* form-state object, not
  `visible`-filtered — so if a user sets `quality` while `format=mp3`,
  then switches `format` to `flac` (hiding the now-inapplicable `quality`
  field from the UI), the stale value is never cleared from state and
  *is* still submitted. Verified live that ffmpeg tolerates this
  gracefully in every case tried (`-b:a` alongside `-codec:a flac` bumps
  the encoder to 24-bit output but stays lossless, exit 0; alongside
  `-codec:a pcm_s16le` it's silently ignored, exit 0) — not a crash, not
  data loss, but a real correctness gap (a value the user can no longer
  see in the UI still reaches the command line) that this milestone's own
  design is what newly exposes, since no prior descriptor's `.when(...)`
  usage ever put a stale value at risk of being consequential. Given how
  small and directly-caused-by-this-work the fix is, the plan includes it
  as its own task (matching this project's established precedent, M2a's
  `validate.rs` fix, of correcting a real gap found while building
  directly on top of the code that exposes it) rather than leaving it as
  a documented risk to work around.
- **No Flatpak bundling**, same as every other Command-backed converter
  tool (`image.convert`, `video.convert`) — already tracked in
  `docs/backlog.md`, no new entry needed since `ffmpeg` is already named
  there.
- **300s timeout is a judgment call**, not derived from an exhaustive
  worst-case (e.g. a very long, slow-codec batch item). Generous relative
  to every real measurement taken (§3); revisit only if real use proves it
  too tight.

## 6. Testing strategy

`ArgForm.tsx`'s fix gets its own regression test first (a `when`-gated
field is set, its trigger arg changes to hide it, `onSubmit`'s payload is
asserted to exclude it) — this is what makes `quality`'s `.when(...)` gate
actually safe to ship. Then the descriptor itself: `render_argv` unit
tests for the gated command segments (minimal case — no quality set; full
case — `format=mp3` + `quality="256k"`; one case per remaining format to
confirm the right `-codec:a` branch fires and no other branch does), a
builtin-registry snapshot update, and live verification on the real
desktop (single-file extraction from a real video, single audio-to-audio
conversion, and a multi-file batch) with `ffprobe`-independent
confirmation of each output, written up in `docs/testing.md`.

## 7. Repo artifacts

- This spec: `docs/superpowers/specs/2026-09-14-wield-m2b-audio-extract-design.md`
- Plan (next): `docs/superpowers/plans/2026-09-14-wield-m2b-audio-extract.md`
- Modified: `apps/wield/src/palette/ArgForm.tsx` (and its test file) — the
  stale-hidden-value fix (§5), landed first since the descriptor task
  relies on it.
- New: `crates/wield-tools/src/audio_extract.rs`
- Modified: `crates/wield-tools/src/lib.rs`,
  `crates/wield-tools/tests/builtin_registry.rs`,
  `crates/wield-tools/tests/snapshots/builtin_registry.json`, and the same
  mechanical registration-count ripples M2a's `video.convert` addition hit
  (`apps/wield/src-tauri/src/commands.rs`'s registration-order test,
  `apps/wield/src-tauri/tests/commands.rs`'s two tool-count assertions —
  `3` becomes `4`).
