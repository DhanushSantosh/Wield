//! Access to the pre-warmed palette window.

use std::sync::atomic::Ordering;
use std::time::Duration;

use gtk::prelude::{GtkWindowExt, WidgetExt};
use tauri::{AppHandle, Manager, WebviewWindow};

pub const LABEL: &str = "palette";

/// Width never changes — only height animates to fit content, matching the
/// established fixed-width/variable-height convention of this kind of
/// launcher (Spotlight, Raycast, Alfred).
const RESIZE_STEPS: u32 = 12;
const RESIZE_TOTAL_MS: u64 = 180;
/// Below this, the window would be smaller than the search box alone; above
/// this, a content-measurement bug can't push the window past a sane size.
const MIN_HEIGHT: f64 = 72.0;
const MAX_HEIGHT: f64 = 640.0;

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
    let app_for_main_thread = app.clone();
    if let Err(error) = window.run_on_main_thread(move || {
        match for_main_thread.gtk_window() {
            Ok(gtk_window) => {
                let gtk_window: &gtk::Window = gtk_window.as_ref();
                crate::layer_shell::force_commit(gtk_window);
            }
            Err(error) => {
                tracing::warn!(%error, "could not get GTK handle to force-commit palette")
            }
        }
        app_for_main_thread
            .state::<crate::state::AppState>()
            .show_click_catcher();
    }) {
        tracing::warn!(%error, "failed to dispatch palette force-commit to the main thread");
    }
    tracing::info!(
        elapsed_ms = started.elapsed().as_millis() as u64,
        "palette shown"
    );
}

pub fn hide(app: &AppHandle) {
    let Some(window) = window(app) else {
        return;
    };
    if let Err(error) = window.hide() {
        tracing::warn!(%error, "failed to hide palette");
    }
    // Dispatched via run_on_main_thread: hide() is called from the
    // click-catcher's own GTK signal handler (already on the main
    // thread - harmless to dispatch again from there) but also from the
    // D-Bus ShowPalette-adjacent paths and the frontend's Escape/blur
    // handlers, which are not guaranteed to be - same reasoning as
    // show()'s existing dispatch above.
    let app_for_main_thread = app.clone();
    if let Err(error) = window.run_on_main_thread(move || {
        app_for_main_thread
            .state::<crate::state::AppState>()
            .hide_click_catcher();
    }) {
        tracing::warn!(%error, "failed to dispatch click-catcher hide to the main thread");
    }
}

/// Ease-out cubic: fast at the start, settling gently into the target —
/// matches the feel of native window/sheet transitions rather than a linear,
/// mechanical resize.
fn ease_out_cubic(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

/// The sequence of intermediate heights an animation from `start` to
/// `target` over `steps` steps should pass through, easing out. The last
/// value is always exactly `target` (no rounding drift at the end).
/// Pure and fully deterministic — no window, no I/O — so this is where the
/// actual interpolation behavior gets tested.
pub fn resize_steps(start: f64, target: f64, steps: u32) -> Vec<f64> {
    if steps == 0 {
        return vec![target];
    }
    (1..=steps)
        .map(|step| {
            if step == steps {
                target
            } else {
                let t = f64::from(step) / f64::from(steps);
                start + (target - start) * ease_out_cubic(t)
            }
        })
        .collect()
}

/// Smoothly resizes the palette window's height to `target_height` (logical
/// pixels), clamped to a sane range; width is left untouched. Any resize
/// already in flight is superseded — only the most recent request actually
/// finishes, so rapid content changes (e.g. typing fast) don't queue up a
/// backlog of stale animations.
pub fn animate_to_height(app: &AppHandle, target_height: f64) {
    let Some(window) = window(app) else {
        tracing::warn!("palette window missing");
        return;
    };
    let state = app.state::<crate::state::AppState>();
    let target_height = target_height.clamp(MIN_HEIGHT, MAX_HEIGHT);
    let my_generation = state.resize_generation.fetch_add(1, Ordering::SeqCst) + 1;
    tracing::info!(target_height, "resize requested");
    // Cloning the handle (cheap) rather than the whole AppState — the async
    // task below only needs to keep re-checking the shared counter.
    let app = app.clone();

    tauri::async_runtime::spawn(async move {
        let (Ok(scale_factor), Ok(current)) = (window.scale_factor(), window.inner_size()) else {
            tracing::warn!("could not read current palette size; skipping resize");
            return;
        };
        let current = current.to_logical::<f64>(scale_factor);
        let width = current.width;
        let start_height = current.height;
        tracing::info!(start_height, target_height, "resize starting");

        if (start_height - target_height).abs() < 0.5 {
            tracing::info!("resize skipped: already at target height");
            return;
        }

        let step_delay = Duration::from_millis(RESIZE_TOTAL_MS / u64::from(RESIZE_STEPS));
        let heights = resize_steps(start_height, target_height, RESIZE_STEPS);
        let state = app.state::<crate::state::AppState>();
        for (index, height) in heights.into_iter().enumerate() {
            if state.resize_generation.load(Ordering::SeqCst) != my_generation {
                tracing::info!("resize superseded by a newer request");
                return;
            }
            let target_width = width.round() as i32;
            let target_height_px = height.round() as i32;
            let for_main_thread = window.clone();
            let dispatch = window.run_on_main_thread(move || {
                // Deliberately not Tauri's generic WebviewWindow::set_size():
                // on a gtk-layer-shell surface that call doesn't correctly
                // participate in the surface's own configure/commit
                // handshake, and the window silently stops rendering
                // anything at all (confirmed live: hyprctl still reports it
                // mapped at the right geometry, but the surface never
                // commits a visible frame again). gtk-layer-shell's own
                // docs give the fix: set the widget's size request, then
                // resize to (1, 1) so GTK's normal natural-size negotiation
                // takes over and actually commits a new frame. This is
                // plain GTK API, not layer-shell-specific, so it's correct
                // for the X11/no-layer-shell fallback window too.
                match for_main_thread.gtk_window() {
                    Ok(gtk_window) => {
                        gtk_window.set_size_request(target_width, target_height_px);
                        gtk_window.resize(1, 1);
                    }
                    Err(error) => {
                        tracing::warn!(%error, "could not get GTK handle during palette resize");
                    }
                }
            });
            if let Err(error) = dispatch {
                tracing::warn!(%error, "failed to dispatch palette resize to the main thread");
                return;
            }
            let is_last = index + 1 == RESIZE_STEPS as usize;
            if !is_last {
                tokio::time::sleep(step_delay).await;
            }
        }
        tracing::info!(target_height, "resize finished");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_steps_ends_exactly_on_target_growing() {
        let steps = resize_steps(100.0, 300.0, 12);
        assert_eq!(steps.len(), 12);
        assert_eq!(*steps.last().unwrap(), 300.0);
        // Monotonically increasing toward the target.
        for pair in steps.windows(2) {
            assert!(pair[1] >= pair[0]);
        }
        assert!(steps[0] > 100.0, "the first step should move immediately");
    }

    #[test]
    fn resize_steps_ends_exactly_on_target_shrinking() {
        let steps = resize_steps(400.0, 150.0, 12);
        assert_eq!(*steps.last().unwrap(), 150.0);
        for pair in steps.windows(2) {
            assert!(pair[1] <= pair[0]);
        }
    }

    #[test]
    fn resize_steps_with_zero_steps_jumps_straight_to_target() {
        assert_eq!(resize_steps(100.0, 300.0, 0), vec![300.0]);
    }

    #[test]
    fn resize_steps_eases_out_rather_than_linear() {
        // Ease-out cubic: the midpoint step should already be well past the
        // linear midpoint, not sitting exactly on it.
        let steps = resize_steps(0.0, 100.0, 2);
        let midpoint_step = steps[0];
        let linear_midpoint = 50.0;
        assert!(midpoint_step > linear_midpoint);
    }
}
