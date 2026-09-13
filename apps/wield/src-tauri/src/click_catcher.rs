//! A full-screen, invisible layer-shell surface shown behind the palette.
//! Its only job is to dismiss the palette when something outside it is
//! clicked - see docs/superpowers/specs/2026-09-13-wield-click-catcher-design.md
//! for why this exists and why it's a plain GTK window rather than a
//! second Tauri/webview window.

use gtk::prelude::{GtkWindowExt, WidgetExt, WidgetExtManual};
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
    if let Some(screen) = GtkWindowExt::screen(&window) {
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
