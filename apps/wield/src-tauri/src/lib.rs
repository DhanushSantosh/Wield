use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use wield_core::{Stage, ToolOutcome};

pub mod capabilities;
pub mod commands;
pub mod instance;
mod layer_shell;
mod logging;
pub mod palette;
pub mod preferences;
pub mod state;
pub mod tray;

#[cfg(test)]
mod test_support;

#[derive(Serialize)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Wield".to_string(),
        version: wield_core::version().to_string(),
    }
}

#[derive(Default)]
struct PendingShell {
    app: Mutex<Option<tauri::AppHandle>>,
    pending_show: AtomicBool,
    bound: tokio::sync::Notify,
}

impl PendingShell {
    fn bind(&self, app: tauri::AppHandle) {
        let show = {
            let mut slot = self
                .app
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *slot = Some(app.clone());
            self.pending_show.swap(false, Ordering::AcqRel)
        };
        self.bound.notify_waiters();
        if show {
            palette::show(&app);
        }
    }

    fn app(&self) -> Option<tauri::AppHandle> {
        self.app
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    async fn wait_for_app(&self) -> tauri::AppHandle {
        loop {
            let notified = self.bound.notified();
            if let Some(app) = self.app() {
                return app;
            }
            notified.await;
        }
    }
}

#[async_trait::async_trait]
impl instance::ShellHandle for PendingShell {
    fn show_palette(&self) {
        let app = {
            let slot = self
                .app
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if slot.is_none() {
                self.pending_show.store(true, Ordering::Release);
            }
            slot.clone()
        };
        if let Some(app) = app {
            palette::show(&app);
        }
    }

    async fn run_tool(&self, id: &str, args_json: &str) -> String {
        let args = match serde_json::from_str(args_json) {
            Ok(args) => args,
            Err(error) => {
                return outcome_json(ToolOutcome::Failed {
                    stage: Stage::Validation,
                    detail: format!("invalid JSON arguments: {error}"),
                    hint: None,
                })
            }
        };
        let app = self.wait_for_app().await;
        let state = app.state::<state::AppState>();
        match commands::run_tool_impl(&state, id, &args, None, |_| {}).await {
            Ok(result) => outcome_json(result.outcome),
            Err(detail) => outcome_json(ToolOutcome::Failed {
                stage: Stage::Validation,
                detail,
                hint: None,
            }),
        }
    }
}

fn outcome_json(outcome: ToolOutcome) -> String {
    match serde_json::to_string(&outcome) {
        Ok(json) => json,
        Err(_) => {
            r#"{"Failed":{"stage":"Native","detail":"outcome serialization failed","hint":null}}"#
                .to_owned()
        }
    }
}

fn runner_args_from_env() -> Option<(String, String)> {
    // The D-Bus runner bridge accepts this shape; P5a launches only activate the palette.
    None
}

pub fn run() {
    let _guard = logging::init();
    let shell = Arc::new(PendingShell::default());
    let acquired =
        tauri::async_runtime::block_on(instance::acquire(shell.clone(), runner_args_from_env()));
    let instance_connection = match acquired {
        instance::Acquired::Primary(connection) => connection,
        instance::Acquired::Secondary => return,
    };
    let state = tauri::async_runtime::block_on(state::AppState::build());
    let setup_shell = shell.clone();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            app_info,
            commands::capabilities,
            commands::list_tools,
            commands::run_tool,
            commands::cancel,
            commands::show_palette,
            commands::hide_palette,
            commands::hotkey_status,
            commands::configure_hotkey,
            commands::resize_palette,
            commands::quit
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            if layer_shell::is_available() {
                let gtk_windows = [palette::LABEL, preferences::LABEL]
                    .into_iter()
                    .map(|label| {
                        let window = app
                            .get_webview_window(label)
                            .ok_or_else(|| format!("window {label:?} is missing"))?
                            .gtk_window()
                            .map_err(|error| {
                                format!("could not get GTK handle for window {label:?}: {error}")
                            })?;
                        Ok((label, window))
                    })
                    .collect::<Result<Vec<_>, String>>();

                match gtk_windows {
                    Ok(windows) => {
                        for (label, window) in &windows {
                            // Only the palette sits near the top, like a
                            // launcher; Preferences is a settings dialog and
                            // stays fully centered.
                            let top_margin = (*label == palette::LABEL)
                                .then_some(layer_shell::PALETTE_TOP_MARGIN_PX);
                            layer_shell::configure(window, top_margin);
                        }
                        tracing::info!("layer-shell positioning enabled");
                    }
                    Err(error) => {
                        tracing::warn!(%error, "layer-shell setup skipped for all windows");
                    }
                }
            } else {
                tracing::info!("layer-shell unavailable; using default window positioning");
            }

            // Binding can service a pending D-Bus request by showing the
            // palette immediately, so layer-shell setup must happen first.
            setup_shell.bind(handle.clone());

            // Closing either window hides it instead of destroying it — the
            // palette must stay warm, and Preferences has no reason to be
            // recreated either.
            for label in [palette::LABEL, preferences::LABEL] {
                if let Some(window) = app.get_webview_window(label) {
                    let window_clone = window.clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();
                            let _ = window_clone.hide();
                        }
                    });
                }
            }

            // Tray icon + menu, built from the current tool list. Logs (does
            // not fail startup) if no SNI host is present.
            let tools = commands::list_tools_impl(&handle.state::<state::AppState>(), None);
            if let Err(error) = tray::build(&handle, &tools) {
                tracing::warn!(%error, "failed to build tray icon");
            }

            // GlobalShortcuts: bind "show palette" to <Super>w. ashpd 0.13 has
            // no session-restore token for this portal, so the desktop
            // environment's confirmation dialog reappears every launch — see
            // docs/testing.md. Never blocks startup either way.
            let hotkey_app = handle.clone();
            tauri::async_runtime::spawn(async move {
                let show_app = hotkey_app.clone();
                let (outcome, controller) =
                    wield_portal::global_shortcuts::bind_show_palette(move || {
                        palette::show(&show_app);
                    })
                    .await;
                let hotkey_state = match outcome {
                    wield_portal::global_shortcuts::BindOutcome::Bound => {
                        state::HotkeyState::Registered
                    }
                    wield_portal::global_shortcuts::BindOutcome::Unavailable {
                        fallback_command,
                    } => state::HotkeyState::Unavailable { fallback_command },
                };
                let app_state = hotkey_app.state::<state::AppState>();
                app_state.set_hotkey_state(hotkey_state);
                app_state.set_hotkey_controller(controller);
            });

            tracing::info!("wield shell ready (headless)");
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("build wield shell");

    // Windows hide instead of closing (see `setup`); the only real exit path
    // is the `quit` command / tray "Quit" item calling `AppHandle::exit`.
    // Verified manually (Task 6): this does not need an `ExitRequested`
    // guard against the installed Tauri version — `quit` terminates the
    // process directly.
    app.run(|_app, _event| {});
    drop(instance_connection);
}
