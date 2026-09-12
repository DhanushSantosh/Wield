# Wield — Wayland Layer-Shell Positioning Design

## 1. Problem

Wield's palette and Preferences windows are created as ordinary GTK toplevel windows (via Tauri's default Linux windowing, which is GTK3 under the hood). `tauri.conf.json`'s `"center": true` calls into `gtk_window_move()` to center the window on screen at creation.

**This is a documented no-op under Wayland.** The Wayland protocol deliberately gives clients no way to set their own absolute screen position — only the compositor decides where a toplevel window lands. `center: true` silently does nothing on any Wayland session (confirmed by tracing the call through `tao` 0.35.3's Linux backend: `WindowRequest::Position((x, y)) => window.move_(x, y)` in `event_loop.rs`, which is GTK's `gtk_window_move()` — GTK's own documentation states this has no effect on Wayland).

Live-tested on this project's dev environment (Hyprland, a wlroots-based Wayland compositor): the palette window opens wherever the compositor's default placement puts new toplevel windows, not centered. For a command-palette-style launcher, this is a significant break from the "opens predictably, front and center" feel of Spotlight/Raycast/Alfred — and, per the standing design principle recorded 2026-09-12 (native, seamless, Mac-app-quality UX as an ongoing goal, not a one-off), this is worth fixing properly rather than accepting.

## 2. Scope

- **In scope:** both the `palette` and `preferences` windows get real, working centered positioning under Wayland on compositors that support it.
- **Out of scope:** X11 sessions are unaffected — `gtk_window_move()` already works correctly there; this design changes nothing for X11.
- **Out of scope:** GNOME. Mutter (GNOME's compositor) does not implement `wlr-layer-shell` at all — there is no code-level fix available for GNOME users. This is a named, accepted gap (see §5), not something this design attempts to route around via GNOME-Shell-extension dependencies or similar.
- **Out of scope:** any other window-manager-specific positioning hack (e.g., asking users to add a compositor config rule). Rejected as a product fix — it doesn't ship as part of Wield and puts the burden on the user.

## 3. Architecture

### 3.1 Why layer-shell is the right mechanism

`wlr-layer-shell` (`zwlr_layer_shell_v1`) is a Wayland protocol extension specifically designed for panels, launchers, notification overlays, and similar "not a regular app window" surfaces — implemented by every wlroots-based compositor (Hyprland, Sway, and others), and by KDE's KWin. It is exactly the mechanism real Linux launchers (Rofi in wayland mode, Wofi, Anyrun, fuzzel) already use to get reliable positioning that ordinary `xdg_toplevel` windows cannot achieve on Wayland.

The protocol's anchoring model directly solves centering: a layer-shell surface anchored to **all four edges** (top, bottom, left, right) with a size smaller than the output is centered on both axes by the compositor itself — no manual coordinate math, no per-monitor geometry queries.

### 3.2 Toolchain reality

Tauri on Linux uses GTK3 (`tao` 0.35.3 depends on the `gtk` crate at `0.18`, which is the GTK3 binding line — GTK4 bindings are published as the separately-named `gtk4` crate, and Tauri doesn't use them). The only Rust binding for retrofitting layer-shell onto an existing GTK window is therefore the GTK3-targeting **`gtk-layer-shell`** crate (a thin, auto-generated safe wrapper around the `gtk-layer-shell` C library). This crate's own repo is **archived/unmaintained upstream** — flagged honestly in §5, not glossed over.

`WebviewWindow::gtk_window(&self) -> tauri::Result<gtk::ApplicationWindow>` (confirmed present in the installed `tauri` 2.11.5) is the integration point: it returns the same `gtk::ApplicationWindow` type the `gtk-layer-shell` crate's `gtk_layer_init_for_window()` function expects.

### 3.3 Lifecycle timing

`gtk_layer_init_for_window()` **must be called before the GTK window is realized** (before its native Wayland surface exists) — calling it after the window has been shown once has no effect. This aligns with Wield's existing architecture: both windows are already created with `"visible": false` and shown later (the palette's pre-warmed-hidden-window pattern from P5a; Preferences is shown on demand from the tray). The layer-shell initialization hooks into Tauri's `setup()` closure, operating on each window immediately after creation and strictly before any `.show()` call — which is already how `setup()` is structured today (window creation happens there; the first `.show()` happens later, from tray-menu/hotkey/palette-activation code, not inside `setup()`).

### 3.4 Detection and fallback

At startup, before touching either window: check whether the current Wayland display advertises `zwlr_layer_shell_v1` in its registry (a global-presence check against the compositor's advertised interfaces — the same kind of capability probe `wield-portal::probe()` already does for portals, just against the Wayland registry instead of D-Bus).

- **Available** (Hyprland, Sway, other wlroots compositors, KDE): initialize both windows as layer-shell surfaces — `Overlay` layer (stays above fullscreen content, matching a launcher's expected always-on-top behavior), anchored to all four edges, keyboard interactivity enabled (layer-shell surfaces don't receive keyboard input by default — a launcher without keyboard focus is useless), sized via the same `set_size()` calls already in use (Wield's dynamic-resize feature composes with this for free: resizing a four-edge-anchored surface keeps it centered through every step).
- **Unavailable** (GNOME, X11, or the detection call itself fails for any reason): skip layer-shell entirely, fall through to exactly today's window creation path. No crash, no behavior change, no partial/broken state.

### 3.5 What doesn't change

- `alwaysOnTop`, `decorations: false`, `skipTaskbar` stay as Tauri config for the X11/fallback path — layer-shell's `Overlay` layer subsumes the always-on-top behavior on the Wayland path, but the config keys stay in place for when layer-shell isn't available.
- The dynamic palette-resize feature (`palette::animate_to_height`) is unmodified — it already only touches size, and centering under layer-shell is a property of the anchor configuration, not something the resize code needs to know about.

## 4. Risks and honest gaps

1. **`gtk-layer-shell` (the Rust crate) is unmaintained/archived upstream.** Mitigated by: it's a thin, auto-generated wrapper around a still-relevant C library targeting GTK3, which is itself a frozen API surface (no more GTK3 releases are coming) — so "unmaintained" here means "nothing needs to change," not "silently rotting." Still a real dependency-health concern to revisit if Tauri ever moves to GTK4.
2. **No code-level fix for GNOME.** Accepted, named gap — not worked around.
3. **This dev environment (Hyprland) is the only compositor this can be verified against directly.** KDE/KWin support is asserted from documentation, not tested here. Sway and other wlroots compositors are not tested either. Treated the same as this project's existing convention for portal/tray code that can't be exhaustively tested across every desktop environment: implemented correctly against the documented protocol, verified on what's actually available, gaps named rather than claimed as verified.
4. **Detection-check correctness matters a lot** — a detection false-positive (claiming layer-shell is available when it isn't) would leave the window never appearing at all, which is worse than the current "appears in the wrong place" state. The detection path needs to fail closed (any error or ambiguity → fall back to the regular window), not fail open.

## 5. Testing strategy

Consistent with this project's established pattern for real OS/compositor-boundary code (GlobalShortcuts portal binding, tray icon registration): the actual layer-shell surface creation and compositor-side anchoring behavior is not meaningfully unit-testable (there is no fake Wayland compositor in this test suite, and building one is out of scope for this fix). What's testable and will be:

- The **detection logic** itself, if it can be structured as a pure function over "what the registry reported" rather than requiring a live connection for the branch decision.
- The **fallback path never panics or partially applies** — this is the one behavior that must be bulletproof, and is checkable via a code-review-level guarantee (every fallible step degrades to "use the regular window," never to a half-configured state) plus manual verification.
- The layer-shell path itself: manual, live verification on this Hyprland dev environment (both windows actually appear centered, both resize while staying centered, both still receive keyboard input) — documented in `docs/testing.md` honestly, the same way GlobalShortcuts and tray verification are documented today.

## 6. Repo artifacts

- New `apps/wield/src-tauri/src/layer_shell.rs` module for layer-shell detection + initialization, isolated from the rest of `apps/wield/src-tauri` so the GNOME/X11 fallback path has zero surface area to break.
- `Cargo.toml`: add `gtk-layer-shell` (Linux-only target dependency, matching how `tao`/`gtk` are already scoped).
- `docs/testing.md`: new section documenting what was and wasn't verifiable, matching existing convention.
