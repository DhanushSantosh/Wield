# Wield M1 · Plan P5b — Shell surfaces: tray, GlobalShortcuts, Preferences — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the headless P5a shell real entry points a user can act on: a StatusNotifierItem tray icon (left-click → palette; menu → tools by category + Preferences + Quit), the `GlobalShortcuts` portal registering a default `Super+W` binding that shows the palette, the DE-binding fallback when that portal is absent, a Preferences window (window + commands only — its content is P6), and correct close/hide/quit semantics (closing a window hides it; only the tray's Quit item actually exits).

**Architecture:** `wield-portal` gains a `global_shortcuts` module: `bind_show_palette(on_activated)` creates a `GlobalShortcuts` session, binds one shortcut (`show-palette`, preferred trigger `<Super>w`), and forwards each `Activated` event to a caller-supplied callback — the same "inject the effectful call, test the pure reaction" shape as P3's `pick_color_with`. `wield-app`'s `run()` adds a `TrayIconBuilder` (menu built from `list_tools_impl` grouped by category), spawns a task that calls `wield_portal::global_shortcuts::bind_show_palette` and wires its callback to `palette::show`, records whether the bind succeeded in `AppState` for the new `hotkey_status` command, opens a second hidden `preferences` window, and replaces P5a's blanket "prevent every exit" with per-window `CloseRequested → hide` plus a `quit` command that actually exits.

**Tech Stack:** Rust 2021, `ashpd` 0.13's `global_shortcuts` feature (new), Tauri 2.x tray + menu APIs (`TrayIconBuilder`, `Menu`/`MenuItem`, `TrayIconEvent`), Tokio.

**Spec:** `docs/superpowers/specs/2026-09-10-wield-design.md` §5 (tray, global shortcut + fallback, lifecycle, Preferences). **Not** in P5b: the Preferences window's actual settings UI, the "System Status" page content, autostart / the `Background` portal, first-run onboarding, `keep.awake`'s tray active-state indicator (no stateful tool exists until M3) — all deferred, most to P6.

---

## Status: COMPLETE — 2026-09-11 (branch `feat/p5b-shell-surfaces`, 8 commits)

Executed by GEON. `cargo test --workspace` + `cargo clippy --workspace --all-targets
-D warnings` + `cargo fmt --all --check` + `npm run check` all green; the ignored
launch smoke test passes; a manual launch confirmed headless + bus ownership + a
working tray (SNI host found) + a successful `GlobalShortcuts` bind (no
`Unavailable` warning logged) in this environment. See `docs/testing.md` for the
full account, including two things that were **not** independently verified here
for lack of an interactive session: an actual tray-icon click, and the Quit menu
item terminating the process (the `AppHandle::exit` contract is documented and
trusted, not click-tested).

**Deviations from the plan (all minor):** `build_menu` is generic over
`tauri::Runtime` (not hardcoded to `Wry`) so it is testable against
`tauri::test::MockRuntime` directly — no `MenuPlan` indirection needed, since
`Menu`/`Submenu::items()` + `MenuItemKind::id()` gave enough introspection.
`FALLBACK_COMMAND` is `"wield-app"` (relaunching the shell binary, which the
single-instance guard turns into a `ShowPalette` relay) rather than a
nonexistent `wield palette` CLI verb — `wield-cli` has no such subcommand.
Tasks 4 and 5 were implemented in dependency order (Preferences window before
the tray that calls into it) rather than the plan's listed order.

---

## GEON amendment — 2026-09-11 (grounded in the installed dependency versions)

Checked against what's actually resolved in this workspace before writing this plan:

1. **`ashpd` 0.13.13's `GlobalShortcuts::create_session` does not support a restore token.** (Confirmed: `restore_token` exists on `InputCapture`/`RemoteDesktop`/`ScreenCast` in this ashpd version, but `GlobalShortcuts`'s `CreateSessionOptions` has no such field.) This means **every launch re-shows the DE's shortcut-binding confirmation dialog** — there is no way to persist the session and skip it with this ashpd version. This is a real, documented limitation, not a bug to chase; note it in `docs/testing.md` and move on. Revisit when `ashpd` adds `GlobalShortcuts` session restore.
2. **Tauri resolves to 2.11.x** (`Cargo.lock` pins `2.11.5`), not the `2.9.5` floor named in earlier plans — the tray/menu APIs below (`TrayIconBuilder`, `MenuBuilder`/`SubmenuBuilder`, `on_menu_event`, `on_tray_icon_event`) are read from the *installed* `tauri-2.11.5` source, not guessed.
3. **P5a's `run()` closure over-prevents exit.** `app.run(|_app, event| { ExitRequested → prevent_exit() })` unconditionally blocks *every* exit, which would also swallow a real Quit. P5b **replaces** this: each window gets its own `on_window_event` handler that intercepts `WindowEvent::CloseRequested`, calls `api.prevent_close()`, and hides the window instead; the app-level `run` closure no longer blanket-prevents `ExitRequested`. `quit` calls `app_handle.exit(0)` directly. **Verify against the installed Tauri that `AppHandle::exit` does not itself route back through a preventable `ExitRequested` you'd need to special-case — if it does, gate the app-level prevention behind a `Arc<AtomicBool>` "quitting" flag set by the `quit` command before calling `exit`.** This is exactly the kind of version-specific behavior the mechanical-correction rule covers.

Additional constraints from GEON:

- **Scope:** `apps/wield` + `wield-portal` (one new module). No `wield-core` changes. No React/frontend content — every window in P5b is chrome only, same as P5a's placeholder palette.
- **Branch:** `feat/p5b-shell-surfaces` off `master`, per-task commits, push at the end, do **not** open the PR.
- **`cargo` is not on the default `PATH`.** `export PATH="$HOME/.cargo/bin:$PATH"` before every cargo invocation.
- **A display, a session bus, and a tray host may or may not be present** in the execution environment. Every live-portal / live-tray behavior must degrade to "log and continue" per spec §5 ("Wield never blocks startup on a missing capability") — write tests against the *pure* logic (session-bind callback wiring, menu construction, availability bookkeeping) and gate anything requiring a real SNI host or a real GlobalShortcuts-capable compositor behind `#[ignore]` with a clear skip message, exactly as P3 did for the live portal and P5a did for the live launch.
- **Mechanical-correction rule (as P2–P5a):** apply obvious fixes to match installed `tauri`/`ashpd` APIs; note them in the Task 8 report. Stop-and-ask only for a genuine design fork.
- **`npm run check`** green at the end of every task that touches buildable code.

---

## Global Constraints

- **Repo root:** `/home/dhanush/Projects/Wield` (branch `feat/p5b-shell-surfaces` off `master`).
- **Crates touched:** `apps/wield/src-tauri/`, `crates/wield-portal/` (new module only), root `Cargo.toml` (workspace dep feature addition).
- **No `unwrap()` / `expect()` / `panic!`** in non-test code except where P5a already established the pattern (poisoned-mutex `.expect`).
- **Never destroy the palette or preferences window.** Both windows are created once (via config or in `setup`), hidden by default, and toggled with `show`/`hide` for the rest of the process lifetime.
- **`npm run check`** green at the end of every task that touches buildable code.

---

## File Structure

**Modified:**

| File | Change |
| --- | --- |
| `Cargo.toml` (root) | `ashpd` workspace dep gains the `global_shortcuts` feature |
| `apps/wield/src-tauri/Cargo.toml` | no new deps (tray/menu are already part of the `tauri` crate) |
| `apps/wield/src-tauri/tauri.conf.json` | add a second window: `preferences` (hidden, normal-decorated, resizable, not always-on-top) |
| `apps/wield/src-tauri/capabilities/default.json` | extend `windows` to include `preferences`; add tray/menu permission identifiers the installed Tauri ACL requires |
| `apps/wield/src-tauri/src/state.rs` | `AppState` gains `hotkey: Mutex<HotkeyState>` (`Registered` \| `Unavailable { fallback_command: String }`) |
| `apps/wield/src-tauri/src/commands.rs` | add `hotkey_status`, `quit` commands |
| `apps/wield/src-tauri/src/palette.rs` | `on_close_requested` helper reused by both windows (see Task 6) |
| `apps/wield/src-tauri/src/lib.rs` | wire the tray, the `GlobalShortcuts` bind task, the two windows' close handling, and `quit` |
| `crates/wield-portal/src/lib.rs` | `pub mod global_shortcuts;` |

**Created:**

| File | Responsibility |
| --- | --- |
| `crates/wield-portal/src/global_shortcuts.rs` | `bind_show_palette(on_activated: impl Fn() + Send + Sync + 'static) -> BindOutcome` (and the testable `bind_show_palette_with` core), `BindOutcome { Bound, Unavailable { fallback_command } }` |
| `apps/wield/src-tauri/src/tray.rs` | `build(app) -> tauri::Result<()>` — icon, menu (tools by category + Preferences + Quit), left-click → `show_palette`, menu-item click routing |
| `apps/wield/src-tauri/src/preferences.rs` | `window(app) -> Option<WebviewWindow>`, `show`/`hide` (mirrors `palette.rs`) |
| `apps/wield/src-tauri/tests/tray.rs` (or in-file) | menu construction from a fixture registry |

**Out of P5b scope (do not add):** any window content (React), System Status data beyond what `capabilities`/`hotkey_status` already expose, autostart/`Background` portal, first-run onboarding, per-tool enable/disable persistence, hotkey rebinding UI (the fallback text is enough — actual rebinding is a Preferences-content feature, P6+).

---

## Task 1: `wield-portal` — `global_shortcuts` module

**Files:**
- Create: `crates/wield-portal/src/global_shortcuts.rs`
- Modify: `crates/wield-portal/src/lib.rs`, root `Cargo.toml` (ashpd feature)

**Interfaces:**
- Root `Cargo.toml`: `ashpd = { version = "0.13", default-features = false, features = ["tokio", "screenshot", "global_shortcuts"] }`.
- `global_shortcuts.rs`:

  ```rust
  //! The `GlobalShortcuts` portal: bind a single "show palette" shortcut and
  //! forward activations to a callback. No session-restore token is available
  //! in ashpd 0.13 for this portal, so the DE's binding dialog reappears on
  //! every bind — documented, not a bug.

  use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
  use ashpd::desktop::Session;

  pub const SHORTCUT_ID: &str = "show-palette";
  pub const PREFERRED_TRIGGER: &str = "<Super>w";
  pub const FALLBACK_COMMAND: &str = "wield palette"; // TODO(P6/CLI): confirm this subcommand exists before shipping onboarding copy

  #[derive(Debug, Clone, PartialEq)]
  pub enum BindOutcome {
      /// The shortcut is bound; activations will invoke the callback.
      Bound,
      /// The portal is absent or binding failed. Carries the copyable fallback
      /// command for Preferences to display.
      Unavailable { fallback_command: String },
  }

  /// Create a session, bind `show-palette` (`<Super>w` preferred), and spawn a
  /// task forwarding every `Activated` signal to `on_activated`. Returns
  /// immediately with the bind outcome; the forwarding task runs for the
  /// process lifetime (or until the portal connection drops).
  pub async fn bind_show_palette(
      on_activated: impl Fn() + Send + Sync + 'static,
  ) -> BindOutcome {
      match try_bind(&on_activated).await {
          Ok(()) => BindOutcome::Bound,
          Err(error) => {
              tracing::warn!(%error, "GlobalShortcuts portal unavailable");
              BindOutcome::Unavailable { fallback_command: FALLBACK_COMMAND.to_owned() }
          }
      }
  }

  async fn try_bind(on_activated: &(impl Fn() + Send + Sync + 'static)) -> ashpd::Result<()> {
      let portal = GlobalShortcuts::new().await?;
      let session = portal.create_session(Default::default()).await?;
      let shortcut = NewShortcut::new(SHORTCUT_ID, "Show the Wield palette")
          .preferred_trigger(PREFERRED_TRIGGER);
      portal
          .bind_shortcuts(&session, &[shortcut], None, Default::default())
          .await?
          .response()?;
      spawn_activation_forwarder(portal, on_activated_boxed(on_activated));
      Ok(())
  }
  ```
  (`spawn_activation_forwarder` — `tokio::spawn` a loop over `portal.receive_activated().await?` filtering `activated.shortcut_id() == SHORTCUT_ID` and calling the callback; hold `portal`/`session` alive by moving them into the task. `on_activated_boxed` is a small helper to erase the closure type into `Arc<dyn Fn() + Send + Sync>` if needed for `'static` — implementer's call on the exact shape.)
  **Testable core:** factor `try_bind`'s post-connection logic so a unit test can exercise "what happens when `bind_shortcuts` returns an error" without a real portal — e.g. a `fn classify_bind_error(e: &ashpd::Error) -> BindOutcome`-style pure mapping, OR simply unit-test `BindOutcome` construction and leave `bind_show_palette` itself covered only by an `#[ignore]`d live test. Prefer the latter (matches P3's precedent) if factoring the ashpd call out cleanly is awkward.

- [ ] **Step 1: Write the failing test**

`crates/wield-portal/src/global_shortcuts.rs` (or `tests/global_shortcuts.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_command_is_a_real_wield_cli_invocation() {
        assert_eq!(FALLBACK_COMMAND, "wield palette");
    }

    #[tokio::test]
    #[ignore = "requires a compositor with the GlobalShortcuts portal; run manually"]
    async fn binds_against_a_real_portal() {
        let outcome = bind_show_palette(|| {}).await;
        // Either shape is acceptable outside a real portal-enabled session;
        // this test is a manual smoke, not a CI gate.
        assert!(matches!(outcome, BindOutcome::Bound | BindOutcome::Unavailable { .. }));
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p wield-portal global_shortcuts`
- [ ] **Step 3: Implement `global_shortcuts.rs`; `pub mod global_shortcuts;` in `lib.rs`**
- [ ] **Step 4: Run to verify pass** — `cargo test -p wield-portal`
- [ ] **Step 5: Commit**

```bash
git checkout -b feat/p5b-shell-surfaces
git add Cargo.toml crates/wield-portal
git commit -m "feat(portal): GlobalShortcuts bind_show_palette (no session-restore token in ashpd 0.13)"
```

---

## Task 2: `AppState` hotkey status

**Files:**
- Modify: `apps/wield/src-tauri/src/state.rs`

**Interfaces:**

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub enum HotkeyState {
    Registered,
    Unavailable { fallback_command: String },
    /// Bind hasn't completed yet (briefly, at startup).
    Pending,
}

// AppState gains:
hotkey: std::sync::Mutex<HotkeyState>,

impl AppState {
    pub fn hotkey_state(&self) -> HotkeyState { self.hotkey.lock().expect("hotkey lock").clone() }
    pub fn set_hotkey_state(&self, state: HotkeyState) { *self.hotkey.lock().expect("hotkey lock") = state; }
}
```
`AppState::build()` / `AppState::for_test` initialize `hotkey: Mutex::new(HotkeyState::Pending)`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn hotkey_state_defaults_pending_and_is_settable() {
    let state = AppState::for_test_sync(); // or reuse the existing test constructor
    assert!(matches!(state.hotkey_state(), HotkeyState::Pending));
    state.set_hotkey_state(HotkeyState::Registered);
    assert!(matches!(state.hotkey_state(), HotkeyState::Registered));
}
```
(Use whatever `#[cfg(test)]` state constructor P5a already added; extend it rather than inventing a second one.)

- [ ] **Step 2-4: fail → implement → pass** (`cargo test -p wield-app`)
- [ ] **Step 5: Commit**

```bash
git add apps/wield/src-tauri/src/state.rs
git commit -m "feat(app): AppState hotkey status tracking"
```

---

## Task 3: `hotkey_status` and `quit` commands

**Files:**
- Modify: `apps/wield/src-tauri/src/commands.rs`

**Interfaces:**

```rust
#[tauri::command]
pub fn hotkey_status(state: tauri::State<'_, AppState>) -> crate::state::HotkeyState {
    state.hotkey_state()
}

#[tauri::command]
pub fn quit(app: tauri::AppHandle) {
    tracing::info!("quit requested");
    app.exit(0);
}
```

- [ ] **Step 1: Write the failing test** (pure, no window needed)

```rust
#[test]
fn hotkey_status_reflects_app_state() {
    let state = /* build a minimal AppState as in Task 2's test */;
    state.set_hotkey_state(crate::state::HotkeyState::Unavailable { fallback_command: "wield palette".into() });
    assert!(matches!(hotkey_status(tauri_state_wrapper(&state)), crate::state::HotkeyState::Unavailable { .. }));
    // If wrapping a bare &AppState as tauri::State is awkward outside a real
    // app, test the state accessor directly (Task 2's test already does) and
    // treat `hotkey_status` as a one-line pass-through — note this in the report.
}
```

- [ ] **Step 2-4: fail → implement → pass**
- [ ] **Step 5: Commit**

```bash
git add apps/wield/src-tauri/src/commands.rs
git commit -m "feat(app): hotkey_status and quit commands"
```

---

## Task 4: Tray icon + menu

**Files:**
- Create: `apps/wield/src-tauri/src/tray.rs`
- Modify: `apps/wield/src-tauri/src/lib.rs` (call `tray::build` from `setup`)
- Test: menu-construction unit tests in `tray.rs`

**Interfaces:**

```rust
//! The StatusNotifierItem tray icon and its menu.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

/// Build the tray menu: one submenu per `Category` present in `tools`
/// (Capture / Convert / Desktop, only the ones that actually have tools,
/// in that fixed order), then a separator, "Preferences", "Quit".
/// Each tool item's id is `"tool:{descriptor.id}"`.
pub fn build_menu<R: tauri::Runtime>(
    app: &AppHandle<R>,
    tools: &[crate::commands::ToolSummary],
) -> tauri::Result<Menu<R>>;

/// Attach the tray icon + menu + event handlers. Call once from `setup`.
pub fn build(app: &AppHandle) -> tauri::Result<()>;
```
`build`:
- `TrayIconBuilder::new().icon(app.default_window_icon().cloned().unwrap_or_default()).menu(&menu).show_menu_on_left_click(false)` — left-click is handled explicitly via `on_tray_icon_event` (`TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }` → `palette::show(app)`); the menu opens on right-click / the platform's normal secondary trigger (default Tauri behavior when `show_menu_on_left_click` is left `true` — pick whichever the installed Tauri makes least surprising and document the choice).
- `on_menu_event`: match `event.id().as_ref()`: `"preferences"` → `preferences::show(app)`; `"quit"` → `app.exit(0)`; `id.strip_prefix("tool:")` → emit a `"tray://select-tool"` event with the tool id **and** call `palette::show(app)` (P6's frontend will later listen for the event to pre-select the tool; showing the palette regardless is useful today).
- Build the menu from `commands::list_tools_impl(&state)` read out of `app.state::<AppState>()` at tray-build time (tools don't change at runtime in P5b, so a static-at-startup menu is correct; a future plan can rebuild it if that changes).

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_tools() -> Vec<crate::commands::ToolSummary> {
        // two tools, Capture + Convert, mirroring the real builtin_registry shape
        vec![
            crate::commands::ToolSummary { id: "color.pick".into(), title: "Pick a colour".into(), keywords: vec![], category: "Capture".into(), args: vec![], available: true },
            crate::commands::ToolSummary { id: "image.convert".into(), title: "Convert image".into(), keywords: vec![], category: "Convert".into(), args: vec![], available: true },
        ]
    }

    #[test]
    fn menu_groups_tools_by_category_and_adds_the_fixed_items() {
        // Building a real tauri::Menu needs an AppHandle; use tauri::test::mock_app()
        // (as P5a's command tests do) to get one, then assert on menu item ids/text
        // via whatever introspection tauri::menu::Menu exposes in the installed
        // version (e.g. iterating `menu.items()`). If menu introspection isn't
        // ergonomic, assert instead on a pure `fn plan_menu(tools) -> MenuPlan`
        // (Vec<(Category, Vec<ToolSummary>)> + trailing fixed items) and keep
        // `build_menu` as a thin renderer over it — note whichever you pick.
    }
}
```
**Implementer's call, per the note above:** if Tauri's `Menu`/`MenuItem` types don't expose enough introspection to assert on directly, split `tray.rs` into a pure `fn plan_menu(tools: &[ToolSummary]) -> MenuPlan` (a plain data structure: ordered `(category_label, Vec<(id, title)>)` groups + `preferences`/`quit`) that is fully unit-testable, and a thin `build_menu` that renders a `MenuPlan` into real Tauri menu types (untested beyond compiling). This mirrors the `list_tools_impl` / `list_tools` split from P5a.

- [ ] **Step 2: Run to verify failure** — `cargo test -p wield-app tray`
- [ ] **Step 3: Implement `tray.rs`; call `tray::build(&app_handle)` from `lib.rs`'s `setup`**
- [ ] **Step 4: Run to verify pass**
- [ ] **Step 5: Commit**

```bash
git add apps/wield/src-tauri/src/tray.rs apps/wield/src-tauri/src/lib.rs
git commit -m "feat(app): tray icon and menu (tools by category + Preferences + Quit)"
```

---

## Task 5: Preferences window

**Files:**
- Modify: `apps/wield/src-tauri/tauri.conf.json`, `apps/wield/src-tauri/capabilities/default.json`
- Create: `apps/wield/src-tauri/src/preferences.rs`
- Modify: `apps/wield/src-tauri/src/lib.rs`

**Interfaces:**
- `tauri.conf.json` `app.windows` gains:

  ```json
  {
    "label": "preferences",
    "title": "Wield Preferences",
    "width": 640,
    "height": 480,
    "resizable": true,
    "visible": false
  }
  ```
- `preferences.rs` (mirrors `palette.rs`):

  ```rust
  use tauri::{AppHandle, Manager, WebviewWindow};

  pub const LABEL: &str = "preferences";

  pub fn window(app: &AppHandle) -> Option<WebviewWindow> { app.get_webview_window(LABEL) }
  pub fn show(app: &AppHandle) {
      if let Some(win) = window(app) { let _ = win.show(); let _ = win.set_focus(); }
  }
  pub fn hide(app: &AppHandle) {
      if let Some(win) = window(app) { let _ = win.hide(); }
  }
  ```
- `capabilities/default.json`: add `"preferences"` to `windows`.

- [ ] **Step 1-4:** no new test-driven behavior beyond "the window exists and show/hide don't panic" — add one smoke test using `tauri::test::mock_app` (or the pattern Task 4 settled on) asserting `preferences::window(&app).is_some()` after setup, or, if that's awkward without a real webview, skip automated coverage here and note it (this file is intentionally thin, symmetric with `palette.rs`).
- [ ] **Step 5: Commit**

```bash
git add apps/wield/src-tauri/tauri.conf.json apps/wield/src-tauri/capabilities/default.json apps/wield/src-tauri/src/preferences.rs apps/wield/src-tauri/src/lib.rs
git commit -m "feat(app): hidden Preferences window"
```

---

## Task 6: Close-to-hide, real quit, `GlobalShortcuts` wiring in `run()`

**Files:**
- Modify: `apps/wield/src-tauri/src/lib.rs`

**Interfaces:**
- Per-window close handling (attach in `setup`, once each window exists):

  ```rust
  for label in [palette::LABEL, preferences::LABEL] {
      if let Some(window) = app.get_webview_window(label) {
          let window_clone = window.clone();
          window.on_window_event(move |event| {
              if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                  api.prevent_close();
                  let _ = window_clone.hide();
              }
          });
      }
  }
  ```
- Replace the P5a `.run(|_app, event| { ExitRequested → prevent_exit() })` closure. Simplest correct version, pending the version check called out in the GEON amendment:

  ```rust
  app.run(|_app, _event| {});
  ```
  If verification shows `AppHandle::exit` in the installed Tauri *does* route back through a preventable `RunEvent::ExitRequested`, keep a guard:

  ```rust
  let quitting = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
  // `quit` command sets `quitting.store(true, Ordering::SeqCst)` before calling `app.exit(0)`.
  app.run(move |_app, event| {
      if let tauri::RunEvent::ExitRequested { api, .. } = event {
          if !quitting.load(std::sync::atomic::Ordering::SeqCst) {
              api.prevent_exit();
          }
      }
  });
  ```
  Pick whichever the installed Tauri actually needs; document the choice in `docs/testing.md`.
- `GlobalShortcuts` wiring in `run()`, after the Tauri app is built and windows exist:

  ```rust
  let hotkey_app = app.handle().clone();
  tauri::async_runtime::spawn(async move {
      let show_app = hotkey_app.clone();
      let outcome = wield_portal::global_shortcuts::bind_show_palette(move || {
          palette::show(&show_app);
      }).await;
      let state = hotkey_app.state::<state::AppState>();
      state.set_hotkey_state(match outcome {
          wield_portal::global_shortcuts::BindOutcome::Bound => state::HotkeyState::Registered,
          wield_portal::global_shortcuts::BindOutcome::Unavailable { fallback_command } => {
              state::HotkeyState::Unavailable { fallback_command }
          }
      });
  });
  ```

- [ ] **Step 1: Write the failing test**

Given this task is mostly Tauri runtime wiring with no pure-logic surface beyond what Tasks 1–3 already cover, its "test" is the manual launch smoke below plus a compile-time check that `state::HotkeyState` round-trips through the spawn closure. If a meaningful unit test exists (e.g. asserting the close handler is registered), write it; otherwise proceed to Step 3 and rely on Step 4's manual verification — note this in the report rather than inventing a test with no signal.

- [ ] **Step 2: (see Step 1)**

- [ ] **Step 3: Implement the `run()` changes**

- [ ] **Step 4: Manual verification**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build -p wield-app
./target/debug/wield-app &
sleep 2
busctl --user list | grep Wield             # still owns the name
# if a tray host is present: confirm the icon appears, left-click shows the
# (still contentless) palette window, the menu lists color.pick / image.convert
# / Preferences / Quit, and Quit actually terminates the process.
kill %1 2>/dev/null   # in case Quit wasn't exercised manually
```
Read `$XDG_STATE_HOME/wield/logs/wield.log.<date>` (or the fallback path) for the `GlobalShortcuts` bind outcome (`Bound` or `Unavailable` with the fallback command) — record which one this environment produced in the Task 8 report; either is a valid pass (spec: never block startup on a missing capability).

- [ ] **Step 5: Commit**

```bash
git add apps/wield/src-tauri/src/lib.rs
git commit -m "feat(app): close-to-hide windows, real quit, GlobalShortcuts wiring"
```

---

## Task 7: `docs/testing.md` — hotkey + tray notes

**Files:**
- Modify: `docs/testing.md`

- [ ] **Step 1: Append a "Tray and global shortcut" section** covering: the `ashpd` 0.13 GlobalShortcuts no-restore-token limitation (Task 1's finding) and its user-visible consequence (dialog on every launch); how to read the bind outcome from the logs; the exit-semantics decision made in Task 6 (which `run()` shape was needed for this Tauri version); and, if this environment lacked a tray host or a GlobalShortcuts-capable compositor, say so plainly rather than claiming untested behavior works.
- [ ] **Step 2: Commit**

```bash
git add docs/testing.md
git commit -m "docs: tray + GlobalShortcuts testing notes"
```

---

## Task 8: Workspace green + P5b wrap-up

- [ ] **Step 1: Full checks**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- [ ] **Step 2: `npm run check`** — PASS.

- [ ] **Step 3: Report to GEON**

```
agent-comms message post --to GEON --kind FYI --subject "P5b complete" --body "Tray (menu by category + Preferences + Quit), GlobalShortcuts portal binding (no session-restore token in ashpd 0.13 - documented) with DE-dialog-on-every-launch as a known limitation, Preferences window, and close-to-hide/real-quit semantics landed on feat/p5b-shell-surfaces. hotkey_status + quit commands added. cargo test --workspace + clippy + npm run check green. Manual smoke: <bound|fallback> in this environment, tray <present|absent>. N commits, branch pushed."
```

- [ ] **Step 4: Commit the plan + push**

```bash
git add docs/superpowers/plans/2026-09-11-wield-m1-p5b-shell-surfaces.md
git commit -m "docs: mark M1 P5b plan complete"
git push -u origin feat/p5b-shell-surfaces
```

GEON opens the PR, reviews, merges. **P6** (the React palette + Preferences UI) is written just-in-time; it is the first plan to touch `apps/wield/src`.

---

## Self-Review

**Spec coverage:**

- §5 "Tray … Always-visible anchor. Left-click opens the palette; the menu lists tools by category plus Preferences and Quit" → Task 4 ✓
- §5 "stateful tools (`keep.awake`) show active state" → **no stateful tool exists yet** (M3); noted as forward-compatible, not a gap ✓
- §5 "On GNOME this needs the AppIndicator extension — if SNI registration finds no host, Wield says so once" → the tray build failure/absence is logged (Task 4's `build` returns `tauri::Result`; a caller-side `if let Err = tray::build(app) { tracing::warn!(..) }` — add this exact one-time log in Task 4's implementation) — a *visible* one-time notice is Preferences-content, P6 ✓ (logged today, surfaced later)
- §5 "Primary: the `GlobalShortcuts` portal, registered on first run (default Super+W) … Tauri's own `globalShortcut` API is X11-only and is not used" → Task 1, 6 ✓ (session-restore caveat honestly documented)
- §5 "Fallback: Preferences shows a copyable `wield palette` command" → `HotkeyState::Unavailable { fallback_command }` exposed via `hotkey_status` (Tasks 2, 3); the *display* of it is Preferences content (P6) — the data is ready ✓
- §5 "Preferences … hotkey setup … System Status page" → the window + `hotkey_status`/`capabilities` commands exist; the page itself is P6 ✓
- §5 "Single instance … the tray, and `wield palette` from a second launch all route through it" → tray left-click and menu already call `palette::show` directly in-process (no D-Bus round-trip needed since tray runs inside the primary instance) — consistent with P5a's D-Bus path being for *external* second launches ✓

**Placeholder scan:** the `FALLBACK_COMMAND` constant carries an inline `TODO(P6/CLI)` comment flagging it for confirmation once the CLI/onboarding copy is finalized — this is a deliberate cross-plan marker, not a plan-completion placeholder, and is called out explicitly rather than hidden. No step body contains an unresolved `TBD`.

**Type consistency:** `BindOutcome` (Task 1) ↔ `HotkeyState` (Task 2) mapped 1:1 in Task 6. `ToolSummary` (P5a) is the exact input to `tray::build_menu` (Task 4) — no new tool-representation type invented. `preferences.rs` mirrors `palette.rs`'s exact shape (Task 5) so both windows are handled identically in Task 6's close-loop.

**Ordering:** 1 (portal module) → 2 (state field) → 3 (commands, needs 2) → 4 (tray, needs 3 for `list_tools_impl`, independent of 1) → 5 (preferences window, independent) → 6 (wiring, needs 1, 3, 4, 5) → 7 (docs) → 8. Consistent — 4 and 5 could run in either order or in parallel.

---

## Execution note

After P5b, the shell is fully reachable (tray always, hotkey when the portal supports it, D-Bus for a second launch) and fully controllable (real Quit, windows that hide instead of vanishing) — but every window remains the P1 placeholder screen. **P6** builds the actual palette (search, keyboard nav, the arg-form generator reading `ToolSummary.args`, progress via a Tauri channel, result cards) and the Preferences content (hotkey status + fallback command display, System Status from `capabilities`), replacing `apps/wield/src`'s placeholder for the first time since P1.
