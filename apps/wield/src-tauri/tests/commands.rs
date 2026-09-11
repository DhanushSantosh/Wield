#[test]
fn capabilities_lists_every_builtin_with_availability() {
    let state = tauri::async_runtime::block_on(wield_app_lib::state::AppState::build());
    let report = wield_app_lib::capabilities::report(&state);
    assert_eq!(report.tools.len(), 2);
}
