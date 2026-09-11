//! Tauri command surface.

#[tauri::command]
pub fn capabilities(
    state: tauri::State<'_, crate::state::AppState>,
) -> crate::capabilities::CapabilitiesReport {
    crate::capabilities::report(&state)
}
