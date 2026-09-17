# Wield — M3a: `screen.ocr` + Real `Native` Capability Infrastructure Design

## 1. Why this exists

M1 and M2 built out the `Command` and `Portal` capability paths in full —
every tool shipped so far is one or the other. `Capability::Native` has
existed in `wield-core`'s type model since the original P2-era plan
(`descriptor.rs`'s `Capability::Native { id: NativeId }`, `builder.rs`'s
`DescriptorBuilder::native(id)`) but has never been backed by real execution:
`Executor::run`'s `Native` match arm is a hardcoded stub
(`ToolOutcome::Failed { detail: "native execution is implemented in P4" }`,
`executor.rs:147`) left over from that early plan.

M3 (Capture, per `docs/superpowers/specs/2026-09-10-wield-design.md` §6) is
`screen.ocr` (`Native`) + `keep.awake` (`Portal`, via `Inhibit`). This is
M3a: `screen.ocr` only, chosen to go first because it's the piece that
actually builds real `Native` capability execution for the first time —
infrastructure M4's `launcher.doctor` (also `Native`) will need later too.
`keep.awake` is a standard `Portal` capability, the same architectural shape
as `color.pick`, and is deferred to M3b.

Screen recorder (ScreenCast + PipeWire) is explicitly out of scope for M3
per the spec — "its own effort."

## 2. What's already there, verified against the actual code

As with M2a, most of what this needs already exists as a real, unused
extension point — not something to build from scratch:

- **`Capability::Native { id: NativeId(String) }`** and
  **`DescriptorBuilder::native(id)`** (`descriptor.rs:170`, `builder.rs:224`)
  already exist. Only the executor-side dispatch is missing.
- **`Capability::Portal`'s dispatch shape is the direct precedent to mirror**:
  a `PortalRunner` trait (`portal.rs`) with one method,
  `async fn run(&self, adapter: &str, args: &ArgMap, cancel: CancellationToken)
  -> ToolOutcome`, injected into `Executor` via `.with_portal(Arc<dyn
  PortalRunner>)` and wired up in `apps/wield/src-tauri/src/state.rs:58`.
  The concrete impl (`wield_portal::PortalAdapterRunner`) just forwards to a
  `match adapter { "screenshot.pick_color" => ..., other => Failed }` in
  `wield-portal/src/adapters/mod.rs`. `Native` needs the exact same shape.
- **`CommandRunner::execute(RunSpec)`** (`command.rs:182`) is already fully
  decoupled from `Descriptor` — it takes a plain `binary: &Path`, `argv: &[String]`,
  `cwd`, `output: Option<&OutputPlan>`, `progress_spec`, `success`, `timeout`,
  a `Progress` sender, and a `CancellationToken`. The whole `command` module
  is `pub`. `screen.ocr`'s Rust code can call this directly to run `tesseract`
  as a subprocess — no new subprocess-spawning mechanism needed.
- **The Screenshot portal call for capture is a different method on the same
  portal `color.pick` already uses**, not a new portal. Verified against the
  vendored `ashpd 0.13.13` source
  (`~/.cargo/registry/.../ashpd-0.13.13/src/desktop/screenshot.rs`):
  `ashpd::desktop::Screenshot::request().interactive(true).modal(true).send()`
  returns a `Request<Screenshot>` whose response exposes `.uri()` — a
  `file://` URI to a saved screenshot image. With `.interactive(true)`, the
  compositor's own screenshot UI handles region selection — matching the
  original spec's "OCR region selection is compositor-provided via the
  interactive Screenshot portal" exactly. No new client-side selection UI.
- **`ValueKind::Text`** (`descriptor.rs`) already exists, unused.
  `ResultView.tsx`'s "Copy" button already handles any `ToolOutcome::Value`
  generically (`navigator.clipboard.writeText`) — no new frontend work needed
  to display and copy OCR'd text.
- **`tesseract` is already installed** on the dev machine (verified live:
  `tesseract 5.5.3`, `eng` + `osd` traineddata under `/usr/share/tessdata/`)
  — matches the original spec's "`eng` bundled, more downloadable" baseline
  and `docs/backlog.md`'s Packaging section, which already lists
  Tesseract+`eng` among the not-yet-Flatpak-bundled binaries.
- **`wield-tools` is descriptor-only by established convention** — its
  `Cargo.toml` depends on nothing but `wield-core`; every file in it is a
  pure `Descriptor` builder. `wield-portal` is the sibling crate that holds
  actual portal execution logic. `screen.ocr`'s execution logic needs an
  equivalent home — see §3.1.

## 3. Design

### 3.1 Crate placement: new `wield-native` crate

Three placements were considered:

- **New `wield-native` crate (chosen).** Mirrors `wield-portal`'s structure.
  Depends on `wield-core` + `ashpd` (for the Screenshot portal call) — no
  other new dependencies, since `tesseract` runs through
  `wield_core::command::CommandRunner`. Gives M4's `launcher.doctor` (also
  `Native`, but unrelated to portals or screenshots) a clean, correctly-named
  home later.
- **Fold into `wield-portal`.** Rejected: `wield-portal`'s own doc comment
  scopes it to "the startup capability probe and the XDG Desktop Portal
  adapters." `Native` is a different capability than `Portal`, and
  `launcher.doctor` isn't portal-related at all — a bad precedent to set for
  one tool's convenience.
- **Fold into `wield-tools`.** Rejected: breaks the working, established
  descriptor-only convention for every other file in that crate, and would
  contaminate it with `ashpd` + subprocess-execution dependencies unrelated
  to the rest of its contents.

### 3.2 `NativeRunner` trait (`wield-core`)

New module `wield-core/src/native.rs`, mirroring `portal.rs` exactly:

```rust
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use crate::args::ArgMap;
use crate::outcome::ToolOutcome;

#[async_trait]
pub trait NativeRunner: Send + Sync {
    async fn run(&self, id: &str, args: &ArgMap, cancel: CancellationToken) -> ToolOutcome;
}
```

No `progress` sender parameter — matching `PortalRunner::run`'s existing
signature. A single screenshot-sized `tesseract` call is sub-second; there's
nothing meaningful to report mid-run (the same reasoning that kept
`document.convert`/`pdf.tools` on `ProgressSpec::None` in M2c/d).

`Executor` gains a `native: Option<Arc<dyn NativeRunner>>` field and a
`.with_native(mut self, native: Arc<dyn NativeRunner>) -> Self` builder
method, exact parallel to `with_portal`. `executor.rs:134-152`'s capability
match becomes:

```rust
Capability::Native { id } => match &self.native {
    Some(runner) => runner.run(&id.0, &effective, cancel).await,
    None => ToolOutcome::Failed {
        stage: Stage::Native,
        detail: "this build has no native runner configured".to_owned(),
        hint: None,
    },
},
```

`apps/wield/src-tauri/src/state.rs`'s two `Executor::new(resolver)...` call
sites (`:57` and `:129`) each gain `.with_native(Arc::new(wield_native::NativeToolRunner))`
alongside the existing `.with_portal(...)` call. The third call site
(`:178`, the test/default path) is left as-is — no native runner configured
there is already the tested "unconfigured" case above.

### 3.3 `Requires::All` — a small, real gap this tool exposes

`screen.ocr` needs **two** things available: the Screenshot portal (to
capture) *and* the `tesseract` binary (to recognize). `Requires` today
(`descriptor.rs`) is a single-valued enum — `None` / `Portal{..}` /
`Binary(..)` — because no tool before this one has ever needed more than one
kind of requirement at once.

Left unaddressed, `screen.ocr` would have to pick just one to gate on, and
whichever one it doesn't gate on fails only *after* the user selects and
runs the tool, instead of being greyed out in the palette proactively the
way every other gap-having tool already is (`AvailabilityView::probe_binaries`,
which only scans for `Requires::Binary` today).

Fix: add one new variant, `Requires::All(Vec<Requires>)`.

- `executor.rs`'s pre-flight check (currently one `match`) becomes a small
  recursive helper, `fn check_requires(requires: &Requires, resolver: &BinaryResolver,
  portals: &HashMap<String, u32>) -> Option<ToolOutcome>`, called once for a
  plain `Requires` and once per element for `All` (short-circuiting on the
  first unmet requirement).
- `AvailabilityView::probe_binaries` recurses into `Requires::All(reqs)`,
  collecting every `Binary` entry found at any depth — so composite
  requirements still participate in proactive greying for their binary half.
- `screen.ocr`'s descriptor uses:
  ```rust
  .requires(Requires::All(vec![
      Requires::Portal { iface: "Screenshot".into(), min_ver: 2 },
      Requires::Binary("tesseract".into()),
  ]))
  ```

This is additive only — every existing descriptor's `Requires::None` /
`Portal` / `Binary` value is untouched, and `check_requires`'s non-`All`
branches are exactly the logic already in `executor.rs:105-132`, moved
into a function instead of inlined.

### 3.4 The `screen.ocr` descriptor (`wield-tools/src/screen_ocr.rs`)

```rust
DescriptorBuilder::new("screen.ocr", "Extract text from screen", Category::Capture)
    .keywords(&["ocr", "text", "extract", "screenshot", "recognize"])
    .requires(Requires::All(vec![
        Requires::Portal { iface: "Screenshot".into(), min_ver: 2 },
        Requires::Binary("tesseract".into()),
    ]))
    .output(OutputSpec::Value(ValueKind::Text))
    .native("screen.ocr")
    .build()
    .expect("screen.ocr descriptor is valid")
```

Registered in `builtin_registry()` alongside the other eight built-ins.

### 3.5 The OCR pipeline (`wield-native/src/tools/screen_ocr.rs`)

Mirrors `wield-portal/src/adapters/pick_color.rs`'s shape: a small,
independently-testable inner function plus a thin real-I/O entry point.

1. **Capture.** `ashpd::desktop::Screenshot::request().interactive(true).modal(true).send()`,
   `tokio::select!`'d against the `CancellationToken` exactly like
   `pick_color_with` does. A dismissed portal dialog maps to
   `ToolOutcome::Cancelled`; a transport/protocol failure maps to
   `ToolOutcome::Failed { stage: Stage::Native, .. }` — reusing
   `wield_portal::PortalError`'s existing mapping logic (`wield-native`
   depends on `wield-portal` for this one shared type, or the enum is
   duplicated — see Open Questions).
2. **Resolve the URI to a local path.** The response's `.uri()` is a
   `file://…` URI (ashpd's `Uri` is a plain string newtype — `.as_str()`
   gives the raw URI text). Parsed into a `PathBuf` via the `url` crate
   (already pulled in transitively through `ashpd`/`zbus`).
3. **Run `tesseract` via `CommandRunner::execute`.** `tesseract <path> stdout -l eng`
   — `stdout` as tesseract's special "outputbase" argument makes it print
   recognized text to stdout instead of writing a `.txt` file, so this needs
   no `OutputPlan` (`output: None` in `RunSpec`, the same `None` branch
   `Executor::run_split` already exercises for zero `command.rs` changes).
   Timeout: 30 seconds — generous for one screenshot-sized image, but tight
   enough that a genuine hang surfaces quickly rather than looking stuck.
   Default page-segmentation mode (fully automatic) — no `--psm` override;
   nothing in this milestone's scope calls for tuning it.
4. **Cleanup.** Delete the portal-created screenshot file (the "cleanup"
   step the original spec's mechanism line names explicitly). Best-effort:
   log and ignore a deletion failure rather than failing the whole tool run
   over a leftover temp file the user never sees.
5. **Map the result:**
   - `tesseract` not found → `CommandRunner::execute` returns
     `CommandResult::SpawnFailed`. In practice this shouldn't be reachable
     given `Requires::Binary("tesseract")` already gates the tool
     proactively (§3.3), but it's handled defensively the same way
     `executor.rs`'s existing `unavailable_binary()` helper does, in case
     the binary is removed between the proactive check and the run.
   - `tesseract` runs but stdout is empty (or whitespace-only) after
     trimming → **not** a silent empty-string `Value` (a "Copy" button that
     copies nothing is a worse outcome than a clear message). Returns
     `ToolOutcome::Failed { stage: Stage::Native, detail: "no text was found
     in the selected region", hint: Some("try a region with clearer text or
     more contrast".to_owned()) }`.
   - `tesseract` runs and produces text → `ToolOutcome::Value { kind:
     ValueKind::Text, data: <trimmed stdout> }`.
   - Non-zero exit, timeout, or other `CommandResult` variants → mapped the
     same way `run_command`'s existing helpers already map them for Command
     tools (reusing that mapping logic rather than re-deriving it).

### 3.6 Dispatch (`wield-native/src/runner.rs`)

```rust
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeToolRunner;

#[async_trait]
impl NativeRunner for NativeToolRunner {
    async fn run(&self, id: &str, args: &ArgMap, cancel: CancellationToken) -> ToolOutcome {
        match id {
            "screen.ocr" => tools::screen_ocr::run(args, cancel).await,
            other => ToolOutcome::Failed {
                stage: Stage::Native,
                detail: format!("unknown native tool: {other}"),
                hint: Some("this is a bug in the tool descriptor".to_owned()),
            },
        }
    }
}
```

Exact parallel to `wield-portal/src/adapters/mod.rs`'s `dispatch` function.

## 4. Result delivery (confirmed, no new UI)

OCR'd text is returned as `ToolOutcome::Value { kind: ValueKind::Text, .. }`
and shown via the existing `ResultView` "Copy" button — the same one-click
pattern `color.pick` already has, not an automatic silent clipboard write.
Chosen over auto-copy specifically because it lets the user see the
recognized text before trusting it (OCR misreads happen) and needs zero new
frontend or backend clipboard code — `navigator.clipboard.writeText` is
already wired to any `Value` outcome.

## 5. Scope: English-only for M3a

Ships with the already-installed `eng` traineddata only; no language
picker or download UI. Tesseract defaults to `eng` when no `-l` override
disagrees with what's installed. "More downloadable" (per the original
spec's tools table) becomes a `docs/backlog.md` entry, the same treatment
M2d gave its own deliberately-cut scope (batch support for `pdf.compress`,
numeric-aware split-file sorting) — real scope, legitimately deferred rather
than silently dropped.

## 6. Testing

Mirrors `pick_color.rs`'s existing test shape:

- `wield-native/src/tools/screen_ocr.rs`: the capture step and the
  tesseract-execution step are each behind a small async function signature
  that tests can substitute a fake for (a fake `Result<PathBuf, PortalError>`
  future for capture; a fake `CommandResult` for the tesseract step) —
  exercising cancellation-wins-over-pending, dismissed-dialog-is-cancelled,
  transport-error-is-failed, empty-output-is-a-clear-failure, and
  happy-path-trims-and-returns-value, without a real portal or a real
  `tesseract` install.
- `wield-core`: `check_requires`'s new recursive `All` handling gets direct
  unit tests (portal-unmet, binary-unmet, both-met, first-of-two-unmet
  short-circuits) alongside the existing `Requires::Portal`/`Binary` tests.
  `AvailabilityView::probe_binaries` gets a test confirming a `Binary` entry
  nested inside `Requires::All` is still collected.
- `wield-tools`: `screen_ocr::descriptor()` gets the same
  build-succeeds/registers-cleanly coverage every other built-in already has
  via the workspace's descriptor snapshot test.
- Live verification (per the project's standing UI-walkthrough-testing
  requirement — verify via a real, end-user-style pass, not code-reasoning
  alone): once built, actually run `screen.ocr` from the palette
  against real on-screen text, confirm the interactive region-picker
  appears, confirm the recognized text is correct and copyable, and confirm
  a genuinely-blank region produces the "no text was found" message rather
  than a silent empty success.

## 7. Out of scope for M3a (tracked separately)

- `keep.awake` — M3b, after this ships.
- Downloadable OCR languages beyond `eng` — `docs/backlog.md`.
- Screen recorder (ScreenCast + PipeWire) — deferred in the original spec,
  its own future effort, not part of M3 at all.
- Bundling `tesseract` + `eng` into the Flatpak — already tracked in
  `docs/backlog.md`'s Packaging section, unaffected by this design.

## 8. Small implementation-detail decisions (settled here, not left open)

- **`wield-native` depends on `wield-portal` for `PortalError`.** Reuses its
  existing `Display`/`into_outcome` mapping (`Cancelled`/`Transport`/
  `BadResponse` → `ToolOutcome`) as-is rather than duplicating that small
  enum. `wield-portal` already has zero dependency on `wield-native` (it
  can't — `wield-native` doesn't exist yet), so this is a one-directional
  dependency, not a cycle. If a future `Native` tool's error shape doesn't
  fit `PortalError` cleanly, split it out then — not worth guarding against
  speculatively now.
- **URI → `PathBuf`**: `url::Url::parse(uri.as_str())?.to_file_path()`. The
  `url` crate is already present transitively through `ashpd`/`zbus`; add it
  as a direct dependency of `wield-native` rather than relying on the
  transitive edge (explicit is better than implicit for a crate boundary).
