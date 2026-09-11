//! Tauri command surface.

use crate::state::AppState;

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
