# Wield M1 · Plan P5a — Shell core: commands, state, palette window, single-instance — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn `apps/wield` from a placeholder into the headless core of the Wield shell: an app that launches with no visible window, holds the built-in registry + a portal-aware `Executor`, exposes `list_tools` / `run_tool` / `cancel` / `capabilities` as Tauri commands, pre-warms a hidden frameless palette window it can show on demand, and owns the single-instance `io.github.DhanushSantosh.Wield` D-Bus name with `ShowPalette` / `RunTool` methods so a second launch reaches the running one.

**Status:** Complete on `feat/p5a-shell-core` — 2026-09-11.

**Architecture:** `wield-app`'s `run()` first acquires the single-instance D-Bus name over raw `zbus`; if another instance holds it, `run()` relays (`ShowPalette`, or `RunTool` when invoked as a runner) and exits. The primary instance builds Tauri with an `AppState` (the `wield-tools` built-in `Registry`, an `Executor` wired with `BinaryResolver` + `wield-portal`'s `PortalAdapterRunner` + a startup `AvailabilityView` from `probe()`, and a map of in-flight `CancellationToken`s), registers the four commands, and creates one hidden `palette` window. `run_tool` awaits the executor and returns the `ToolOutcome` (progress streaming over a Tauri channel is P6). `show_palette` shows the window and logs the show→visible latency via `tracing`. `logging` sends `tracing` to `$XDG_STATE_HOME/wield/logs` with daily rotation.

**Tech Stack:** Rust 2021, Tauri 2.9, `zbus` 5, Tokio, `tokio-util` `CancellationToken`, `tracing` + `tracing-subscriber` + `tracing-appender`, `uuid`; `wield-core` + `wield-tools` + `wield-portal` (path deps). `tauri` `test` feature for command tests.

**Spec:** `docs/superpowers/specs/2026-09-10-wield-design.md` — implements the §5 "lifecycle" (headless launch, single-instance D-Bus), the §5 "palette window" scaffold (hidden pre-warmed window; show/hide), the §3 "Rust side … Tauri commands: `list_tools`, `run_tool`, `cancel`, `capabilities`", and §7 logging. **Not** in P5a: tray, `GlobalShortcuts` portal, Preferences window, blur-to-hide, the React palette UI.

---

## GEON amendment — 2026-09-11 (owner design decisions)

Locked with the owner:

1. **P5 is split.** This is **P5a** — the shell *core*. **P5b** (separate plan) adds the tray (StatusNotifierItem) + menu, the `GlobalShortcuts` portal registration + fallback detection, the Preferences window, and blur/Esc hide wiring. The React palette UI is **P6**.
2. **Single-instance is a raw `zbus` service.** `wield-app` owns `io.github.DhanushSantosh.Wield` directly via `zbus` and serves `ShowPalette` / `RunTool`. No `tauri-plugin-single-instance`.
3. **Palette latency is instrumented, not gated.** `show_palette` logs the show→visible duration via `tracing`. A latency *verdict* waits for P6 (the real palette UI). Record the measurement method in `docs/testing.md`.

Additional constraints from GEON:

- **Scope:** `apps/wield` only. No changes to `wield-core` / `wield-portal` / `wield-tools` (their public APIs are sufficient — if one genuinely is not, **stop and ask GEON**). No tray, no `GlobalShortcuts`, no Preferences.
- **Branch:** `feat/p5a-shell-core` off `master`, per-task commits, push at the end, do **not** open the PR.
- **`cargo` is not on the default `PATH`.** `export PATH="$HOME/.cargo/bin:$PATH"` before every cargo invocation. A display **is** available here (`DISPLAY`, `WAYLAND_DISPLAY`) and the session bus is up, so the launch smoke test in Task 9 can run.
- **Mechanical-correction rule (as P2–P4):** Tauri 2.9 API details (window builder, tray, `State` extraction, the `test` module, event/emit signatures) shift between minor versions — apply the obvious fix to match the installed `tauri` and note it in the Task 10 report. Stop-and-ask only for a genuine design fork.
- **`npm run check`** green at the end of every task that touches buildable code.

---

## Global Constraints

- **Repo root:** `/home/dhanush/Projects/Wield` (branch `feat/p5a-shell-core` off `master`).
- **Crate:** all work in `apps/wield/src-tauri/` plus its `tauri.conf.json`; root `Cargo.toml` gains workspace deps.
- **App identifier:** `io.github.DhanushSantosh.Wield` — this is also the D-Bus well-known name and the config-dir key. Do not reintroduce `dev.wield.Wield`.
- **No `unwrap()` / `expect()` / `panic!`** in non-test code except: `run()` may `expect` on Tauri build failure (unrecoverable, as today), and the single-instance acquire may `expect` on a total D-Bus-unavailable failure only after logging — prefer degrading (log + run without the D-Bus service) where reasonable.
- **Headless:** the app starts with **no visible window**. The `palette` window exists but is `visible: false` until `show_palette`.
- **Wield never blocks startup** on a missing capability (spec §5): a failed `probe()` or an unavailable session bus logs a warning and the app still runs.
- **`npm run check`** green at the end of every task that touches buildable code.

---

## File Structure

**Modified:**

| File | Change |
| --- | --- |
| `Cargo.toml` (root) | add workspace deps: `tracing`, `tracing-subscriber`, `tracing-appender`, `zbus` already present, `uuid` already present |
| `apps/wield/src-tauri/Cargo.toml` | add deps: `wield-tools`, `wield-portal`, `zbus`, `tokio`, `tokio-util`, `tracing`, `tracing-subscriber`, `tracing-appender`, `uuid`; `tauri` gains no extra runtime features; add `[dev-dependencies]` `tauri = { workspace? no — } features = ["test"]` via `[features]`/dev — see Task 3 |
| `apps/wield/src-tauri/tauri.conf.json` | replace the single visible window with one hidden frameless `palette` window |
| `apps/wield/src-tauri/src/lib.rs` | `run()` rewritten: logging → acquire single-instance → (secondary: relay + exit) → primary: build Tauri with state + commands + palette window + serve the D-Bus object |
| `apps/wield/src-tauri/src/main.rs` | unchanged (`wield_app_lib::run()`) |
| `apps/wield/src-tauri/capabilities/default.json` | grant the `palette` window `core:window:allow-show` / `allow-hide` / `allow-set-focus` (adjust identifiers to the installed Tauri ACL) |

**Created — `apps/wield/src-tauri/src/`:**

| File | Responsibility |
| --- | --- |
| `logging.rs` | `init() -> Option<WorkerGuard>` — `tracing` to `$XDG_STATE_HOME/wield/logs/wield.log` (daily rotation, non-blocking) + stderr in debug |
| `state.rs` | `AppState { registry, executor, availability, runs: Mutex<HashMap<RunId, CancellationToken>> }`, `RunId(String)`, `AppState::build()` (async: `builtin_registry()`, resolve binaries, `probe()`), `register_run` / `take_run` |
| `capabilities.rs` | `CapabilitiesReport` (binaries map, portals map, per-tool `{ id, available, reason }`) + `fn report(state: &AppState) -> CapabilitiesReport` |
| `commands.rs` | the four `#[tauri::command]`s + `run_tool`'s json-args → `ArgMap` coercion + `show_palette` |
| `palette.rs` | `create_hidden(app) -> tauri::WebviewWindow`, `show(app)` (logs show→visible ms), `hide(app)` |
| `instance.rs` | `enum Acquired { Primary(zbus::Connection), Secondary }`, `async fn acquire(handle: InstanceHandle) -> Acquired`, `WieldService` (`ShowPalette`, `RunTool`), `async fn relay_show()` / `relay_run(id, args)` |
| `tests/` (in-file `#[cfg(test)]` where possible; `apps/wield/src-tauri/tests/commands.rs` for `tauri::test`) | command tests, capabilities report, arg coercion, instance relay |

**Out of P5a scope (do not add):** tray, `GlobalShortcuts`/`Background` portals, Preferences window, blur-to-hide, Esc-to-hide (frontend keybind → P6), progress streaming over a channel (P6), autostart, first-run onboarding, the full xvfb E2E smoke (P7 CI).

---

## Task 1: Deps, module skeleton, logging

**Files:**
- Modify: `Cargo.toml` (root), `apps/wield/src-tauri/Cargo.toml`
- Create: `apps/wield/src-tauri/src/logging.rs`
- Modify: `apps/wield/src-tauri/src/lib.rs` (declare modules; keep `app_info` + `run` compiling)

**Interfaces:**
- Produces: `logging::init() -> Option<tracing_appender::non_blocking::WorkerGuard>` — installs a `tracing_subscriber` writing to `$XDG_STATE_HOME/wield/logs/` (fallback `~/.local/state/wield/logs/`), daily rotation via `tracing_appender::rolling::daily`, non-blocking. In `debug_assertions` also layer a stderr writer. Returns the guard the caller must hold for the process lifetime; `None` if the log dir can't be created (log to stderr only, never fail).

- [x] **Step 1: Workspace deps**

Root `Cargo.toml` `[workspace.dependencies]` — add:

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"
```

(`zbus`, `uuid`, `tokio`, `tokio-util`, `serde`, `serde_json` already present.)

- [x] **Step 2: `apps/wield/src-tauri/Cargo.toml`**

```toml
[dependencies]
wield-core = { path = "../../../crates/wield-core" }
wield-tools = { path = "../../../crates/wield-tools" }
wield-portal = { path = "../../../crates/wield-portal" }
serde.workspace = true
serde_json.workspace = true
tauri = { version = "2.9.5", features = [] }
zbus.workspace = true
tokio = { workspace = true, features = ["rt-multi-thread", "sync", "macros", "time"] }
tokio-util.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
tracing-appender.workspace = true
uuid.workspace = true

[dev-dependencies]
tauri = { version = "2.9.5", features = ["test"] }
tempfile.workspace = true
```

- [x] **Step 3: `src/logging.rs`** — implement `init()` per the Interfaces block.

- [x] **Step 4: `src/lib.rs`** — add `mod logging;` etc. as modules are created; call `let _guard = logging::init();` at the top of `run()`. Keep the existing `app_info` command and `tauri::Builder` for now (later tasks replace the builder body).

- [x] **Step 5: Verify**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo build -p wield-app && npm run check`
Expected: PASS.

- [x] **Step 6: Commit**

```bash
git checkout -b feat/p5a-shell-core
git add Cargo.toml apps/wield/src-tauri
git commit -m "chore(app): P5a deps + tracing logging to XDG state dir"
```

---

## Task 2: `AppState`

**Files:**
- Create: `apps/wield/src-tauri/src/state.rs`
- Modify: `apps/wield/src-tauri/src/lib.rs`

**Interfaces:**
- Produces:

  ```rust
  use std::collections::HashMap;
  use std::sync::Mutex;
  use tokio_util::sync::CancellationToken;
  use wield_core::{AvailabilityView, BinaryResolver, Executor, Registry};

  #[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
  pub struct RunId(pub String);

  impl RunId {
      pub fn new() -> Self { Self(uuid::Uuid::new_v4().to_string()) }
  }

  pub struct AppState {
      pub registry: Registry,
      pub executor: Executor,
      pub availability: AvailabilityView,
      runs: Mutex<HashMap<RunId, CancellationToken>>,
  }

  impl AppState {
      /// Build the registry, resolve binaries, probe portals. Never fails —
      /// a failed portal probe leaves `availability.portals` empty.
      pub async fn build() -> Self {
          let registry = wield_tools::builtin_registry();
          let resolver = BinaryResolver::from_env();
          let mut availability = AvailabilityView::probe_binaries(&resolver, registry.list());
          wield_portal::probe().await.apply_to(&mut availability);
          let executor = Executor::new(resolver)
              .with_portal(std::sync::Arc::new(wield_portal::PortalAdapterRunner))
              .with_availability(availability.clone());
          Self { registry, executor, availability, runs: Mutex::new(HashMap::new()) }
      }

      pub fn register_run(&self, id: RunId, token: CancellationToken) {
          self.runs.lock().expect("runs lock").insert(id, token);
      }
      /// Remove and return the token (call when a run finishes).
      pub fn take_run(&self, id: &RunId) -> Option<CancellationToken> {
          self.runs.lock().expect("runs lock").remove(id)
      }
      /// Cancel a run by id; returns whether it was in flight.
      pub fn cancel_run(&self, id: &RunId) -> bool {
          match self.runs.lock().expect("runs lock").get(id) {
              Some(token) => { token.cancel(); true }
              None => false,
          }
      }
  }
  ```
  (`.expect("runs lock")` on a poisoned `Mutex` is acceptable in the shell — a poisoned lock means a prior panic already broke the process.)

- [x] **Step 1: Write the failing test**

`apps/wield/src-tauri/src/state.rs` tests module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn build_produces_the_builtin_registry() {
        let state = AppState::build().await;
        assert!(state.registry.get("image.convert").is_some());
        assert!(state.registry.get("color.pick").is_some());
    }

    #[test]
    fn cancel_run_toggles_a_registered_token() {
        let state = AppState {
            registry: wield_core::Registry::new(),
            executor: wield_core::Executor::new(wield_core::BinaryResolver::from_env()),
            availability: Default::default(),
            runs: std::sync::Mutex::new(std::collections::HashMap::new()),
        };
        let id = RunId::new();
        let token = CancellationToken::new();
        state.register_run(id.clone(), token.clone());
        assert!(state.cancel_run(&id));
        assert!(token.is_cancelled());
        assert!(state.take_run(&id).is_some());
        assert!(!state.cancel_run(&RunId::new()));
    }
}
```

- [x] **Step 2: Run to verify failure** — `cargo test -p wield-app state`
- [x] **Step 3: Implement `state.rs`; `mod state;` in `lib.rs`**
- [x] **Step 4: Run to verify pass** — `cargo test -p wield-app state`
- [x] **Step 5: Commit**

```bash
git add apps/wield/src-tauri
git commit -m "feat(app): AppState — registry, portal-wired executor, run tokens"
```

---

## Task 3: `capabilities` command

**Files:**
- Create: `apps/wield/src-tauri/src/capabilities.rs`
- Create: `apps/wield/src-tauri/src/commands.rs` (start it — just `capabilities`)
- Modify: `apps/wield/src-tauri/src/lib.rs`
- Test: `apps/wield/src-tauri/tests/commands.rs`

**Interfaces:**
- `capabilities.rs`:

  ```rust
  use serde::Serialize;
  use wield_core::{Capability, Requires};
  use crate::state::AppState;

  #[derive(Serialize)]
  pub struct ToolAvailability {
      pub id: String,
      pub title: String,
      pub available: bool,
      /// Present only when `available` is false — the spec's "why unavailable" text.
      pub reason: Option<String>,
  }

  #[derive(Serialize)]
  pub struct CapabilitiesReport {
      pub binaries: std::collections::BTreeMap<String, bool>,
      pub portals: std::collections::BTreeMap<String, u32>,
      pub tools: Vec<ToolAvailability>,
  }

  pub fn report(state: &AppState) -> CapabilitiesReport;
  ```
  `report` walks `state.registry.list()`: a tool is `available` when its `requires` is satisfied by `state.availability` (`Requires::None` always; `Binary(b)` → `availability.binaries.contains(b)`; `Portal { iface, min_ver }` → `availability.portals.get(iface) >= Some(min_ver)`). `reason` mirrors the P3 executor `Unavailable` wording ("the Screenshot desktop portal is version 1, but this tool needs version 2" / "magick is not installed"). `binaries` lists every distinct `Requires::Binary` name across the registry with its resolved bool; `portals` is `availability.portals` sorted.
- `commands.rs`:

  ```rust
  #[tauri::command]
  pub fn capabilities(state: tauri::State<'_, crate::state::AppState>) -> crate::capabilities::CapabilitiesReport {
      crate::capabilities::report(&state)
  }
  ```

- [x] **Step 1: Write the failing test**

`apps/wield/src-tauri/tests/commands.rs`:

```rust
// Uses tauri's mock runtime to exercise commands without a window.
use tauri::test::{mock_builder, mock_context, noop_assets};

fn app() -> tauri::App<tauri::test::MockRuntime> {
    let state = tauri::async_runtime::block_on(wield_app_lib::state::AppState::build());
    mock_builder()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            wield_app_lib::commands::capabilities
        ])
        .build(mock_context(noop_assets()))
        .expect("mock app")
}

#[test]
fn capabilities_lists_every_builtin_with_availability() {
    let app = app();
    let report: wield_app_lib::capabilities::CapabilitiesReport =
        tauri::test::get_ipc_response(
            &app.get_webview_window("main").unwrap_or_else(|| {
                // no window in headless mock — invoke via the app handle instead
                unreachable!("adjust to the installed tauri::test API")
            }),
            /* ... */
        )
        .unwrap();
    assert_eq!(report.tools.len(), 2);
}
```

**NOTE for the implementer:** the exact `tauri::test` invocation API (whether it's `get_ipc_response`, `InvokePayload`, or `WebviewWindow::run_on_main_thread`) differs across Tauri 2.x. Use whatever the installed `tauri` 2.9 `test` module provides to invoke a command and read its JSON result. If `tauri::test` cannot invoke a command that takes only `State` in this version, fall back to **testing `crate::capabilities::report(&state)` directly** (it is a pure function) and keep one thin `#[tauri::command]` wrapper untested — note the fallback in the report.

- [x] **Step 2: Run to verify failure** — `cargo test -p wield-app --test commands`
- [x] **Step 3: Implement `capabilities.rs` + `commands.rs` + wire `mod`s and `invoke_handler` in `lib.rs`**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src-tauri
git commit -m "feat(app): capabilities command — per-tool availability report"
```

---

## Task 4: `list_tools` command

**Files:**
- Modify: `apps/wield/src-tauri/src/commands.rs`, `apps/wield/src-tauri/tests/commands.rs`, `lib.rs` (handler list)

**Interfaces:**
- `commands.rs`:

  ```rust
  #[derive(serde::Serialize)]
  pub struct ToolSummary {
      pub id: String,
      pub title: String,
      pub keywords: Vec<String>,
      pub category: String,       // "Capture" | "Convert" | "Desktop"
      pub args: Vec<wield_core::ArgSpec>, // the schema the frontend renders the form from
      pub available: bool,
  }

  #[tauri::command]
  pub fn list_tools(state: tauri::State<'_, crate::state::AppState>) -> Vec<ToolSummary>;
  ```
  `list_tools` = `state.registry.list()` mapped to `ToolSummary`, with `available` computed exactly as in `capabilities::report`. Factor the availability check into `capabilities::is_available(&AvailabilityView, &Requires) -> (bool, Option<String>)` and use it in both.

- [x] **Step 1: Write the failing test**

```rust
#[test]
fn list_tools_returns_both_builtins_with_arg_schemas() {
    let state = tauri::async_runtime::block_on(wield_app_lib::state::AppState::build());
    let tools = wield_app_lib::commands::list_tools_impl(&state); // pure helper, see note
    assert_eq!(tools.len(), 2);
    let convert = tools.iter().find(|t| t.id == "image.convert").unwrap();
    assert_eq!(convert.args.len(), 4);
    assert_eq!(convert.category, "Convert");
}
```
Implement `list_tools` as a thin `#[tauri::command]` wrapper over `pub fn list_tools_impl(state: &AppState) -> Vec<ToolSummary>` so it is unit-testable regardless of the `tauri::test` API.

- [x] **Step 2-4: fail → implement → pass** (`cargo test -p wield-app`)
- [x] **Step 5: Commit**

```bash
git add apps/wield/src-tauri
git commit -m "feat(app): list_tools command with arg schemas"
```

---

## Task 5: `run_tool` command

**Files:**
- Modify: `apps/wield/src-tauri/src/commands.rs`, tests, `lib.rs`

**Interfaces:**
- `commands.rs`:

  ```rust
  #[derive(serde::Serialize)]
  pub struct RunResult {
      pub run_id: crate::state::RunId,
      pub outcome: wield_core::ToolOutcome,
  }

  /// Coerce a JSON args object (`{ "input": "/x/a.png", "format": "webp", "width": 800 }`)
  /// into an `ArgMap` using the descriptor's `ArgSpec` types.
  pub fn coerce_json_args(descriptor: &wield_core::Descriptor, args: &serde_json::Value)
      -> Result<wield_core::ArgMap, String>;

  pub async fn run_tool_impl(state: &AppState, id: &str, args: &serde_json::Value) -> RunResult;

  #[tauri::command]
  pub async fn run_tool(
      state: tauri::State<'_, AppState>,
      id: String,
      args: serde_json::Value,
  ) -> Result<RunResult, String>; // Err only for "unknown tool" / bad args json
  ```
  `run_tool_impl`: `state.registry.get(id)` → `coerce_json_args` → make `RunId` + `CancellationToken`, `state.register_run` → `state.executor.run(ExecutionRequest { descriptor: clone, args }, drained_progress_tx, token.clone()).await` → `state.take_run(&run_id)` → `RunResult { run_id, outcome }`. Progress is drained and discarded in P5a; P6 replaces the drain with a Tauri channel.
  `coerce_json_args`: for each key present, look up the `ArgSpec`; `File`/`Dir` → `ArgValue::Path`; `Int` → require a JSON integer; `Float` → number; `Bool` → bool; `Str`/`Text`/`Enum` → string. Unknown key or type mismatch → `Err`.

- [x] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn run_tool_runs_a_command_tool_against_a_stub() {
    // Register a throwaway descriptor via a private test helper on AppState, OR
    // point image.convert at a stub `magick` on a custom BinaryResolver.
    // Simplest: build a small AppState with a stub registry + stub-dir resolver.
    let dir = tempfile::tempdir().unwrap();
    // stub "magick": copy $1 -> last arg
    // ... write_stub_script(dir, "magick", "#!/bin/sh\ncat \"$1\" > \"${@: -1}\"\n")
    let input = dir.path().join("in.png");
    std::fs::write(&input, b"IMG").unwrap();

    let state = wield_app_lib::state::AppState::for_test(
        wield_tools::builtin_registry(),
        wield_core::BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]),
    ).await;

    let args = serde_json::json!({ "input": input.to_string_lossy(), "format": "png" });
    let result = wield_app_lib::commands::run_tool_impl(&state, "image.convert", &args).await;
    assert!(matches!(result.outcome, wield_core::ToolOutcome::File { .. }));
    assert!(state.take_run(&result.run_id).is_none()); // cleaned up
}

#[tokio::test]
async fn run_tool_rejects_unknown_tool() {
    let state = wield_app_lib::state::AppState::build().await;
    // via the pure impl returning Result at the command layer:
    assert!(wield_app_lib::commands::coerce_json_args(
        state.registry.get("image.convert").unwrap(),
        &serde_json::json!({ "bogus": 1 }),
    ).is_err());
}
```
Add `AppState::for_test(registry, resolver)` under `#[cfg(any(test, feature = "test-helpers"))]` — or just `#[cfg(test)]` and keep the test in `state.rs`/`commands.rs` in-file rather than the external `tests/` dir. Prefer in-file `#[cfg(test)]` so `for_test` needn't be public API.

- [x] **Step 2-4: fail → implement → pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src-tauri
git commit -m "feat(app): run_tool command — json args, executor, run registry"
```

---

## Task 6: `cancel` command

**Files:**
- Modify: `apps/wield/src-tauri/src/commands.rs`, tests, `lib.rs`

**Interfaces:**
- `commands.rs`:

  ```rust
  #[tauri::command]
  pub fn cancel(state: tauri::State<'_, AppState>, run_id: crate::state::RunId) -> bool {
      state.cancel_run(&run_id)
  }
  ```

- [x] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn cancel_stops_an_in_flight_run() {
    let dir = tempfile::tempdir().unwrap();
    // stub "magick": sleep 5 then write output
    // write_stub_script(dir, "magick", "#!/bin/sh\nsleep 5\ncat \"$1\" > \"${@: -1}\"\n")
    let input = dir.path().join("in.png");
    std::fs::write(&input, b"IMG").unwrap();
    let state = std::sync::Arc::new(
        wield_app_lib::state::AppState::for_test(
            wield_tools::builtin_registry(),
            wield_core::BinaryResolver::with_dirs(vec![dir.path().to_path_buf()]),
        ).await,
    );
    let args = serde_json::json!({ "input": input.to_string_lossy(), "format": "png" });

    let s2 = state.clone();
    let run = tokio::spawn(async move {
        wield_app_lib::commands::run_tool_impl(&s2, "image.convert", &args).await
    });

    // give it a moment to register, then cancel the only in-flight run
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    // find the one run id — expose AppState::in_flight_ids() under #[cfg(test)]
    let ids = state.in_flight_ids();
    assert_eq!(ids.len(), 1);
    assert!(state.cancel_run(&ids[0]));

    let result = run.await.unwrap();
    assert!(matches!(result.outcome, wield_core::ToolOutcome::Cancelled));
}
```

- [x] **Step 2-4: fail → implement → pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src-tauri
git commit -m "feat(app): cancel command — cancels an in-flight run"
```

---

## Task 7: Hidden palette window + `show_palette`

**Files:**
- Modify: `apps/wield/src-tauri/tauri.conf.json`, `apps/wield/src-tauri/capabilities/default.json`
- Create: `apps/wield/src-tauri/src/palette.rs`
- Modify: `apps/wield/src-tauri/src/commands.rs` (`show_palette`), `lib.rs`

**Interfaces:**
- `tauri.conf.json` `app.windows` → a single entry:

  ```json
  {
    "label": "palette",
    "title": "Wield",
    "width": 720,
    "height": 480,
    "center": true,
    "decorations": false,
    "alwaysOnTop": true,
    "skipTaskbar": true,
    "resizable": false,
    "visible": false
  }
  ```
- `palette.rs`:

  ```rust
  use tauri::{AppHandle, Manager, WebviewWindow};

  pub const LABEL: &str = "palette";

  /// The palette window (created from config, so this just fetches it).
  pub fn window(app: &AppHandle) -> Option<WebviewWindow> { app.get_webview_window(LABEL) }

  /// Show + focus the palette. Logs the show→visible duration.
  pub fn show(app: &AppHandle) {
      let Some(win) = window(app) else { tracing::warn!("palette window missing"); return; };
      let start = std::time::Instant::now();
      let _ = win.show();
      let _ = win.set_focus();
      tracing::info!(elapsed_ms = start.elapsed().as_millis() as u64, "palette shown");
  }

  pub fn hide(app: &AppHandle) {
      if let Some(win) = window(app) { let _ = win.hide(); }
  }
  ```
- `commands.rs`:

  ```rust
  #[tauri::command]
  pub fn show_palette(app: tauri::AppHandle) { crate::palette::show(&app); }

  #[tauri::command]
  pub fn hide_palette(app: tauri::AppHandle) { crate::palette::hide(&app); }
  ```
- `capabilities/default.json`: the `palette` window needs the window-show/hide/focus permissions. Set `"windows": ["palette"]` and add the show/hide/set-focus permission identifiers the installed Tauri exposes (e.g. `core:window:allow-show`, `core:window:allow-hide`, `core:window:allow-set-focus`). Verify against `cargo tauri` ACL or the generated `gen/schemas` after a build.

- [x] **Step 1: Update `tauri.conf.json` + `capabilities/default.json`**

- [x] **Step 2: Write `palette.rs` + the two commands**

- [x] **Step 3: Verify the backend compiles and the window is defined**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && rm -rf apps/wield/src-tauri/gen && cargo check -p wield-app`
Expected: PASS. Grep the regenerated `gen/schemas` or run `cargo tauri` to confirm `palette` is a known window label.

- [x] **Step 4: `npm run check`** — PASS.

- [x] **Step 5: Commit**

```bash
git add apps/wield/src-tauri
git commit -m "feat(app): hidden pre-warmed palette window + show/hide commands"
```

---

## Task 8: Single-instance D-Bus service

**Files:**
- Create: `apps/wield/src-tauri/src/instance.rs`
- Modify: `apps/wield/src-tauri/src/lib.rs`

**Interfaces:**
- `instance.rs`:

  ```rust
  use std::sync::Arc;

  pub const BUS_NAME: &str = "io.github.DhanushSantosh.Wield";
  pub const OBJECT_PATH: &str = "/io/github/DhanushSantosh/Wield";

  /// What the app can do for the D-Bus service without a Tauri `AppHandle`
  /// (so `instance.rs` is testable). Implemented over the real `AppHandle` in
  /// `lib.rs`.
  #[async_trait::async_trait]  // if wield-core exports async_trait re-use it; else add the dep
  pub trait ShellHandle: Send + Sync + 'static {
      fn show_palette(&self);
      async fn run_tool(&self, id: &str, args_json: &str) -> String; // ToolOutcome as JSON
  }

  pub enum Acquired {
      /// We own the name and the object server is running (hold the connection).
      Primary(zbus::Connection),
      /// Another instance answered — this process should exit 0.
      Secondary,
  }

  /// Try to own `BUS_NAME`. If we can't, relay `ShowPalette` (or, when `run` is
  /// `Some((id,args))`, `RunTool`, printing its result to stdout) to the running
  /// instance and return `Secondary`. A totally unavailable session bus logs a
  /// warning and returns `Primary`-without-a-connection-equivalent — model that
  /// as `Acquired::Primary` but with a note; simplest is a third arm or an
  /// `Option<Connection>`. Pick one and document it.
  pub async fn acquire(
      handle: Arc<dyn ShellHandle>,
      run: Option<(String, String)>,
  ) -> Acquired;
  ```
  Implementation:
  - `zbus::Connection::session().await` — on `Err`, `tracing::warn!` and return `Primary` (run without the service).
  - `conn.request_name_with_flags(BUS_NAME, [ReplaceExisting off, DoNotQueue on])` — `Ok(PrimaryOwner)` → register `WieldService { handle }` at `OBJECT_PATH`, return `Primary(conn)`.
  - `Ok(AlreadyExists)` / name taken → build a proxy to `BUS_NAME`, call `ShowPalette` or `RunTool` per `run`, return `Secondary`.
  - `WieldService` `#[zbus::interface(name = "io.github.DhanushSantosh.Wield")]`: `async fn show_palette(&self)` → `self.handle.show_palette()`; `async fn run_tool(&self, id: &str, args_json: &str) -> String` → `self.handle.run_tool(id, args_json).await`.

- [x] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct FakeShell { shown: Mutex<u32>, last_run: Mutex<Option<(String, String)>> }
    #[async_trait::async_trait]
    impl ShellHandle for FakeShell {
        fn show_palette(&self) { *self.shown.lock().unwrap() += 1; }
        async fn run_tool(&self, id: &str, args: &str) -> String {
            *self.last_run.lock().unwrap() = Some((id.into(), args.into()));
            "{\"Cancelled\":null}".into()
        }
    }

    #[tokio::test]
    async fn second_acquire_relays_show_palette_to_the_first() {
        // needs a private bus so the well-known name is free; reuse a dbus-daemon helper.
        let Some(bus) = crate::test_support::PrivateBus::launch() else { return; };
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", bus.address());

        let first = Arc::new(FakeShell { shown: Mutex::new(0), last_run: Mutex::new(None) });
        let Acquired::Primary(_conn) = acquire(first.clone(), None).await else { panic!("first should be primary") };

        let second = Arc::new(FakeShell { shown: Mutex::new(0), last_run: Mutex::new(None) });
        let acq = acquire(second.clone(), None).await;
        assert!(matches!(acq, Acquired::Secondary));

        // the relay calls ShowPalette on `first`
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        assert_eq!(*first.shown.lock().unwrap(), 1);
    }
}
```
Add a small `PrivateBus` helper (copy the shape from `wield-portal/tests/support/mod.rs` — a private `dbus-daemon`). Put it in `apps/wield/src-tauri/src/test_support.rs` under `#[cfg(test)]`, or a `tests/support/mod.rs`. Note the `DBUS_SESSION_BUS_ADDRESS` mutation makes these tests non-parallel-safe — gate the module with a serial guard or run this test file single-threaded and note it.

`async_trait`: `wield-core` already depends on `async-trait` (P3) but does not re-export it. Add `async-trait.workspace = true` to `wield-app`'s deps (allowed — it is not one of the three lib crates).

- [x] **Step 2: Run to verify failure** — `cargo test -p wield-app instance`
- [x] **Step 3: Implement `instance.rs`**
- [x] **Step 4: Run to verify pass**
- [x] **Step 5: Commit**

```bash
git add apps/wield/src-tauri
git commit -m "feat(app): single-instance io.github.DhanushSantosh.Wield D-Bus service"
```

---

## Task 9: `run()` wiring + headless launch

**Files:**
- Modify: `apps/wield/src-tauri/src/lib.rs`
- Test: `apps/wield/src-tauri/tests/launch.rs`

**Interfaces:**
- `lib.rs` `run()`:

  ```rust
  pub fn run() {
      let _guard = logging::init();

      // Decide primary/secondary before building Tauri.
      let handle = std::sync::Arc::new(PendingShell::default()); // ShellHandle that queues calls
      let acquired = tauri::async_runtime::block_on(instance::acquire(handle.clone(), runner_args_from_env()));
      if let instance::Acquired::Secondary = acquired {
          return; // relayed; exit 0
      }

      let state = tauri::async_runtime::block_on(state::AppState::build());

      tauri::Builder::default()
          .manage(state)
          .invoke_handler(tauri::generate_handler![
              commands::list_tools, commands::run_tool, commands::cancel,
              commands::capabilities, commands::show_palette, commands::hide_palette,
          ])
          .setup(move |app| {
              // bind the real AppHandle into the ShellHandle so D-Bus calls work
              handle.bind(app.handle().clone());
              // palette window already exists from config (visible:false) — nothing to show
              tracing::info!("wield shell ready (headless)");
              Ok(())
          })
          .build(tauri::generate_context!())
          .expect("build wield shell")
          .run(|_app, event| {
              // keep running with no windows visible; exit only on explicit Quit (P5b tray)
              if let tauri::RunEvent::ExitRequested { api, .. } = event {
                  api.prevent_exit(); // headless: closing the palette must not quit (P5b adds Quit)
              }
          });
  }
  ```
  `PendingShell` — a `ShellHandle` that, before `bind()`, records/deferred calls; after `bind(AppHandle)`, forwards `show_palette` to `palette::show` and `run_tool` to `commands::run_tool_impl` (block_on). Keep it small.
  `runner_args_from_env()` — `None` for P5a (the `RunTool`-from-CLI relay is exercised by P4's standalone one-shot; P5a only needs `ShowPalette` relay). Return `None` and leave a `// P5b/P4-bridge` note.

- [x] **Step 1: Write the launch smoke test**

`apps/wield/src-tauri/tests/launch.rs`:

```rust
//! Only runs where a display + session bus are available.
#[test]
#[ignore = "needs a display + session bus; run with --ignored"]
fn shell_launches_headless_and_owns_the_bus_name() {
    if std::env::var("WAYLAND_DISPLAY").is_err() && std::env::var("DISPLAY").is_err() {
        eprintln!("skipping: no display");
        return;
    }
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_wield"))
        .spawn()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_secs(3));

    // the name should now be owned
    let owned = std::process::Command::new("busctl")
        .args(["--user", "list"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("io.github.DhanushSantosh.Wield"))
        .unwrap_or(false);

    let _ = child.kill();
    let _ = child.wait();
    assert!(owned, "shell should own the D-Bus name while running");
}
```
(`busctl` is from systemd — present on this machine. If absent, skip.)

- [x] **Step 2: Run to verify failure / manual check**

Run: `export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p wield-app --test launch -- --ignored`
Expected: FAIL until `run()` is wired.

- [x] **Step 3: Implement the `run()` rewrite**

- [x] **Step 4: Run to verify pass** + a manual launch:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p wield-app --test launch -- --ignored
cargo run -p wield-app &   # launches headless — no window appears
sleep 2
busctl --user list | grep Wield          # name is owned
cargo run -p wield-app                    # second launch: relays ShowPalette, exits 0 (palette would show if a UI existed)
kill %1
```

- [x] **Step 5: Commit**

```bash
git add apps/wield/src-tauri
git commit -m "feat(app): headless run() wiring — single-instance guard + Tauri shell"
```

---

## Task 10: Workspace green + P5a wrap-up

- [x] **Step 1: Full checks**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p wield-app -- --ignored --list   # confirm the launch test is registered
```

- [x] **Step 2: `npm run check`** — PASS.

- [x] **Step 3: `docs/testing.md`** — create it (or append) with a "Palette latency" section: how `show_palette` logs `elapsed_ms`, where to read it (`$XDG_STATE_HOME/wield/logs/wield.log`), and that the real verdict comes in P6.

- [x] **Step 4: Manual smoke** (Task 9 Step 4 commands) — paste the outcome into the report.

- [x] **Step 5: Report to GEON**

```
agent-comms message post --to GEON --kind FYI --subject "P5a complete" --body "wield-app shell core on feat/p5a-shell-core: AppState (registry + portal-wired executor + run tokens), list_tools/run_tool/cancel/capabilities commands, hidden pre-warmed palette window + show_palette (latency logged), single-instance io.github.DhanushSantosh.Wield D-Bus service (ShowPalette/RunTool), headless run() wiring. cargo test --workspace + clippy + npm run check green; manual launch: headless, owns the bus name, second launch relays. N commits, branch pushed."
```

- [x] **Step 6: Commit the plan + push**

```bash
git add docs/superpowers/plans/2026-09-11-wield-m1-p5a-shell-core.md docs/testing.md
git commit -m "docs: mark M1 P5a plan complete"
git push -u origin feat/p5a-shell-core
```

GEON opens the PR, reviews, merges. **P5b** (tray + `GlobalShortcuts` portal + fallback + Preferences + blur/Esc hide) is written just-in-time.

---

## Self-Review

**Spec coverage (P5a slice):**

- §3 "Rust side … exposes Tauri commands: `list_tools`, `run_tool`, `cancel`, `capabilities`" → Tasks 3–6 ✓
- §3 "React side renders … per-tool argument forms generated from each descriptor's args schema" → `ToolSummary.args` carries the `ArgSpec` list (Task 4); the form itself is P6 ✓
- §5 "Launches headless: tray + hidden palette + registered shortcut; no main window" → headless + hidden palette here (Task 7, 9); **tray + shortcut are P5b** ✓
- §5 "single **frameless, centered, always-on-top** Tauri window, created hidden at startup and never destroyed" → `tauri.conf.json` window (Task 7); never destroyed — `ExitRequested` prevents exit (Task 9) ✓
- §5 "Show/hide on activation keeps the webview warm" → `palette::show`/`hide` toggle visibility, never recreate (Task 7) ✓
- §5 "Single instance owns the D-Bus name `io.github.DhanushSantosh.Wield` with `ShowPalette` / `RunTool` methods" → Task 8 ✓
- §5 "CLI with no instance running … runs the tool one-shot, no shell" → that path is P4 (already shipped); `RunTool` here is the *instance-running* path ✓
- §5 "Wield never blocks startup on a missing capability" → `AppState::build` + `acquire` degrade, never fail (Tasks 2, 8) ✓
- §7 logging: "`tracing` → `$XDG_STATE_HOME/wield/logs` with rotation. Per invocation: tool id, rendered argv, outcome, duration" → `logging::init` (Task 1); `run_tool_impl` should `tracing::info!` the id + outcome + elapsed (add to Task 5) ✓
- §7 "Native tools are wrapped at the Tauri command boundary (`catch_unwind` → `Failed`)" → **no Native tools exist yet** (P4's built-ins are Portal + Command); add the `catch_unwind` wrap when the first Native tool lands (M3). Noted, not a gap for P5a.
- §8 "E2E smoke (CI: xvfb + dbus session + mock portal): App boots headless → … owns `io.github.DhanushSantosh.Wield` → `RunTool` executes `image.convert` … → output file appears" → the launch test (Task 9) covers "boots headless + owns the name"; the full `RunTool`→file E2E is **P7 (CI)** — the pieces (`run_tool_impl`, the D-Bus `RunTool` method) are unit-tested here ✓

**Placeholder scan:** `run_tool`'s progress drain is a deliberate, documented P6 seam (not a TODO). The `acquire` "session bus totally unavailable" arm is called out as a design choice to pin during implementation, not left vague — the implementer picks `Option<Connection>` or a third enum arm and documents it. No bare `TODO`/`TBD` in step bodies.

**Type consistency:** `AppState` fields (Task 2) read by every command (Tasks 3–7). `RunId` (Task 2) in `RunResult` (Task 5), `cancel` arg (Task 6), and `AppState::{register,take,cancel}_run`. `is_available(&AvailabilityView, &Requires) -> (bool, Option<String>)` (Task 3) reused by `list_tools` (Task 4). `ShellHandle` trait (Task 8) implemented by `PendingShell` (Task 9). `coerce_json_args` (Task 5) is the app-side analogue of `wield-cli`'s `parse::coerce` — separate on purpose (JSON types vs CLI string tokens).

**Ordering:** 1 (deps/logging) → 2 (state) → 3 (capabilities, needs state) → 4 (list_tools, reuses 3's helper) → 5 (run_tool, needs state) → 6 (cancel, needs 5's run registry) → 7 (palette window, independent) → 8 (instance, needs a ShellHandle shape) → 9 (run wiring, needs 2/7/8) → 10. Consistent.

---

## Execution note

After P5a, the shell runs headless, holds the tools, executes them on command, and is reachable as a single D-Bus instance — but has no way for a user to *invoke* it (no tray, no hotkey) and nothing to *look at* (no palette UI). **P5b** adds the tray + the `GlobalShortcuts` portal (+ the "bind it yourself" fallback) + Preferences, giving the user entry points. **P6** builds the React palette that the `palette` window loads and wires it to `list_tools` / `run_tool` / `cancel` / `capabilities`.
