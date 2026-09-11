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
