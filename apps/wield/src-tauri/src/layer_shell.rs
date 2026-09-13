//! Wayland layer-shell positioning for launchers and utility windows.
//!
//! Regular Wayland toplevels cannot choose absolute screen coordinates.
//! `wlr-layer-shell` lets the compositor center an unanchored surface. wlroots
//! compositors and KDE implement the protocol;
//! GNOME does not. [`is_available`] is the fail-closed gate, so unsupported
//! sessions keep Tauri's existing window behavior unchanged.

use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

/// Distance in logical pixels from the top of the usable area (below any
/// reserved bars/docks — gtk-layer-shell margins already account for those)
/// to the palette's top edge. Keeps it near the top like Spotlight/Raycast
/// rather than dead-center, and — because only the top edge is anchored —
/// keeps that top edge fixed while the window's height animates, so
/// resizing shrinks/grows the bottom edge instead of shifting the whole
/// window up and down on every keystroke the way vertical centering would.
pub const PALETTE_TOP_MARGIN_PX: i32 = 140;

/// Returns whether the current session advertises `wlr-layer-shell`.
///
/// This is false on X11, GNOME, unsupported Wayland compositors, and when no
/// usable display is present. It may perform a Wayland round trip on its first
/// call, so callers should probe once during startup.
pub fn is_available() -> bool {
    if !gtk::is_initialized_main_thread() {
        return false;
    }
    gtk_layer_shell::is_supported()
}

/// Configures a not-yet-realized GTK window as an overlay surface.
///
/// Layer-shell surfaces are centered by default when no edges are anchored.
/// Anchoring opposite edges would stretch the window and make GTK ignore its
/// requested size, so `top_margin_px` anchors only the top edge — left and
/// right stay unanchored, so the window is still horizontally centered, but
/// vertical placement follows the margin from the top of the usable area
/// (below any reserved bars/docks) instead of being vertically centered.
/// Pass `None` for a fully centered surface (used for Preferences, which
/// isn't a launcher and has no reason to sit near the top).
///
/// `namespace` is set explicitly rather than left at gtk-layer-shell's
/// default (which is literally the string `"gtk-layer-shell"`) because a
/// compositor's own layer rules are matched by namespace, and a shared
/// default namespace means picking up whatever a user's compositor config
/// already does for *any* app using this library. Confirmed live: this
/// dev machine's Hyprland config has `layerrule = ignorealpha 0,
/// gtk-layer-shell` (paired with `blur = true`) — meant for some other
/// generic gtk-layer-shell-based utility — which made the whole palette
/// render as a near-fully-blurred, see-through ghost of itself, not the
/// solid card the CSS actually specifies. A distinct per-window namespace
/// means Wield only ever renders exactly what it asks for unless a user
/// deliberately writes a rule matching it.
///
/// ## Keyboard mode: `Exclusive`, with a known, accepted tradeoff
///
/// Neither of gtk-layer-shell's other keyboard modes works cleanly here
/// (verified with `wtype`/`ydotool` synthetic input against a live
/// Hyprland 0.56.2 session, not just manual clicking):
///
/// - `OnDemand` never actually grants this surface keyboard focus, even
///   after synthetically clicking directly inside it first. Per the
///   wlr-layer-shell protocol, on-demand surfaces are expected to
///   separately *request* focus through a compositor-specific mechanism;
///   nothing in Wield or in Tauri's generic `WebviewWindow::set_focus()`
///   does that, and it matches a long-standing upstream report that
///   Hyprland's `ON_DEMAND` handling doesn't behave as documented
///   (<https://github.com/hyprwm/Hyprland/issues/2264>). With `OnDemand`,
///   pointer clicks worked but no keyboard input ever reached the window
///   — Escape-to-hide and the blur-to-hide listener both never fired
///   because the surface never held focus to lose in the first place.
/// - `Exclusive` does grant keyboard focus reliably (Escape-to-hide is
///   confirmed working) — but appears to also prevent *other* windows
///   from receiving focus (confirmed: `hyprctl activewindow` stayed on
///   the previously-focused app even after showing the palette, and a
///   real click on another window while the palette was open did not
///   register at all). This is effectively modal behavior while the
///   palette is visible — acceptable for how briefly it's shown, but a
///   real, deliberate tradeoff, not an oversight: it means click-away-to-
///   dismiss does not work; Escape (or the tray) are the ways to close
///   it. Revisit if Hyprland's on-demand handling improves, or if a
///   different mechanism (Wield detecting an outside click itself,
///   rather than relying on the compositor's normal focus handoff) turns
///   out to be worth the extra complexity.
pub fn configure(window: &gtk::ApplicationWindow, top_margin_px: Option<i32>, namespace: &str) {
    window.init_layer_shell();
    window.set_namespace(namespace);
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::Exclusive);
    if let Some(margin) = top_margin_px {
        window.set_anchor(Edge::Top, true);
        window.set_layer_shell_margin(Edge::Top, margin);
    }
}

type ForceCommitFn = unsafe extern "C" fn(*mut gtk_sys::GtkWindow);

/// Resolves `gtk_layer_try_force_commit` from whatever gtk-layer-shell
/// shared library is already loaded into this process, instead of
/// declaring it as a hard `extern "C"` link-time dependency.
///
/// It exists in the system library on the primary dev machine (confirmed:
/// gtk-layer-shell 0.10.1 there) but isn't exposed by either the safe
/// `gtk-layer-shell` crate (0.8.2) or its `-sys` bindings (0.7.2) - it was
/// added to the C library after these Rust bindings were last updated
/// (crates.io's own description of the crate is literally "UNMAINTAINED").
/// The C header documents it for exactly the situation this works around:
/// "the surface is in a state where it does not receive frame callbacks and
/// the regular deferred commit mechanism is unavailable." Confirmed via
/// repeated `grim` screenshots that this isn't hypothetical: the palette
/// was intermittently, non-deterministically fully unrendered (hyprctl
/// reported it mapped at the correct geometry; nothing was actually
/// painted) on repeated identical launches of identical builds.
///
/// A hard `extern "C"` declaration was tried first and failed CI outright:
/// GitHub's ubuntu-latest runners install an older `libgtk-layer-shell-dev`
/// that doesn't export this symbol at all, so linking `wield-app` failed
/// with "undefined symbol: gtk_layer_try_force_commit" - a real portability
/// gap the local dev machine's newer system library couldn't have caught.
/// Resolving it at runtime instead means an absent symbol is just "this
/// build doesn't have the workaround" (`None`, handled the same as the
/// X11/GNOME fallback path below), not a build failure - on *any* system,
/// not just CI's.
fn force_commit_fn() -> Option<ForceCommitFn> {
    const SYMBOL: &[u8] = b"gtk_layer_try_force_commit\0";
    unsafe {
        let process = libloading::os::unix::Library::this();
        let symbol = process.get::<ForceCommitFn>(SYMBOL).ok()?;
        Some(*symbol)
    }
}

/// Forces a pending surface commit if GTK hasn't already scheduled one and
/// the system's gtk-layer-shell exposes the C function that does it (see
/// [`force_commit_fn`]). A no-op if the symbol isn't available, or if
/// `window` was never turned into a layer surface in the first place (the
/// X11/GNOME fallback path) — calling the underlying C function on a
/// non-layer window is undefined behavior, so this checks first.
pub fn force_commit(window: &gtk::ApplicationWindow) {
    use gtk::glib::translate::ToGlibPtr;
    let window: &gtk::Window = window.as_ref();
    if !window.is_layer_window() {
        return;
    }
    let Some(force_commit_fn) = force_commit_fn() else {
        return;
    };
    unsafe {
        force_commit_fn(window.to_glib_none().0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_available_never_panics_without_a_display() {
        let _ = is_available();
    }
}
