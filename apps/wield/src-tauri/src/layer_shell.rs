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
/// Keyboard mode is `Exclusive`, not `OnDemand` — corrected after live
/// testing showed `OnDemand` never actually grants this surface keyboard
/// focus. Per the wlr-layer-shell protocol, on-demand surfaces are expected
/// to separately *request* focus through a compositor-specific mechanism;
/// Tauri's generic `WebviewWindow::set_focus()` doesn't do that for a
/// layer-shell surface, so nothing ever asked for it. Pointer input still
/// worked (clicks don't need keyboard focus), but no keyboard input ever
/// reached the window - including Escape-to-hide and the blur-to-hide
/// listener, which never fired because the surface never gained focus to
/// lose. `Exclusive` grants keyboard focus automatically whenever the
/// surface is mapped on the overlay layer, matching what a palette that's
/// only ever shown to receive input actually needs - no separate
/// focus-request step required.
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
