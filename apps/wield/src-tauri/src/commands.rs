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
}

pub fn list_tools_impl(state: &AppState) -> Vec<ToolSummary> {
    state
        .registry
        .list()
        .iter()
        .map(|descriptor| ToolSummary {
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
            available: crate::capabilities::is_available(
                &state.availability,
                &descriptor.requires,
            )
            .0,
        })
        .collect()
}

#[tauri::command]
pub fn list_tools(state: tauri::State<'_, AppState>) -> Vec<ToolSummary> {
    list_tools_impl(&state)
}

#[tauri::command]
pub fn capabilities(
    state: tauri::State<'_, AppState>,
) -> crate::capabilities::CapabilitiesReport {
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
            ArgType::File { .. } | ArgType::Dir => value
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
) -> Result<RunResult, String> {
    let descriptor = state
        .registry
        .get(id)
        .cloned()
        .ok_or_else(|| format!("unknown tool: {id}"))?;
    let args = coerce_json_args(&descriptor, args)?;
    let run_id = crate::state::RunId::new();
    let token = CancellationToken::new();
    state.register_run(run_id.clone(), token.clone());
    let (progress, mut progress_rx) = tokio::sync::mpsc::channel(16);
    let drain = tokio::spawn(async move { while progress_rx.recv().await.is_some() {} });
    let started = std::time::Instant::now();
    let outcome = state
        .executor
        .run(
            ExecutionRequest {
                descriptor,
                args,
            },
            progress,
            token,
        )
        .await;
    let _ = drain.await;
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
) -> Result<RunResult, String> {
    run_tool_impl(&state, &id, &args).await
}

#[tauri::command]
pub fn cancel(state: tauri::State<'_, AppState>, run_id: crate::state::RunId) -> bool {
    state.cancel_run(&run_id)
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
        let result = run_tool_impl(&state, "image.convert", &args)
            .await
            .unwrap();
        assert!(matches!(
            result.outcome,
            wield_core::ToolOutcome::File { .. }
        ));
        assert!(state.take_run(&result.run_id).is_none());
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
        let run = tokio::spawn(async move {
            run_tool_impl(&run_state, "image.convert", &args)
                .await
                .unwrap()
        });

        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        let ids = state.in_flight_ids();
        assert_eq!(ids.len(), 1);
        assert!(state.cancel_run(&ids[0]));
        let result = run.await.unwrap();
        assert!(matches!(result.outcome, ToolOutcome::Cancelled));
    }
}
