//! Tauri command surface.

use crate::state::AppState;
use tokio_util::sync::CancellationToken;
use wield_core::{ArgMap, ArgType, ArgValue, ExecutionRequest, ToolOutcome};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolSummary {
    pub id: String,
    pub title: String,
    pub keywords: Vec<String>,
    pub category: String,
    pub args: Vec<wield_core::ArgSpec>,
    pub available: bool,
    pub reason: Option<String>,
}

pub fn list_tools_impl(state: &AppState, query: Option<&str>) -> Vec<ToolSummary> {
    let descriptors: Vec<&wield_core::Descriptor> =
        match query.map(str::trim).filter(|query| !query.is_empty()) {
            Some(query) => state.registry.search(query),
            None => state.registry.list().iter().collect(),
        };
    descriptors
        .into_iter()
        .map(|descriptor| {
            let (available, reason) =
                crate::capabilities::is_available(&state.availability, &descriptor.requires);
            ToolSummary {
                id: descriptor.id.as_ref().to_owned(),
                title: descriptor.title.clone(),
                keywords: descriptor.keywords.clone(),
                category: match descriptor.category {
                    wield_core::Category::Capture => "Capture",
                    wield_core::Category::Convert => "Convert",
                    wield_core::Category::Desktop => "Desktop",
                }
                .to_owned(),
                args: descriptor.args.clone(),
                available,
                reason,
            }
        })
        .collect()
}

#[tauri::command]
pub fn list_tools(state: tauri::State<'_, AppState>, query: Option<String>) -> Vec<ToolSummary> {
    list_tools_impl(&state, query.as_deref())
}

#[tauri::command]
pub fn capabilities(state: tauri::State<'_, AppState>) -> crate::capabilities::CapabilitiesReport {
    crate::capabilities::report(&state)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RunResult {
    pub run_id: crate::state::RunId,
    pub outcome: ToolOutcome,
}

/// Coerce a JSON object into the descriptor's typed argument map.
pub fn coerce_json_args(
    descriptor: &wield_core::Descriptor,
    args: &serde_json::Value,
) -> Result<ArgMap, String> {
    let object = args
        .as_object()
        .ok_or_else(|| "tool arguments must be a JSON object".to_owned())?;
    let mut result = ArgMap::new();
    for (name, value) in object {
        let spec = descriptor
            .args
            .iter()
            .find(|spec| spec.name == *name)
            .ok_or_else(|| format!("unknown argument: {name}"))?;
        let value = match &spec.arg_type {
            ArgType::File { multiple: true, .. } => {
                let items = value
                    .as_array()
                    .ok_or_else(|| format!("{name} must be an array of path strings"))?;
                let paths = items
                    .iter()
                    .map(|item| {
                        item.as_str()
                            .map(std::path::PathBuf::from)
                            .ok_or_else(|| format!("{name} must be an array of path strings"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                ArgValue::Paths(paths)
            }
            ArgType::File {
                multiple: false, ..
            }
            | ArgType::Dir => value
                .as_str()
                .map(|value| ArgValue::Path(value.into()))
                .ok_or_else(|| format!("{name} must be a path string"))?,
            ArgType::Int { .. } => value
                .as_i64()
                .map(ArgValue::Int)
                .ok_or_else(|| format!("{name} must be an integer"))?,
            ArgType::Float { .. } => value
                .as_f64()
                .map(ArgValue::Float)
                .ok_or_else(|| format!("{name} must be a number"))?,
            ArgType::Bool => value
                .as_bool()
                .map(ArgValue::Bool)
                .ok_or_else(|| format!("{name} must be a boolean"))?,
            ArgType::Str | ArgType::Text | ArgType::Enum { .. } => value
                .as_str()
                .map(|value| ArgValue::Str(value.to_owned()))
                .ok_or_else(|| format!("{name} must be a string"))?,
        };
        result.insert(name.clone(), value);
    }
    Ok(result)
}

pub async fn run_tool_impl(
    state: &AppState,
    id: &str,
    args: &serde_json::Value,
    requested_run_id: Option<crate::state::RunId>,
    mut on_progress: impl FnMut(wield_core::Progress) + Send + 'static,
) -> Result<RunResult, String> {
    let descriptor = state
        .registry
        .get(id)
        .cloned()
        .ok_or_else(|| format!("unknown tool: {id}"))?;
    let args = coerce_json_args(&descriptor, args)?;
    let run_id = requested_run_id.unwrap_or_default();
    let token = CancellationToken::new();
    state.register_run(run_id.clone(), token.clone());
    let (progress, mut progress_rx) = tokio::sync::mpsc::channel(32);
    let forward = tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            on_progress(progress);
        }
    });
    let started = std::time::Instant::now();
    let outcome = state
        .executor
        .run(ExecutionRequest { descriptor, args }, progress, token)
        .await;
    let _ = forward.await;
    state.take_run(&run_id);
    tracing::info!(
        tool_id = id,
        elapsed_ms = started.elapsed().as_millis() as u64,
        outcome = ?outcome,
        "tool run finished"
    );
    Ok(RunResult { run_id, outcome })
}

#[tauri::command]
pub async fn run_tool(
    state: tauri::State<'_, AppState>,
    id: String,
    args: serde_json::Value,
    run_id: Option<crate::state::RunId>,
    progress: tauri::ipc::Channel<wield_core::Progress>,
) -> Result<RunResult, String> {
    run_tool_impl(&state, &id, &args, run_id, move |event| {
        let _ = progress.send(event);
    })
    .await
}

#[tauri::command]
pub fn cancel(state: tauri::State<'_, AppState>, run_id: crate::state::RunId) -> bool {
    state.cancel_run(&run_id)
}

#[tauri::command]
pub fn hotkey_status(state: tauri::State<'_, AppState>) -> crate::state::HotkeyState {
    state.hotkey_state()
}

/// Opens the desktop environment's own shortcut-rebinding UI. Errors (surfaced
/// to the frontend as a rejected promise) when there's no active portal
/// session to reconfigure — the bind never succeeded, hasn't resolved yet, or
/// the backend doesn't support it.
#[tauri::command]
pub async fn configure_hotkey(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state.configure_hotkey().await
}

#[tauri::command]
pub fn quit(app: tauri::AppHandle) {
    tracing::info!("quit requested");
    app.exit(0);
}

#[tauri::command]
pub fn show_palette(app: tauri::AppHandle) {
    crate::palette::show(&app);
}

#[tauri::command]
pub fn hide_palette(app: tauri::AppHandle) {
    crate::palette::hide(&app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn write_stub(directory: &std::path::Path, body: &str) {
        let path = directory.join("magick");
        std::fs::write(&path, body).unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(path, permissions).unwrap();
    }

    #[test]
    fn list_tools_with_no_query_returns_registration_order() {
        let state = AppState::for_test_sync();
        let all = list_tools_impl(&state, None);
        assert_eq!(
            all.iter().map(|tool| tool.id.as_str()).collect::<Vec<_>>(),
            vec![
                "audio.extract",
                "color.pick",
                "document.convert",
                "image.convert",
                "video.convert"
            ]
        );
    }

    #[test]
    fn list_tools_with_a_query_ranks_matches() {
        let state = AppState::for_test_sync();
        let hits = list_tools_impl(&state, Some("img conv"));
        assert_eq!(hits.first().unwrap().id, "image.convert");
        assert!(list_tools_impl(&state, Some("zzz nonsense")).is_empty());
    }

    #[test]
    fn unavailable_tool_carries_a_reason() {
        let state = AppState::for_test_sync();
        let convert = list_tools_impl(&state, None)
            .into_iter()
            .find(|tool| tool.id == "image.convert")
            .unwrap();
        assert!(!convert.available);
        assert!(convert.reason.unwrap().contains("magick"));
    }

    #[tokio::test]
    async fn run_tool_runs_a_command_tool_against_a_stub() {
        let directory = tempfile::tempdir().unwrap();
        write_stub(
            directory.path(),
            "#!/bin/sh\nlast=\"\"\nfor arg in \"$@\"; do last=\"$arg\"; done\ncat \"$1\" > \"$last\"\n",
        );
        let input = directory.path().join("in.png");
        std::fs::write(&input, b"IMG").unwrap();
        let state = AppState::for_test(
            wield_tools::builtin_registry(),
            wield_core::BinaryResolver::with_dirs(vec![directory.path().to_path_buf()]),
        )
        .await;
        let args = serde_json::json!({ "input": input.to_string_lossy(), "format": "png" });
        let result = run_tool_impl(&state, "image.convert", &args, None, |_| {})
            .await
            .unwrap();
        assert!(matches!(
            result.outcome,
            wield_core::ToolOutcome::File { .. }
        ));
        assert!(state.take_run(&result.run_id).is_none());
    }

    #[tokio::test]
    async fn run_tool_forwards_progress_before_the_final_outcome() {
        let directory = tempfile::tempdir().unwrap();
        write_stub(
            directory.path(),
            "#!/bin/sh\nlast=\"\"\nfor arg in \"$@\"; do last=\"$arg\"; done\ncat \"$1\" > \"$last\"\n",
        );
        let input = directory.path().join("in.png");
        std::fs::write(&input, b"IMG").unwrap();
        let state = AppState::for_test(
            wield_tools::builtin_registry(),
            wield_core::BinaryResolver::with_dirs(vec![directory.path().to_path_buf()]),
        )
        .await;
        let args = serde_json::json!({ "input": input.to_string_lossy(), "format": "png" });
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let seen_clone = seen.clone();

        let result = run_tool_impl(&state, "image.convert", &args, None, move |progress| {
            seen_clone.lock().unwrap().push(progress);
        })
        .await
        .unwrap();

        assert!(matches!(result.outcome, ToolOutcome::File { .. }));
        let events = seen.lock().unwrap();
        assert!(events.contains(&wield_core::Progress::Started));
        assert!(events.contains(&wield_core::Progress::Finished));
    }

    #[tokio::test]
    async fn run_tool_rejects_unknown_arguments() {
        let state = AppState::build().await;
        assert!(coerce_json_args(
            state.registry.get("image.convert").unwrap(),
            &serde_json::json!({ "bogus": 1 }),
        )
        .is_err());
    }

    #[tokio::test]
    async fn coerce_json_args_accepts_an_array_for_a_multi_file_arg() {
        let mut descriptor = wield_core::DescriptorBuilder::new(
            "test.batch",
            "Test batch",
            wield_core::Category::Convert,
        )
        .arg(
            wield_core::ArgSpecBuilder::new(
                "input",
                "Input",
                wield_core::ArgType::File {
                    filters: vec![],
                    multiple: true,
                },
            )
            .required(true)
            .build(),
        )
        .requires(wield_core::Requires::None)
        .output(wield_core::OutputSpec::File {
            name: "output".to_owned(),
            dir: wield_core::OutputDir::SameAsInput,
        })
        .command(wield_core::CommandSpecBuilder::new("true"))
        .build()
        .expect("descriptor should be valid");
        descriptor.id = wield_core::ToolId::parse("test.batch").unwrap();

        let args = serde_json::json!({ "input": ["/a.mp4", "/b.mp4"] });
        let result = coerce_json_args(&descriptor, &args).expect("array should coerce");
        assert_eq!(
            result.get("input"),
            Some(&ArgValue::Paths(vec!["/a.mp4".into(), "/b.mp4".into()]))
        );
    }

    #[tokio::test]
    async fn coerce_json_args_rejects_an_array_for_a_single_file_arg() {
        let state = AppState::build().await;
        let descriptor = state.registry.get("image.convert").unwrap();
        let args = serde_json::json!({ "input": ["/a.png"], "format": "png" });
        assert!(coerce_json_args(descriptor, &args).is_err());
    }

    #[tokio::test]
    async fn cancel_stops_an_in_flight_run() {
        let directory = tempfile::tempdir().unwrap();
        write_stub(
            directory.path(),
            "#!/bin/sh\nsleep 5\nlast=\"\"\nfor arg in \"$@\"; do last=\"$arg\"; done\ncat \"$1\" > \"$last\"\n",
        );
        let input = directory.path().join("in.png");
        std::fs::write(&input, b"IMG").unwrap();
        let state = std::sync::Arc::new(
            AppState::for_test(
                wield_tools::builtin_registry(),
                wield_core::BinaryResolver::with_dirs(vec![directory.path().to_path_buf()]),
            )
            .await,
        );
        let args = serde_json::json!({ "input": input.to_string_lossy(), "format": "png" });
        let run_state = state.clone();
        let requested_run_id = crate::state::RunId("frontend-known-id".to_owned());
        let task_run_id = requested_run_id.clone();
        let run = tokio::spawn(async move {
            run_tool_impl(
                &run_state,
                "image.convert",
                &args,
                Some(task_run_id),
                |_| {},
            )
            .await
            .unwrap()
        });

        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let ids = state.in_flight_ids();
        assert_eq!(ids, vec![requested_run_id.clone()]);
        assert!(state.cancel_run(&requested_run_id));
        let result = run.await.unwrap();
        assert_eq!(result.run_id, requested_run_id);
        assert!(matches!(result.outcome, ToolOutcome::Cancelled));
    }
}
