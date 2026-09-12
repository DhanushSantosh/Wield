//! Wayland layer-shell positioning for launchers and utility windows.
//!
//! Regular Wayland toplevels cannot choose absolute screen coordinates.
//! `wlr-layer-shell` lets the compositor center a smaller surface anchored to
//! both opposing edges. wlroots compositors and KDE implement the protocol;
//! GNOME does not. [`is_available`] is the fail-closed gate, so unsupported
//! sessions keep Tauri's existing window behavior unchanged.

use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

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
/// Anchoring to both edges of each axis delegates centering to the compositor.
/// On-demand keyboard mode allows Wield's existing explicit focus behavior to
/// keep controlling when the window receives keyboard input.
pub fn configure(window: &gtk::ApplicationWindow) {
    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        window.set_anchor(edge, true);
    }
    window.set_keyboard_mode(KeyboardMode::OnDemand);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_available_never_panics_without_a_display() {
        let _ = is_available();
    }
}
