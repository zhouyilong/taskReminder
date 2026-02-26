use tauri::{
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};

use crate::create_custom_sticky_note_via_app;
use crate::paths;
use crate::state::AppState;
use crate::trigger_sticky_note_window_visibility;

fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    } else {
        let title = if paths::is_dev_mode() {
            "任务提醒应用 [开发]"
        } else {
            "任务提醒应用"
        };
        let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
            .title(title)
            .inner_size(1000.0, 650.0)
            .min_inner_size(800.0, 600.0)
            .decorations(false)
            .transparent(false)
            .resizable(true)
            .build();
    }
}

fn show_sticky_notes(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        trigger_sticky_note_window_visibility(app, state.db.clone(), true);
    } else {
        let _ = app.emit("tray-open-sticky-notes", ());
    }
}

fn create_sticky_note(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Err(err) = create_custom_sticky_note_via_app(app, state.inner(), "", None, None) {
            eprintln!("[tray] 新建便签失败: {}", err);
        }
    } else {
        let _ = app.emit("tray-create-sticky-note", ());
    }
}

pub fn setup_tray(app: &AppHandle) -> Result<(), tauri::Error> {
    let dev_tag = if paths::is_dev_mode() {
        " [开发]"
    } else {
        ""
    };
    let menu = MenuBuilder::new(app)
        .text("open", format!("打开{}", dev_tag))
        .text("open_sticky", "打开桌面便签")
        .text("new_note", "新建便签")
        .text("sync_now", "立即同步")
        .separator()
        .text("quit", "退出")
        .build()?;

    let mut tray_builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                show_main(app);
            }
            "open_sticky" => {
                show_sticky_notes(app);
            }
            "new_note" => {
                create_sticky_note(app);
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
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(&tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray_builder = tray_builder.icon(icon.clone());
    }

    let _ = tray_builder.build(app)?;
    Ok(())
}
