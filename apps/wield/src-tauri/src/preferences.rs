//! Access to the (currently content-less) Preferences window.

use tauri::{AppHandle, Manager, WebviewWindow};

pub const LABEL: &str = "preferences";

pub fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

pub fn show(app: &AppHandle) {
    let Some(window) = window(app) else {
        tracing::warn!("preferences window missing");
        return;
    };
    if let Err(error) = window.show() {
        tracing::warn!(%error, "failed to show preferences");
        return;
    }
    if let Err(error) = window.set_focus() {
        tracing::warn!(%error, "failed to focus preferences");
    }
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = window(app) {
        if let Err(error) = window.hide() {
            tracing::warn!(%error, "failed to hide preferences");
        }
    }
}
