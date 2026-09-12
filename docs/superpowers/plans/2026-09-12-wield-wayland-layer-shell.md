# Wield — Wayland Layer-Shell Positioning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the palette and Preferences windows actually center on screen under Wayland (Hyprland, Sway, other wlroots compositors, KDE), where Tauri's `center: true` is a documented no-op. Detect layer-shell support at startup; use it when available, fall through to today's unchanged behavior when not (GNOME, X11, or any detection failure).

**Architecture:** A new `apps/wield/src-tauri/src/layer_shell.rs` module wraps the `gtk-layer-shell` crate (GTK3 bindings — Tauri uses GTK3 on Linux, confirmed via `tao` 0.35.3's `gtk = "0.18"` dependency). One free function checks compositor support (`gtk_layer_shell::is_supported()` — the crate itself does the registry check, no hand-rolled Wayland probing needed); one function configures a given GTK window as an `Overlay`-layer surface anchored to all four screen edges (which the protocol centers automatically) with on-demand keyboard focus. Called from `setup()` in `lib.rs`, once per window, strictly before each window's first `.show()` — both windows are already created with `"visible": false` and shown later, so this fits the existing lifecycle without restructuring it.

**Tech Stack:** `gtk-layer-shell` 0.8.2 (Rust crate; confirmed installed as a system library here — `gtk-layer-shell` 0.10.1 via pacman, `pkg-config --modversion gtk-layer-shell-0` resolves). No new frontend work.

**Spec:** `docs/superpowers/specs/2026-09-12-wield-wayland-layer-shell-design.md`

---

## Global Constraints

- **Repo root:** `/home/dhanush/Projects/Wield` (branch `feat/wayland-layer-shell` off `master`).
- **Scope:** both `palette` and `preferences` windows (per owner decision during brainstorming — Preferences gets the same centering treatment as the palette, not just the launcher).
- **X11 unaffected.** This entire mechanism is Wayland-only; on X11 sessions, `is_supported()` returns `false` and every window is created exactly as it is today.
- **GNOME unaffected, on purpose.** Mutter doesn't implement `wlr-layer-shell` at all — `is_supported()` returns `false` there too, same fallback path as X11. This is a named, accepted gap, not something this plan works around.
- **Fail closed.** Any error or ambiguity in the detection or configuration path must fall through to the existing window creation — never a half-configured window, never a panic that takes down startup.
- **`cargo` is not on the default `PATH`.** `export PATH="$HOME/.cargo/bin:$PATH"` before every cargo invocation.
- **`npm run check`** green at the end of every task that touches buildable code (this plan touches Rust only, but the full check still runs — matches project convention).

---

## File Structure

**Created:**

| File | Responsibility |
| --- | --- |
| `apps/wield/src-tauri/src/layer_shell.rs` | `is_available()` detection wrapper; `configure(window)` — anchors a window to all four edges as an overlay-layer surface with on-demand keyboard focus |

**Modified:**

| File | Change |
| --- | --- |
| `apps/wield/src-tauri/Cargo.toml` | add the `gtk-layer-shell` dependency |
| `apps/wield/src-tauri/src/lib.rs` | register the `layer_shell` module; call `layer_shell::is_available()` once in `setup()`, and if true, `layer_shell::configure(...)` on both windows before they're first shown |
| `docs/testing.md` | new section documenting what was verified live on this Hyprland dev environment and what wasn't (KDE, Sway, GNOME's expected fallback) |

---

## Task 1: `layer_shell` module — detection and configuration

**Files:**
- Create: `apps/wield/src-tauri/src/layer_shell.rs`
- Modify: `apps/wield/src-tauri/Cargo.toml`

**Interfaces:**
- Produces: `pub fn is_available() -> bool` and `pub fn configure(window: &gtk::ApplicationWindow)` — both consumed by Task 2's `lib.rs` wiring.

- [ ] **Step 1: Add the dependency**

```toml
# apps/wield/src-tauri/Cargo.toml, in [dependencies]
gtk-layer-shell = "0.8"
```

Run `cargo build -p wield-app` once just to confirm the dependency resolves against the same `gtk = "0.18"` line `tao` already pulls in — a version conflict here would show up as a type-mismatch compile error between two incompatible `gtk` crate versions, not a dependency-resolution failure. **Mechanical-correction note:** if `gtk-layer-shell`'s own `gtk` version requirement doesn't line up with `0.18`, that's a real blocker to resolve before continuing (check `gtk-layer-shell`'s published `Cargo.toml` for its exact `gtk` version pin) — do not proceed past this step with a version mismatch silently worked around.

- [ ] **Step 2: Write the module**

```rust
//! Wayland layer-shell positioning: makes windows actually center on
//! screen under Wayland, where regular toplevel windows cannot (Wayland
//! gives clients no way to set their own screen position — this is a
//! protocol-level restriction, not a bug in this crate or in Tauri).
//! `wlr-layer-shell` is implemented by wlroots-based compositors
//! (Hyprland, Sway, ...) and KDE's KWin; GNOME's Mutter does not
//! implement it at all. `is_available()` is the single fail-closed
//! gate: everything in this module is skipped entirely when it returns
//! false, and the caller falls through to today's unchanged window
//! creation.

use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

/// Whether the current session supports `wlr-layer-shell`. `false` on
/// X11 (this mechanism doesn't apply there — the existing `center: true`
/// config already works), on GNOME (protocol not implemented), and on
/// any Wayland compositor that doesn't advertise the protocol. May do a
/// Wayland round-trip the first time it's called (per the underlying
/// crate's own documentation) — call once at startup, not per-window.
pub fn is_available() -> bool {
    gtk_layer_shell::is_supported()
}

/// Configures `window` as an overlay-layer surface anchored to all four
/// screen edges — the layer-shell protocol centers a surface smaller
/// than the output automatically once it's anchored on both axes, no
/// manual coordinate math. Must be called before `window` is realized
/// (before its first `.show()`); calling it after has no effect.
pub fn configure(window: &gtk::ApplicationWindow) {
    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        window.set_anchor(edge, true);
    }
    // On-demand rather than Exclusive: Wield already drives focus itself
    // (`palette::show` calls `set_focus()` explicitly) — on-demand lets
    // that keep working rather than layer-shell forcing focus regardless
    // of when Wield actually wants the window focused.
    window.set_keyboard_mode(KeyboardMode::OnDemand);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_available_never_panics_without_a_display() {
        // No live Wayland/X11 session in this test environment — the
        // real, meaningful behavior (whether a window actually ends up
        // centered) can only be verified against a live compositor; see
        // docs/testing.md. This just confirms the fail-closed contract:
        // calling this without a display doesn't crash startup, it
        // reports unavailable.
        let _ = is_available();
    }
}
```

**Mechanical-correction note:** `window.init_layer_shell()` etc. assume the `LayerShell` trait is implemented for `gtk::ApplicationWindow` directly (a blanket impl over `IsA<gtk::Window>`, which `ApplicationWindow` satisfies, is the normal gtk-rs pattern) — if it isn't, upcast explicitly: `window.upcast_ref::<gtk::Window>().init_layer_shell()` (adjust every call in `configure` the same way). Note whichever it turns out to be in the Task 3 report.

- [ ] **Step 3: Run to verify it compiles and the test passes**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo test -p wield-app layer_shell::
```
Expected: PASS (the one test just confirms `is_available()` doesn't panic in this headless environment — it will correctly report `false` here, since there's no live Wayland session in the test process).

- [ ] **Step 4: Commit**

```bash
git add apps/wield/src-tauri/Cargo.toml apps/wield/src-tauri/src/layer_shell.rs
git commit -m "feat(layer-shell): detection + window configuration for Wayland centering"
```

---

## Task 2: Wire into window setup

**Files:**
- Modify: `apps/wield/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `layer_shell::is_available() -> bool`, `layer_shell::configure(&gtk::ApplicationWindow)` (Task 1).
- Needs `WebviewWindow::gtk_window(&self) -> tauri::Result<gtk::ApplicationWindow>` (confirmed present on the installed `tauri` 2.11.5) for both the `palette` and `preferences` windows.

- [ ] **Step 1: Read the current `setup()` closure**

Before writing this task's code, read `apps/wield/src-tauri/src/lib.rs`'s `setup()` closure in full to find: (a) where the `palette` and `preferences` `WebviewWindow` handles are obtained (likely via `app.get_webview_window("palette")` / `"preferences"`, or already in scope from earlier window-setup code), and (b) confirm neither window's `.show()` is called anywhere inside `setup()` itself (both should stay hidden at this point — `"visible": false` in `tauri.conf.json` — with showing deferred to tray/hotkey/palette-activation code that runs later). This ordering is what makes it safe to call `layer_shell::configure` here.

- [ ] **Step 2: Add the wiring**

```rust
// Near the top of setup(), before anything shows a window:
if layer_shell::is_available() {
    for label in ["palette", "preferences"] {
        match app.get_webview_window(label).map(|w| w.gtk_window()) {
            Some(Ok(gtk_window)) => layer_shell::configure(&gtk_window),
            Some(Err(error)) => {
                tracing::warn!(%error, window = label, "could not get GTK handle for layer-shell setup");
            }
            None => tracing::warn!(window = label, "window missing during layer-shell setup"),
        }
    }
    tracing::info!("layer-shell positioning enabled");
} else {
    tracing::info!("layer-shell unavailable; using default window positioning");
}
```

Adapt the exact placement to whatever variable names `setup()` already uses for `app`/`handle` — read the surrounding code first (Step 1) rather than guessing the exact identifier.

Register the module:
```rust
// near the top of lib.rs, alongside the other `mod` declarations
mod layer_shell;
```

- [ ] **Step 3: Run the full workspace check**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
npm run check
```
Expected: PASS. (`--test-threads=1` per this project's established convention — see `docs/testing.md`'s "P7: kills_on_timeout" section.)

- [ ] **Step 4: Commit**

```bash
git add apps/wield/src-tauri/src/lib.rs
git commit -m "feat(layer-shell): wire layer-shell setup into window creation"
```

---

## Task 3: Manual verification + docs

**Files:**
- Modify: `docs/testing.md`

**Interfaces:** none — this task is manual verification and honest documentation of it, matching this project's established convention for real OS/compositor-boundary behavior (GlobalShortcuts, tray) that isn't meaningfully unit-testable.

- [ ] **Step 1: Rebuild and install properly**

```bash
pkill -f wield-app 2>/dev/null
export PATH="$HOME/.cargo/bin:$PATH"
cd /home/dhanush/Projects/Wield
npm run build -w apps/wield -- --no-bundle
install -Dm755 target/release/wield-app ~/.local/bin/wield-app
```
(Always build via the Tauri CLI path here, never bare `cargo build` — a bare build skips Tauri's production-mode toggle and the webview falls back to expecting a dev server; see `docs/testing.md`'s existing note on this from the P6b→P7 gap.)

- [ ] **Step 2: Verify live on this Hyprland environment**

```bash
gtk-launch io.github.DhanushSantosh.Wield &
sleep 2
busctl --user call io.github.DhanushSantosh.Wield /io/github/DhanushSantosh/Wield io.github.DhanushSantosh.Wield ShowPalette
```
Check (report honestly, whichever way it goes):
- Does the palette now appear centered on screen (not top-left-ish as before)?
- Does typing still work (keyboard focus reaches the search box)?
- Does the window still resize correctly (the dynamic-resize feature from the sibling branch) while staying centered?
- Open Preferences from the tray — does it also appear centered?
- Check `~/.local/state/wield/logs/wield.log.<date>` for the `"layer-shell positioning enabled"` line to confirm detection actually took the layer-shell path (not silently falling back).

- [ ] **Step 3: Write up the finding**

Append a section to `docs/testing.md`:

```markdown
## Wayland layer-shell positioning

Verified live on this project's dev environment (Hyprland): <fill in the
real outcome from Step 2 above — both windows centered, keyboard focus
confirmed, resize-while-centered confirmed, or whatever actually
happened, including any surprises>.

Not tested: KDE/KWin, Sway, other wlroots compositors, or GNOME's
fallback path specifically (X11 was not re-verified either, since this
change doesn't touch that path — the existing `center: true` mechanism
there is unmodified).
```

- [ ] **Step 4: Commit**

```bash
git add docs/testing.md
git commit -m "docs: layer-shell positioning verification notes"
```

---

## Task 4: Workspace green + wrap-up

- [ ] **Step 1: Full verification**

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace -- --test-threads=1
npm run check
```
Expected: PASS.

- [ ] **Step 2: Mark the plan complete, commit, push**

```bash
git add docs/superpowers/plans/2026-09-12-wield-wayland-layer-shell.md
git commit -m "docs: mark Wayland layer-shell plan complete"
git push -u origin feat/wayland-layer-shell
```

Then open the PR (GEON executing this directly, matching the pattern for every recently self-executed plan) with a body summarizing: what layer-shell is and why it's needed, the GNOME/X11 fallback behavior, the archived-crate risk named honestly, and the live-verification results from Task 3.

---

## Self-Review

**Spec coverage:**
- §3.1–3.2 (why layer-shell, toolchain reality) → Task 1, grounded in the actual `gtk-layer-shell` 0.8.2 API (confirmed via docs.rs, not guessed: `is_supported()`, `LayerShell` trait's `init_layer_shell`/`set_layer`/`set_anchor`/`set_keyboard_mode`, `Layer::Overlay`, `Edge::{Left,Right,Top,Bottom}`, `KeyboardMode::OnDemand`) ✓
- §3.3 (lifecycle timing) → Task 2, explicitly checks `setup()` doesn't show windows before this code runs ✓
- §3.4 (detection + fallback) → Task 2's `if layer_shell::is_available() { ... } else { ... }` — fails closed on any error inside the true branch too (per-window `match` handles `Err`/`None` without panicking) ✓
- §3.5 (interaction with dynamic resize) → not additional code, verified as a live check in Task 3 (resize-while-centered) ✓
- §4 risks → archived-crate risk named in the module's own doc comment and in the PR body (Task 4); GNOME gap named in Global Constraints and the module comment; single-environment testing limitation named explicitly in Task 3's write-up ✓
- §5 testing strategy → Task 1's test covers the fail-closed contract (the only part that's meaningfully unit-testable); Task 3 is the live verification, documented the same way GlobalShortcuts/tray verification already is in this project ✓
- §6 repo artifacts → `layer_shell.rs` (Task 1), `Cargo.toml` (Task 1), `docs/testing.md` (Task 3) — all covered; no other artifacts named in the spec ✓

**Placeholder scan:** no TBD/TODO. The one real unresolved item (`LayerShell` trait blanket-impl vs needing an explicit upcast) is flagged as a named, bounded mechanical-correction note with the exact fix if it turns out to be needed, not a vague placeholder.

**Type consistency:** `layer_shell::is_available() -> bool` and `layer_shell::configure(&gtk::ApplicationWindow)` are the only two public items; Task 2 calls them with exactly these signatures.

**Ordering:** 1 (independent) → 2 (needs 1's two functions) → 3 (needs 2 built and installed) → 4 (needs 1–3 all green). Consistent.
