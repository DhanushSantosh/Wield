//! The StatusNotifierItem tray icon and its menu.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, SubmenuBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter};

use crate::commands::ToolSummary;

const CATEGORY_ORDER: [&str; 3] = ["Capture", "Convert", "Desktop"];

/// Attach the tray icon + menu + event handlers. Call once from `setup`, once
/// the app's windows exist. Logs (rather than fails) if no SNI host is
/// present — the spec asks for a one-time notice; a *visible* one is
/// Preferences content (P6), so this is a log line for now.
pub fn build(app: &AppHandle, tools: &[ToolSummary]) -> tauri::Result<Option<CheckMenuItem<tauri::Wry>>> {
    let (menu, keep_awake_item) = build_menu(app, tools)?;

    let icon = app.default_window_icon().cloned();
    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Wield")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| handle_menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::palette::show(tray.app_handle());
            }
        });
    if let Some(icon) = icon {
        builder = builder.icon(icon);
    }

    match builder.build(app) {
        Ok(_tray) => Ok(keep_awake_item),
        Err(error) => {
            tracing::warn!(
                %error,
                "no tray/StatusNotifierItem host found; the tray icon will not appear. \
                 Use the GlobalShortcuts hotkey (or its fallback command) to reach Wield."
            );
            Ok(None)
        }
    }
}

/// One submenu per category present in `tools`, in a fixed order, followed by
/// a separator and the "Preferences" / "Quit" items. Each tool item's menu id
/// is `"tool:{descriptor.id}"`.
///
/// Generic over the Tauri runtime so it can be exercised against
/// `tauri::test::MockRuntime` in tests without a real tray/window backend.
fn build_menu<R: tauri::Runtime>(
    app: &AppHandle<R>,
    tools: &[ToolSummary],
) -> tauri::Result<(Menu<R>, Option<CheckMenuItem<R>>)> {
    let menu = Menu::new(app)?;
    let mut keep_awake_item = None;

    for category in CATEGORY_ORDER {
        let in_category: Vec<&ToolSummary> = tools
            .iter()
            .filter(|tool| tool.category == category)
            .collect();
        if in_category.is_empty() {
            continue;
        }
        let mut submenu = SubmenuBuilder::new(app, category);
        for tool in in_category {
            if tool.id == "keep.awake" {
                let item = CheckMenuItem::with_id(
                    app,
                    format!("tool:{}", tool.id),
                    &tool.title,
                    true,
                    crate::commands::keep_awake_is_active(),
                    None::<&str>,
                )?;
                submenu = submenu.item(&item);
                keep_awake_item = Some(item);
                continue;
            }
            let item = MenuItem::with_id(
                app,
                format!("tool:{}", tool.id),
                &tool.title,
                true,
                None::<&str>,
            )?;
            submenu = submenu.item(&item);
        }
        menu.append(&submenu.build()?)?;
    }

    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "preferences",
        "Preferences",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?)?;

    Ok((menu, keep_awake_item))
}

fn handle_menu_event(app: &AppHandle, id: &str) {
    if let Some(tool_id) = id.strip_prefix("tool:") {
        let _ = app.emit("tray://select-tool", tool_id);
        crate::palette::show(app);
        return;
    }
    match id {
        // Settings is a view inside the palette window, not a separate
        // window (see App.tsx / SettingsView) - so "opening" it from the
        // tray means showing the palette and telling it to switch view,
        // the same two-step shape as selecting a tool above.
        "preferences" => {
            let _ = app.emit("tray://open-settings", ());
            crate::palette::show(app);
        }
        "quit" => app.exit(0),
        other => tracing::warn!(menu_id = other, "unhandled tray menu item"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_tools() -> Vec<ToolSummary> {
        vec![
            ToolSummary {
                id: "color.pick".into(),
                title: "Pick a colour".into(),
                keywords: vec![],
                category: "Capture".into(),
                args: vec![],
                available: true,
                reason: None,
            },
            ToolSummary {
                id: "image.convert".into(),
                title: "Convert image".into(),
                keywords: vec![],
                category: "Convert".into(),
                args: vec![],
                available: true,
                reason: None,
            },
        ]
    }

    #[test]
    fn menu_groups_tools_by_category_and_adds_the_fixed_items() {
        let app = tauri::test::mock_app();
        let (menu, _keep_awake_item) = build_menu(&app.handle().clone(), &fixture_tools()).unwrap();
        let items = menu.items().unwrap();

        // Two category submenus + separator + preferences + quit.
        assert_eq!(items.len(), 5);

        let tauri::menu::MenuItemKind::Submenu(capture) = &items[0] else {
            panic!("expected the first item to be a submenu");
        };
        assert_eq!(capture.text().unwrap(), "Capture");
        let capture_items = capture.items().unwrap();
        assert_eq!(capture_items.len(), 1);
        assert_eq!(capture_items[0].id().as_ref(), "tool:color.pick");

        let tauri::menu::MenuItemKind::Submenu(convert) = &items[1] else {
            panic!("expected the second item to be a submenu");
        };
        assert_eq!(convert.text().unwrap(), "Convert");
        let convert_items = convert.items().unwrap();
        assert_eq!(convert_items[0].id().as_ref(), "tool:image.convert");

        assert_eq!(items[3].id().as_ref(), "preferences");
        assert_eq!(items[4].id().as_ref(), "quit");
    }

    #[test]
    fn empty_registry_still_has_preferences_and_quit() {
        let app = tauri::test::mock_app();
        let (menu, _keep_awake_item) = build_menu(&app.handle().clone(), &[]).unwrap();
        let items = menu.items().unwrap();
        assert_eq!(items.len(), 3); // separator, preferences, quit
        assert_eq!(items[1].id().as_ref(), "preferences");
        assert_eq!(items[2].id().as_ref(), "quit");
    }

    #[test]
    fn keep_awake_renders_as_a_check_menu_item() {
        let app = tauri::test::mock_app();
        let mut tools = fixture_tools();
        tools.push(ToolSummary {
            id: "keep.awake".into(),
            title: "Keep awake".into(),
            keywords: vec![],
            category: "Capture".into(),
            args: vec![],
            available: true,
            reason: None,
        });
        let (menu, keep_awake_item) = build_menu(&app.handle().clone(), &tools).unwrap();
        let item = keep_awake_item.expect("keep.awake should produce a CheckMenuItem");
        assert!(!item.is_checked().unwrap());

        // Also present under the Capture submenu, alongside color.pick.
        let items = menu.items().unwrap();
        let tauri::menu::MenuItemKind::Submenu(capture) = &items[0] else {
            panic!("expected the first item to be a submenu");
        };
        let capture_items = capture.items().unwrap();
        assert_eq!(capture_items.len(), 2);
        assert_eq!(capture_items[1].id().as_ref(), "tool:keep.awake");
    }
}
