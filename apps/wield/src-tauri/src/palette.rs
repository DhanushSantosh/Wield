//! Access to the pre-warmed palette window.

use tauri::{AppHandle, Manager, WebviewWindow};

pub const LABEL: &str = "palette";

pub fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

/// Show and focus the palette, logging the synchronous visibility latency.
pub fn show(app: &AppHandle) {
    let Some(window) = window(app) else {
        tracing::warn!("palette window missing");
        return;
    };
    let started = std::time::Instant::now();
    if let Err(error) = window.show() {
        tracing::warn!(%error, "failed to show palette");
        return;
    }
    if let Err(error) = window.set_focus() {
        tracing::warn!(%error, "failed to focus palette");
    }
    // Works around an observed gtk-layer-shell flake where the surface is
    // mapped (correct geometry, everything correct at the protocol level)
    // but never actually commits a visible frame - see
    // layer_shell::force_commit's own doc comment. A no-op on the
    // X11/GNOME fallback path (force_commit checks is_layer_window itself).
    // Dispatched via run_on_main_thread: this fn is called from the D-Bus
    // ShowPalette handler, not guaranteed to already be on the GTK main
    // thread, and raw GTK objects (what gtk_window() returns) are not
    // thread-safe to touch from anywhere else.
    let for_main_thread = window.clone();
    if let Err(error) = window.run_on_main_thread(move || match for_main_thread.gtk_window() {
        Ok(gtk_window) => crate::layer_shell::force_commit(&gtk_window),
        Err(error) => tracing::warn!(%error, "could not get GTK handle to force-commit palette"),
    }) {
        tracing::warn!(%error, "failed to dispatch palette force-commit to the main thread");
    }
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis() as u64,
        "palette shown"
    );
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = window(app) {
        if let Err(error) = window.hide() {
            tracing::warn!(%error, "failed to hide palette");
        }
    }
}
