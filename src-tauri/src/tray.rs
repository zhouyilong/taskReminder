use tauri::{
    AppHandle, CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu, WindowBuilder,
};

use crate::paths;
use crate::state::AppState;

pub fn build_tray() -> SystemTray {
    let dev_tag = if paths::is_dev_mode() { " [开发]" } else { "" };
    let open = CustomMenuItem::new("open".to_string(), format!("打开{}", dev_tag));
    let cloud = CustomMenuItem::new("cloud".to_string(), "云同步（WebDAV）...");
    let sync_now = CustomMenuItem::new("sync_now".to_string(), "立即同步");
    let quit = CustomMenuItem::new("quit".to_string(), "退出");

    let menu = SystemTrayMenu::new()
        .add_item(open)
        .add_item(cloud)
        .add_item(sync_now)
        .add_native_item(tauri::SystemTrayMenuItem::Separator)
        .add_item(quit);

    SystemTray::new().with_menu(menu)
}

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    } else {
        let title = if paths::is_dev_mode() {
            "任务提醒应用 [开发]"
        } else {
            "任务提醒应用"
        };
        let _ = WindowBuilder::new(app, "main", tauri::WindowUrl::App("index.html".into()))
            .title(title)
            .inner_size(1000.0, 650.0)
            .min_inner_size(800.0, 600.0)
            .decorations(false)
            .transparent(false)
            .resizable(true)
            .build();
    }
}

pub fn handle_event(app: &AppHandle, event: SystemTrayEvent) {
    match event {
        SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
            "open" => {
                show_main(app);
            }
            "cloud" => {
                let _ = app.emit_all("open-sync-settings", ());
                show_main(app);
            }
            "sync_now" => {
                if let Some(state) = app.try_state::<AppState>() {
                    let _ = state.sync.request_sync("tray");
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        },
        SystemTrayEvent::DoubleClick { .. } | SystemTrayEvent::LeftClick { .. } => {
            show_main(app);
        }
        _ => {}
    }
}
