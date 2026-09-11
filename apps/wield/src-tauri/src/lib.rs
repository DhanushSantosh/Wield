use serde::Serialize;

mod logging;
pub mod state;

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

pub fn run() {
    let _guard = logging::init();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running Wield");
}
