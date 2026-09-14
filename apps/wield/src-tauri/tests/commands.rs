#[test]
fn capabilities_lists_every_builtin_with_availability() {
    let state = tauri::async_runtime::block_on(wield_app_lib::state::AppState::build());
    let report = wield_app_lib::capabilities::report(&state);
    assert_eq!(report.tools.len(), 7);
}

#[test]
fn list_tools_returns_both_builtins_with_arg_schemas() {
    let state = tauri::async_runtime::block_on(wield_app_lib::state::AppState::build());
    let tools = wield_app_lib::commands::list_tools_impl(&state, None);
    assert_eq!(tools.len(), 7);
    let convert = tools
        .iter()
        .find(|tool| tool.id == "image.convert")
        .unwrap();
    assert_eq!(convert.args.len(), 4);
    assert_eq!(convert.category, "Convert");
}
