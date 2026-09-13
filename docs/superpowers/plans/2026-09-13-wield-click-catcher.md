# Click-Catcher (Outside-Click Dismiss) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clicking anywhere outside the palette's own visible bounds while it's shown dismisses it — including clicking the window that was already active before the palette opened, which currently does nothing at all.

**Architecture:** A second, content-free `gtk::Window` (not a Tauri-managed window — it has no HTML/JS at all), configured via `gtk-layer-shell` to cover the whole output on `Layer::Top` (the palette stays on `Layer::Overlay`, which always renders above it), fully transparent, with a single click handler that calls the same `palette::hide()` the Escape key already uses. Shown/hidden in lockstep with the palette from `AppState`.

**Tech Stack:** Rust, `gtk` 0.18, `gtk-layer-shell` 0.8 (already dependencies of `apps/wield/src-tauri`), Tauri 2.9.5.

**Spec:** `docs/superpowers/specs/2026-09-13-wield-click-catcher-design.md` — read both; this plan argues from that spec's decisions (raw GTK window vs. a second Tauri window, `Layer::Top` vs `Overlay`, rejected alternatives) without re-litigating them.

## Global Constraints

- Branch: `fix/click-catcher-design` (already created and checked out).
- The palette's own `KeyboardMode::Exclusive`, typing, and Escape behavior do not change — this work is purely additive (spec §2).
- Gated by `layer_shell::is_available()` — on X11/GNOME, the catcher is never created; no partial or broken state (spec §3.3, §4.4).
- Every raw GTK object touched from outside the GTK main thread must go through `WebviewWindow::run_on_main_thread` first — GTK objects are not `Send`/thread-safe (established convention throughout `palette.rs`/`layer_shell.rs`).
- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace -- --test-threads=1` must be clean before each commit.
- This feature is not meaningfully unit-testable past its state-plumbing and fail-closed gate (no fake Wayland compositor in this test suite) — final verification is live, using `hyprctl`/`grim`/synthetic input, matching this project's established convention for compositor-boundary code. Document findings in `docs/testing.md`.

---

### Task 1: `force_commit` generalized from `&gtk::ApplicationWindow` to `&gtk::Window`

> **Pre-flight ruling:** this task and Task 2 were swapped from the order
> they were first drafted in. Task 2's `show_click_catcher` calls
> `force_commit` on the catcher (a fix for a real gap found in the
> pre-flight scan — see Task 2's own note) and needs this task's widened
> signature to already exist to do that; the two tasks have no other
> dependency on each other in either direction, so the swap is safe.

**Files:**
- Modify: `apps/wield/src-tauri/src/layer_shell.rs`
- Modify: `apps/wield/src-tauri/src/palette.rs:49-51` (the one existing call site)

**Interfaces:**
- Consumes: nothing new.
- Produces: `layer_shell::force_commit(window: &gtk::Window)` (signature changed from `&gtk::ApplicationWindow`) — Task 2's `AppState::show_click_catcher` and Task 3's click-catcher creation code both depend on this exact signature, since the catcher is a plain `gtk::Window`, not an `ApplicationWindow`.

The catcher is the same class of gtk-layer-shell surface as the palette (both `wlr-layer-shell` overlays created via the same, already-unmaintained bindings) and so is just as exposed to the non-rendering flake `force_commit` exists to work around (spec §4.1) — cheap and proven to apply defensively, but it currently only accepts `&gtk::ApplicationWindow`, and the catcher is a plain `gtk::Window`. The function's own body already immediately converts its argument to `&gtk::Window` internally (`let window: &gtk::Window = window.as_ref();`), so this is a widening of the parameter type to what the function already needed internally, not a behavior change.

- [ ] **Step 1: Change the signature and remove the now-redundant internal conversion**

In `apps/wield/src-tauri/src/layer_shell.rs`, change:

```rust
pub fn force_commit(window: &gtk::ApplicationWindow) {
    use gtk::glib::translate::ToGlibPtr;
    let window: &gtk::Window = window.as_ref();
    if !window.is_layer_window() {
```

to:

```rust
pub fn force_commit(window: &gtk::Window) {
    use gtk::glib::translate::ToGlibPtr;
    if !window.is_layer_window() {
```

(the rest of the function body is unchanged — `window.is_layer_window()` and `window.to_glib_none().0` both already work directly on `&gtk::Window`).

Update the function's doc comment (currently describes taking a window "that was never turned into a layer surface"; no wording about `ApplicationWindow` specifically needs to change, so check it still reads correctly — it does, since it never named the concrete type).

- [ ] **Step 2: Update the one existing call site**

In `apps/wield/src-tauri/src/palette.rs`, in `show()`:

```rust
    if let Err(error) = window.run_on_main_thread(move || match for_main_thread.gtk_window() {
        Ok(gtk_window) => crate::layer_shell::force_commit(&gtk_window),
        Err(error) => tracing::warn!(%error, "could not get GTK handle to force-commit palette"),
    }) {
```

becomes:

```rust
    if let Err(error) = window.run_on_main_thread(move || match for_main_thread.gtk_window() {
        Ok(gtk_window) => {
            let gtk_window: &gtk::Window = gtk_window.as_ref();
            crate::layer_shell::force_commit(gtk_window)
        }
        Err(error) => tracing::warn!(%error, "could not get GTK handle to force-commit palette"),
    }) {
```

- [ ] **Step 3: Run the workspace build to confirm both changes compile together**

Run: `cargo build --release -p wield-app`
Expected: builds clean, no type errors.

- [ ] **Step 4: Run layer_shell.rs's existing test to confirm no regression**

Run: `cd apps/wield/src-tauri && cargo test layer_shell::`
Expected: `is_available_never_panics_without_a_display` passes (unchanged — this task doesn't touch `is_available`).

- [ ] **Step 5: Commit**

```bash
git add apps/wield/src-tauri/src/layer_shell.rs apps/wield/src-tauri/src/palette.rs
git commit -m "refactor(layer-shell): widen force_commit to accept any gtk::Window"
```

---

### Task 2: `AppState` plumbing for a thread-confined GTK handle

**Files:**
- Modify: `apps/wield/src-tauri/src/state.rs`

**Interfaces:**
- Consumes: `layer_shell::force_commit(&gtk::Window)` (Task 1).
- Produces: `AppState::set_click_catcher(&self, window: gtk::Window)`, `AppState::show_click_catcher(&self)`, `AppState::hide_click_catcher(&self)` — all three must only be called from the GTK main thread (documented on each). Later tasks depend on exactly these three names and signatures.

`AppState` must stay `Send + Sync` (Tauri's `.manage()`/`State<T>` require it), but a raw `gtk::Window` is not `Send`. A small wrapper that asserts `Send` — sound only because every access in this codebase is already disciplined to the GTK main thread via `run_on_main_thread` — resolves this the same way this codebase already accepts other "unsafe, but the invariant is upheld everywhere it's used" tradeoffs (see `layer_shell::force_commit`'s hand-written `extern "C"` block for the precedent).

`show_click_catcher` also calls `force_commit` right after showing the window, mirroring exactly how `palette::show()` already does — the catcher is the same class of gtk-layer-shell surface, just as exposed to the non-rendering flake `force_commit` exists to work around (spec §4.1), and it's cheap enough to apply defensively regardless.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module at the bottom of `apps/wield/src-tauri/src/state.rs`:

```rust
    #[test]
    fn click_catcher_show_and_hide_are_safe_noops_before_its_set() {
        let state = empty_state();
        // Nothing was ever created (mirrors the X11/GNOME fallback path,
        // or the brief startup window before setup() runs) - these must
        // not panic.
        state.show_click_catcher();
        state.hide_click_catcher();
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd apps/wield/src-tauri && cargo test click_catcher_show_and_hide_are_safe_noops_before_its_set`
Expected: FAIL to compile — `show_click_catcher`/`hide_click_catcher` don't exist yet.

- [ ] **Step 3: Add the wrapper type and the `click_catcher` field**

At the top of `apps/wield/src-tauri/src/state.rs`, after the existing `use` block:

```rust
/// Wraps a GTK object so it can live behind `AppState`'s `Send + Sync`
/// bound (required by Tauri's `.manage()`). Actually touching the inner
/// value is only sound from the GTK main thread - every access in this
/// codebase already goes through `WebviewWindow::run_on_main_thread`
/// first before touching a raw GTK object for exactly this reason (see
/// `palette::show`/`force_commit`); `AppState::show_click_catcher` and
/// `hide_click_catcher` below carry that same requirement in their own
/// doc comments rather than re-deriving thread-safety here.
struct MainThreadOnly<T>(T);

// SAFETY: `T` (a raw GTK object) is not actually `Send`. This is sound
// only because nothing in this codebase ever touches the wrapped value
// except from the GTK main thread - see the struct's own doc comment.
unsafe impl<T> Send for MainThreadOnly<T> {}
```

Add the field to `AppState`, right after `resize_generation`:

```rust
    /// The full-screen, invisible layer-shell surface that dismisses the
    /// palette on an outside click (see click_catcher.rs). `None` until
    /// `click_catcher::create_and_register` runs during setup, and always
    /// `None` on X11/GNOME (layer-shell unavailable) - every access must
    /// degrade gracefully rather than panic.
    click_catcher: Mutex<Option<MainThreadOnly<gtk::Window>>>,
```

Initialize it to `Mutex::new(None)` in `AppState::build()`, `AppState::for_test()`, and the `empty_state()` test helper (three places, matching how `hotkey_controller` is already initialized in all three).

- [ ] **Step 4: Add the three methods**

Add to `impl AppState`, after `set_hotkey_controller`:

```rust
    /// Registers the click-catcher window created during setup. Called
    /// once, from `click_catcher::create_and_register`, which already
    /// runs on the GTK main thread (inside Tauri's `setup()` closure).
    pub fn set_click_catcher(&self, window: gtk::Window) {
        *self
            .click_catcher
            .lock()
            .expect("click catcher lock") = Some(MainThreadOnly(window));
    }

    /// Shows the click-catcher alongside the palette, and force-commits
    /// it (see layer_shell::force_commit) exactly like palette::show()
    /// does for the palette itself - the same class of surface, exposed
    /// to the same flake. A no-op if it was never created (X11/GNOME) or
    /// hasn't been registered yet. Must only be called from the GTK main
    /// thread - see `MainThreadOnly`.
    pub fn show_click_catcher(&self) {
        use gtk::prelude::WidgetExt;
        if let Some(MainThreadOnly(window)) =
            self.click_catcher.lock().expect("click catcher lock").as_ref()
        {
            window.show();
            crate::layer_shell::force_commit(window);
        }
    }

    /// Hides the click-catcher alongside the palette. Same no-op and
    /// main-thread requirements as `show_click_catcher`.
    pub fn hide_click_catcher(&self) {
        use gtk::prelude::WidgetExt;
        if let Some(MainThreadOnly(window)) =
            self.click_catcher.lock().expect("click catcher lock").as_ref()
        {
            window.hide();
        }
    }
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cd apps/wield/src-tauri && cargo test click_catcher_show_and_hide_are_safe_noops_before_its_set`
Expected: PASS.

- [ ] **Step 6: Run the full existing state.rs test suite to check nothing else broke**

Run: `cd apps/wield/src-tauri && cargo test --lib state::`
Expected: all pass, including the pre-existing `build_produces_the_builtin_registry`, `cancel_run_toggles_a_registered_token`, `configure_hotkey_errors_with_no_active_session`, `hotkey_state_defaults_pending_and_is_settable`.

- [ ] **Step 7: Commit**

```bash
git add apps/wield/src-tauri/src/state.rs
git commit -m "feat(state): add thread-confined storage for the click-catcher window"
```

---

### Task 3: `click_catcher.rs` — the surface itself

**Files:**
- Create: `apps/wield/src-tauri/src/click_catcher.rs`
- Modify: `apps/wield/src-tauri/src/lib.rs` (add `mod click_catcher;` to the module list — the `create_and_register` call itself is wired in Task 5, not here, to keep this task's deliverable to "the module compiles and its own gate is tested")

**Interfaces:**
- Consumes: `layer_shell::is_available()` (existing), `layer_shell::force_commit(&gtk::Window)` (Task 1), `AppState::set_click_catcher` (Task 2).
- Produces: `click_catcher::create_and_register(app: &tauri::AppHandle)` — Task 5 depends on this exact name and signature.

- [ ] **Step 1: Write the module with its one gating test**

Create `apps/wield/src-tauri/src/click_catcher.rs`:

```rust
//! A full-screen, invisible layer-shell surface shown behind the palette.
//! Its only job is to dismiss the palette when something outside it is
//! clicked - see docs/superpowers/specs/2026-09-13-wield-click-catcher-design.md
//! for why this exists and why it's a plain GTK window rather than a
//! second Tauri/webview window.

use gtk::prelude::{GtkWindowExt, WidgetExt};
use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use tauri::{AppHandle, Manager};

/// Creates the click-catcher and registers it with `AppState`. Call once,
/// during Tauri's `setup()`, after the palette's own layer-shell
/// configuration and before anything that might show the palette (see
/// lib.rs). A no-op if layer-shell isn't available (X11/GNOME) - the
/// same fail-closed gate the palette's own positioning already uses, so
/// `AppState::show_click_catcher`/`hide_click_catcher` staying no-ops in
/// that case is the *intended* fallback, not a bug to fix later.
pub fn create_and_register(app: &AppHandle) {
    if !crate::layer_shell::is_available() {
        return;
    }

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_decorated(false);

    window.init_layer_shell();
    // A distinct namespace, not gtk-layer-shell's own default - continuing
    // the convention adopted after the palette was found silently
    // inheriting a compositor rule meant for the generic
    // "gtk-layer-shell"-namespaced surfaces (docs/testing.md).
    window.set_namespace("wield-click-catcher");
    // Layer::Top, not Overlay: the palette stays on Overlay, which always
    // composites above Top regardless of map/show ordering - this
    // guarantees the palette is never visually covered by the catcher
    // without depending on which surface was shown first.
    window.set_layer(Layer::Top);
    window.set_keyboard_mode(KeyboardMode::None);
    // Anchored to all four edges: covers the entire output. This is the
    // one case in this codebase where anchoring opposite edges is
    // correct - layer_shell::configure's own doc comment warns against
    // it for the palette specifically because it makes GTK ignore the
    // surface's requested size, which is exactly the effect wanted here.
    // No explicit output/monitor is set here, so this targets the same
    // default (compositor-focused) output the palette itself already
    // relies on for the same reason - the two stay in sync without
    // either one querying monitor geometry (spec §3.5).
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        window.set_anchor(edge, true);
    }

    // Fully transparent: an RGBA visual so the surface's Wayland buffer
    // actually supports per-pixel alpha, app_paintable so GTK doesn't
    // paint its own opaque theme background over that, and a draw
    // handler that explicitly clears to zero alpha - without all three,
    // an empty GTK window still renders as an opaque rectangle, which
    // here would mean an opaque black rectangle covering the whole
    // screen instead of being invisible.
    if let Some(screen) = window.screen() {
        if let Some(visual) = screen.rgba_visual() {
            window.set_visual(Some(&visual));
        }
    }
    window.set_app_paintable(true);
    window.connect_draw(|_widget, cr| {
        use gtk::cairo::Operator;
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        cr.set_operator(Operator::Source);
        let _ = cr.paint();
        gtk::glib::Propagation::Proceed
    });

    window.add_events(gtk::gdk::EventMask::BUTTON_PRESS_MASK);
    let app_for_click = app.clone();
    window.connect_button_press_event(move |_widget, _event| {
        // This handler runs on the GTK main thread (all GTK signal
        // handlers do), so calling straight into palette::hide() here -
        // which itself dispatches its own GTK-touching work via
        // run_on_main_thread - is safe regardless of which thread
        // *originally* triggered this click.
        crate::palette::hide(&app_for_click);
        gtk::glib::Propagation::Stop
    });

    app.state::<crate::state::AppState>()
        .set_click_catcher(window);
}

#[cfg(test)]
mod tests {
    use super::*;

    // Creating a real GTK window needs a live display, which this test
    // suite doesn't have (no Wayland/X11 in CI) - this only checks that
    // the fail-closed gate itself is safe to call without one, matching
    // layer_shell.rs's own is_available_never_panics_without_a_display
    // test in spirit. The window-creation path itself is exercised by
    // live verification only (Task 6), same as the palette's own
    // layer-shell setup.
    #[test]
    fn is_available_gate_never_panics_without_a_display() {
        let _ = crate::layer_shell::is_available();
    }
}
```

- [ ] **Step 2: Register the module in lib.rs**

In `apps/wield/src-tauri/src/lib.rs`, add to the module declarations (near `mod layer_shell;`):

```rust
mod click_catcher;
```

- [ ] **Step 3: Run the new test**

Run: `cd apps/wield/src-tauri && cargo test click_catcher::`
Expected: `is_available_gate_never_panics_without_a_display` passes.

- [ ] **Step 4: Run the full build**

Run: `cargo build --release -p wield-app`
Expected: builds clean. `create_and_register` is unused at this point (Task 5 wires the call) - if the compiler warns about that, add `#[allow(dead_code)]` temporarily above the function and remove it in Task 5 once it's called; do not silence the warning any other way.

- [ ] **Step 5: Commit**

```bash
git add apps/wield/src-tauri/src/click_catcher.rs apps/wield/src-tauri/src/lib.rs
git commit -m "feat(click-catcher): add the full-screen invisible catcher surface"
```

---

### Task 4: Wire the catcher into `palette::show()`/`hide()`

**Files:**
- Modify: `apps/wield/src-tauri/src/palette.rs`

**Interfaces:**
- Consumes: `AppState::show_click_catcher()`, `AppState::hide_click_catcher()` (Task 2).
- Produces: nothing new — `palette::show`/`hide`'s existing signatures are unchanged, only their bodies grow.

`hide()` currently has no `run_on_main_thread` dispatch at all (Tauri's own `WebviewWindow::hide()` handles its own thread-safety) - it needs one now, since it's about to also touch a raw GTK object (the catcher) via `AppState`.

- [ ] **Step 1: Extend `show()` to also show the catcher**

In `apps/wield/src-tauri/src/palette.rs`, Task 1 left the `run_on_main_thread` closure in `show()` looking like this:

```rust
    let for_main_thread = window.clone();
    if let Err(error) = window.run_on_main_thread(move || match for_main_thread.gtk_window() {
        Ok(gtk_window) => {
            let gtk_window: &gtk::Window = gtk_window.as_ref();
            crate::layer_shell::force_commit(gtk_window)
        }
        Err(error) => tracing::warn!(%error, "could not get GTK handle to force-commit palette"),
    }) {
        tracing::warn!(%error, "failed to dispatch palette force-commit to the main thread");
    }
```

This becomes (the app handle captured alongside the window, and a new line added after the `match` to also show the catcher — note the `match` is no longer the closure's tail expression, so its arms now end in `;` rather than `,`/no punctuation):

```rust
    let for_main_thread = window.clone();
    let app_for_main_thread = app.clone();
    if let Err(error) = window.run_on_main_thread(move || {
        match for_main_thread.gtk_window() {
            Ok(gtk_window) => {
                let gtk_window: &gtk::Window = gtk_window.as_ref();
                crate::layer_shell::force_commit(gtk_window);
            }
            Err(error) => {
                tracing::warn!(%error, "could not get GTK handle to force-commit palette")
            }
        }
        app_for_main_thread
            .state::<crate::state::AppState>()
            .show_click_catcher();
    }) {
        tracing::warn!(%error, "failed to dispatch palette force-commit to the main thread");
    }
```

`palette.rs` already imports `tauri::Manager` (`use tauri::{AppHandle, Manager, WebviewWindow};`), which is what `app.state::<T>()` requires — no import changes needed for this step.

- [ ] **Step 2: Extend `hide()` to also hide the catcher**

Current:

```rust
pub fn hide(app: &AppHandle) {
    if let Some(window) = window(app) {
        if let Err(error) = window.hide() {
            tracing::warn!(%error, "failed to hide palette");
        }
    }
}
```

becomes:

```rust
pub fn hide(app: &AppHandle) {
    let Some(window) = window(app) else {
        return;
    };
    if let Err(error) = window.hide() {
        tracing::warn!(%error, "failed to hide palette");
    }
    // Dispatched via run_on_main_thread: hide() is called from the
    // click-catcher's own GTK signal handler (already on the main
    // thread - harmless to dispatch again from there) but also from the
    // D-Bus ShowPalette-adjacent paths and the frontend's Escape/blur
    // handlers, which are not guaranteed to be - same reasoning as
    // show()'s existing dispatch above.
    let app_for_main_thread = app.clone();
    if let Err(error) = window.run_on_main_thread(move || {
        app_for_main_thread
            .state::<crate::state::AppState>()
            .hide_click_catcher();
    }) {
        tracing::warn!(%error, "failed to dispatch click-catcher hide to the main thread");
    }
}
```

- [ ] **Step 3: Run the full build**

Run: `cargo build --release -p wield-app`
Expected: builds clean.

- [ ] **Step 4: Run the full workspace test suite**

Run: `cargo test --workspace -- --test-threads=1`
Expected: all pass (this task doesn't add new automated tests of its own — `show`/`hide`'s GTK-touching behavior was never unit-tested before this change either, matching this project's established convention that this class of behavior is live-verified, not unit-tested; Task 6 is where this task's actual behavior gets checked).

- [ ] **Step 5: Commit**

```bash
git add apps/wield/src-tauri/src/palette.rs
git commit -m "feat(palette): show/hide the click-catcher alongside the palette"
```

---

### Task 5: Create the catcher at startup

**Files:**
- Modify: `apps/wield/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `click_catcher::create_and_register(&AppHandle)` (Task 3).

Must run after the palette's own layer-shell configuration (so `click_catcher::create_and_register`'s internal `layer_shell::is_available()` check and the palette's setup don't race on whatever one-time Wayland-registry probe `is_available()` performs) and before `setup_shell.bind(handle.clone())` (which can service a pending D-Bus `ShowPalette` request immediately - the existing comment above that call already documents this exact ordering requirement for the palette itself; the catcher needs to exist before that same call for the same reason, since `palette::show()` now also tries to show the catcher).

- [ ] **Step 1: Add the call in the correct position**

In `apps/wield/src-tauri/src/lib.rs`, inside the `setup()` closure, immediately after the existing `if layer_shell::is_available() { ... } else { ... }` block that configures the palette (i.e., right before the comment `// Binding can service a pending D-Bus request...`), add:

```rust
            click_catcher::create_and_register(&handle);

```

- [ ] **Step 2: Remove the temporary `#[allow(dead_code)]` from Task 3 if it was added**

Confirm `click_catcher::create_and_register` is now called; delete the `#[allow(dead_code)]` attribute above it in `click_catcher.rs` if Step 4 of Task 3 needed one.

- [ ] **Step 3: Run the full build**

Run: `npm run build -w apps/wield -- --no-bundle`
Expected: builds clean (this is the full Tauri production build, not `cargo build` directly — required so the webview loads embedded assets correctly rather than trying a dev server, per this project's established build discipline).

- [ ] **Step 4: Run the full workspace test suite one more time**

Run: `cargo test --workspace -- --test-threads=1`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add apps/wield/src-tauri/src/lib.rs
git commit -m "feat(lib): create the click-catcher during setup"
```

---

### Task 6: Live verification and `docs/testing.md`

**Files:**
- Modify: `docs/testing.md`

No new code in this task — installing the build from Task 5 and verifying the actual behavior live, the only way this class of feature can be checked, matching this project's established convention (GlobalShortcuts portal binding, tray registration, every layer-shell fix so far were all verified this same way).

- [ ] **Step 1: Install and launch the build from Task 5**

```bash
pkill -f wield-app
cp target/release/wield-app ~/.local/bin/wield-app
nohup ~/.local/bin/wield-app > /tmp/wield-click-catcher-test.log 2>&1 &
disown
```

- [ ] **Step 2: Confirm the catcher surface actually exists and is transparent**

Show the palette (`busctl --user call io.github.DhanushSantosh.Wield /io/github/DhanushSantosh/Wield io.github.DhanushSantosh.Wield ShowPalette`), then check `hyprctl layers -j` for a `wield-click-catcher` namespace entry covering the full output, alpha reported as expected (mapped, not the alpha-stuck-at-0 flake — if it is stuck, `force_commit` isn't working for this surface and needs investigation before continuing). Take a `grim` screenshot and visually confirm nothing behind the palette looks different from before this feature existed (proving it's actually invisible, not an accidental grey/black overlay).

- [ ] **Step 3: Verify the actual fix — click the previously-active window**

With a specific window already focused (e.g. a terminal or browser), show the palette, then send a synthetic click at a point clearly within that window and outside the palette's own bounds (space press/release events with realistic ~100ms gaps between them, matching the timing lesson already learned this session about `ydotool`). Confirm via `hyprctl layers -j` that the palette (and catcher) are no longer mapped afterward.

- [ ] **Step 4: Regression check — a different, previously-unfocused window/monitor**

Repeat with a click on a genuinely different app on a different monitor. Confirm this still dismisses the palette (it did before this feature, via the existing blur listener) — this task should not change that path's behavior, only add coverage for the case it didn't already handle.

- [ ] **Step 5: Confirm clicking inside the palette is unaffected**

Show the palette, click on a result row inside it (e.g. "Pick a colour"). Confirm the palette does **not** dismiss and normal interaction (row selection / activation) still works exactly as before.

- [ ] **Step 6: A fresh-launch stress test for the catcher's own rendering**

Repeat steps 1-2 across 5 fresh launches (kill, relaunch, show, check `alpha`), matching the stress-test discipline used for the palette's own rendering-flake fix - confirms `force_commit` is doing its job for this surface too, not just working by luck on the first try.

- [ ] **Step 7: Write up the results in `docs/testing.md`**

Add a new section following this project's existing convention (what was tried, what was confirmed, any false leads or surprises along the way — not just the final "it works"). Include the exact verification steps and their outcomes from Steps 2-6 above.

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
git commit -m "docs(testing): verify the click-catcher outside-click dismiss live"
```

---

## After this plan

Push the branch and open a PR against `master`, following this project's established pattern (PR description summarizing the fix, linking the spec, noting live verification results) — do not merge without the project owner's go-ahead, matching how every previous PR in this project has been handled.
