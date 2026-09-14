# Wield — Outside-Click Dismiss ("Click-Catcher") Design

## 1. Problem

`KeyboardMode::Exclusive` (see `docs/superpowers/specs/2026-09-12-wield-wayland-layer-shell-design.md` and `docs/testing.md`) is currently required for the palette to work as a command palette at all: `KeyboardMode::OnDemand` never actually grants this surface keyboard focus on this Hyprland version (confirmed live, matches upstream `hyprwm/Hyprland#2264`), so without `Exclusive` neither typing into the search box nor Escape-to-hide would work.

Live-tested during real usage, this has a real cost beyond the already-documented "click-away doesn't dismiss": clicking on **whatever window was already active before the palette was summoned** does nothing at all — reported directly ("i couldnt register a click at all couldnt even click the main foreground window") and reproduced. This is the single most natural thing a user tries.

A more precise live test narrowed the actual gap. Two cases were checked with the palette shown:

1. Clicking a window that was **not** already focused (a different app, on a different monitor) — **worked**: Hyprland's own focus tracking changed, the palette's existing JS `blur` listener (`getBlurToHide()` → `hidePalette()`) fired, and the palette hid correctly.
2. Clicking the window that was **already** the active one before the palette opened — **did nothing**: no focus transition occurs from Hyprland's point of view (it was never "unfocused" to begin with), so no `blur` event fires for us either.

So the existing blur-to-hide mechanism already covers "switch to some other app." The actual gap is narrower and specific: dismissing the palette by clicking back onto whatever was open underneath it.

## 2. Scope

- **In scope:** clicking anywhere outside the palette's own visible bounds, while it's shown, dismisses it — regardless of whether the target window was already focused.
- **Explicitly out of scope (confirmed with the project owner):** click-through. The click that dismisses the palette does **not** need to also land on the window underneath in the same gesture — "click closes it, click again to interact" is an accepted, deliberate simplification, not a gap to close later.
- **Out of scope:** changing `KeyboardMode::Exclusive` on the palette itself, or anything about how typing/Escape currently work. This design is purely additive.
- **Out of scope:** X11/GNOME fallback (no layer-shell available) — this feature only exists where layer-shell does, gated the same way the palette's own positioning already is.

## 3. Architecture

### 3.1 Why a second surface, not a second detection mechanism

Two alternatives were considered and rejected before this one:

- **Hyprland IPC event subscription** (listen for `activewindow` on Hyprland's own event socket, hide on any change). Rejected: this has the *exact same blind spot* as the existing JS blur listener — it only fires on a real focus transition, which is precisely the case that doesn't happen for "click the already-focused window." It would add a whole new IPC-subscription code path for zero actual improvement over today.
- **The `InputCapture` portal** (globally observe pointer clicks via XDG portal, compositor-agnostic in principle). Rejected for now: this portal is designed for a different use case (zone-based capture-and-forward, e.g. a virtual KVM), and its exact behavior for "passively observe global clicks without capturing input" is unverified. Given this exact session already found this system's `xdg-desktop-portal-hyprland` silently ignoring part of the `GlobalShortcuts` portal's contract (the `preferred_trigger` field), trusting a second, more exotic portal capability on the same backend without a feasibility spike first would be repeating a mistake this project just paid down.

The chosen approach instead uses a technique already proven by other Wayland launchers for exactly this problem (Rofi, Wofi, GNOME Shell's overview all use some form of it): a second, fully invisible surface that sits behind the palette and covers the rest of the screen, whose only job is "something clicked me → hide the palette." It needs no compositor-specific IPC and no unverified portal capability — just `wlr-layer-shell` primitives already in use.

### 3.2 Why a raw GTK window, not a second Tauri window

The catcher has **zero content** — no HTML, no CSS, no JS, nothing a user ever looks at. Giving it a full Tauri-managed webview window (a second `tauri.conf.json` entry, a second webview process, a second React root) would be paying the same cost this project just removed when Preferences was folded into the palette (see `docs/testing.md`'s "second window means a second copy of every fix" lesson) — for a component that doesn't need a webview at all.

Instead, it's a plain `gtk::ApplicationWindow`, created directly via the `gtk` crate already in `apps/wield/src-tauri/Cargo.toml`, configured via `gtk-layer-shell` exactly like the palette's own window, with a native GTK signal handler — no IPC round-trip, since the handler runs in the same process and can call straight into `palette::hide()`.

### 3.3 Configuration

Via `gtk-layer-shell` (reusing `layer_shell::configure`'s existing patterns, not a new API):

- **Anchored to all four edges**, 0 margins → covers the entire output. (This is the one case in this codebase where anchoring opposite edges is correct — `layer_shell::configure`'s existing doc comment warns against it for the palette specifically because an anchored-both-ways surface has its size request ignored; that's exactly the effect wanted here, since a full-screen click-catcher doesn't have a size to preserve.)
- **`Layer::Top`**, not `Overlay`. The palette itself stays on `Layer::Overlay`, which always composites above `Layer::Top` regardless of map/show ordering — this guarantees correct z-order (palette visually on top of the catcher) without depending on which surface was shown first, avoiding a race that a same-layer approach would need map-order tricks to avoid.
- **`KeyboardMode::None`** — it never receives keyboard input, and requesting more would be pointless (and , per the whole `Exclusive`-tradeoff investigation this session, actively risk re-introducing the same "blocks everything else" problem this feature exists to work around).
- **Fully transparent** — same `transparent: true`-equivalent treatment already applied to the palette (a raw GTK RGBA visual, no `tauri.conf.json` involved since this isn't a Tauri window).
- **Its own namespace**, `wield-click-catcher` — continuing the per-window-namespace convention adopted after the palette was found silently inheriting an unrelated compositor blur/opacity rule meant for generic `gtk-layer-shell`-named surfaces.
- **Gated by `layer_shell::is_available()`**, the same fail-closed check already used for the palette and Preferences-as-a-view. On X11/GNOME, the catcher is never created — no partial or broken state, matching this project's established fallback discipline.

### 3.4 Lifecycle and data flow

- Created once, at startup, in the same `setup()` closure that configures the palette's own layer-shell surface — after the palette, so its existence never affects palette configuration.
- Its handle is stored in `AppState` (mirroring how `hotkey_controller` is already stored there), so `palette::show()`/`palette::hide()` can reach it without a new inter-module dependency.
- `palette::show()`: after showing the palette itself, also shows the catcher.
- `palette::hide()`: after hiding the palette itself, also hides the catcher. This runs regardless of *why* `hide()` was called — Escape, the tray, the existing blur listener, or the catcher's own click — so the two surfaces can never end up out of sync.
- The catcher's `button-press-event` handler calls `palette::hide()` directly.

Click routing is then just ordinary compositor hit-testing, nothing bespoke: a click within the palette's own visible bounds hits the palette (topmost there) exactly as today; a click anywhere else on that output hits the catcher (since it covers everything the palette doesn't), which hides both.

### 3.5 Multi-monitor

The catcher only needs to cover the **same output the palette lands on** — gtk-layer-shell's "no explicit output" default targets the compositor's focused monitor, the same default the palette itself already relies on, so the two naturally stay in sync without either one querying monitor geometry. Clicking a window on a genuinely *different*, unfocused monitor is already handled today by the existing blur listener (verified live, §1) — this design isn't duplicating that path, only filling the specific gap it doesn't cover.

## 4. Risks and honest gaps

1. **A second layer-shell surface is a second thing that can hit the same bug class already found this session** (rendering flakes, the `resizable: true` alpha-stuck-at-0 bug, transparent-corner artifacts). Mitigated by scope: the catcher is about as simple as a layer-shell surface can be — no resize, no keyboard, no content — so most of that bug class (which centered on resize behavior and content rendering) has no surface to occur on here. Still worth a real fresh-launch verification pass, not just "it compiled."
2. **z-order correctness (`Overlay` above `Top`) is asserted from gtk-layer-shell's documented layer ordering, not independently re-derived here.** If wrong, the failure mode is visible and severe (catcher would cover the palette) rather than silent, so it will be obvious immediately in live testing rather than shipping unnoticed.
3. **This only covers the same-output case by relying on gtk-layer-shell's default-output behavior matching the palette's.** If a future change gives the palette an explicit output assignment (e.g., "always open on the primary monitor" as its own feature), the catcher's configuration would need the same explicit assignment to stay in sync — not a problem today, but a coupling worth remembering if that assumption changes.
4. **No code-level fix for X11/GNOME**, consistent with every other layer-shell-dependent feature in this project — named, not silently absent.

## 5. Testing strategy

Consistent with this project's established pattern for compositor-boundary code: this is not meaningfully unit-testable (no fake Wayland compositor in this test suite). What's testable and will be:

- Pure/structural pieces if any emerge during implementation (e.g., namespace string construction), matching the existing `is_available_never_panics_without_a_display`-style tests.
- Live verification, the same toolkit already used all session (`hyprctl layers`/`activewindow`, `grim`, synthetic input with realistic timing): palette shown, click the previously-focused window behind it → palette hides; click a different app on a different monitor → existing blur-based hide still works (regression check); click within the palette itself → nothing happens, normal interaction continues; a fresh-launch stress test for rendering, matching the discipline used for the palette's own rendering-flake fix.
- Documented in `docs/testing.md` honestly — including anything that doesn't work as designed on the first attempt, matching this project's existing convention of writing up false leads, not just the final answer.

## 6. Repo artifacts

- New `apps/wield/src-tauri/src/click_catcher.rs`: creation, layer-shell configuration, and the click handler, isolated from `palette.rs`/`layer_shell.rs` so its narrow, single-purpose surface has a correspondingly narrow, single-purpose module.
- `apps/wield/src-tauri/src/state.rs`: new field holding the catcher's window handle.
- `apps/wield/src-tauri/src/palette.rs`: `show()`/`hide()` extended to also show/hide the catcher.
- `docs/testing.md`: new section documenting live verification, matching existing convention.
