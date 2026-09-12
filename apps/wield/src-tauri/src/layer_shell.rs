//! Wayland layer-shell positioning for launchers and utility windows.
//!
//! Regular Wayland toplevels cannot choose absolute screen coordinates.
//! `wlr-layer-shell` lets the compositor center an unanchored surface. wlroots
//! compositors and KDE implement the protocol;
//! GNOME does not. [`is_available`] is the fail-closed gate, so unsupported
//! sessions keep Tauri's existing window behavior unchanged.

use gtk_layer_shell::{KeyboardMode, Layer, LayerShell};

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

/// Configures a not-yet-realized GTK window as a centered overlay surface.
///
/// Layer-shell surfaces are centered by default when no edges are anchored.
/// Anchoring opposite edges would stretch the window and make GTK ignore its
/// requested size.
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
pub fn configure(window: &gtk::ApplicationWindow) {
    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::Exclusive);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_available_never_panics_without_a_display() {
        let _ = is_available();
    }
}
