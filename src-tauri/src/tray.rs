use std::sync::{Mutex, OnceLock};

use chrono::{Datelike, Duration, Local, NaiveDateTime};
use tauri::{
    menu::{Menu, MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Listener, Manager, WebviewUrl, WebviewWindowBuilder, Wry,
};

use crate::commands::sticky::{create_custom_sticky_note_via_app, CreateStickyNotePayload};
use crate::commands::tasks::{complete_task_by_id, reschedule_task_reminder};
use crate::models::{RecurringTask, Task};
use crate::paths;
use crate::quiet_hours;
use crate::state::AppState;
use crate::time::{format_datetime, parse_datetime_any};
use crate::windows::sticky;

const TRAY_ID: &str = "main-tray";
/// 托盘定时刷新间隔：跨天后“明天”变“今天”、勿扰状态变化等不依赖数据变更的情况。
const REFRESH_INTERVAL_SECONDS: u64 = 30;
/// 托盘“推迟”的分钟数。
const TRAY_SNOOZE_MINUTES: i64 = 15;
/// 托盘中待办描述的最大显示字数（Windows 托盘提示最多约 127 个字符）。
const DESCRIPTION_MAX_CHARS: usize = 18;
const NEXT_COMPLETE_PREFIX: &str = "next_complete:";
const NEXT_SNOOZE_PREFIX: &str = "next_snooze:";

/// 托盘展示的下一条提醒。
#[derive(Debug, Clone, PartialEq)]
pub struct NextReminder {
    pub id: String,
    /// 一次性待办可在托盘中完成或推迟；循环提醒只展示。
    pub is_task: bool,
    pub description: String,
    pub time: NaiveDateTime,
}

/// 未来最近的一条提醒：未完成待办的提醒时间与未暂停循环提醒的下次触发时间中最早的一个。
pub fn find_next_reminder(
    tasks: &[Task],
    recurring: &[RecurringTask],
    now: NaiveDateTime,
) -> Option<NextReminder> {
    let task_items = tasks.iter().filter_map(|task| {
        if task.deleted_at.is_some() || task.status == "COMPLETED" {
            return None;
        }
        let time = task.reminder_time.as_deref().and_then(parse_datetime_any)?;
        (time > now).then(|| NextReminder {
            id: task.id.clone(),
            is_task: true,
            description: task.description.clone(),
            time,
        })
    });
    let recurring_items = recurring.iter().filter_map(|task| {
        if task.deleted_at.is_some() || task.is_paused {
            return None;
        }
        let time = parse_datetime_any(&task.next_trigger)?;
        (time > now).then(|| NextReminder {
            id: task.id.clone(),
            is_task: false,
            description: task.description.clone(),
            time,
        })
    });
    task_items
        .chain(recurring_items)
        .min_by_key(|item| item.time)
}

/// 今天只显示钟点，明天显示“明天”，今年内显示月日，更远的显示完整日期。
pub fn format_when(time: NaiveDateTime, now: NaiveDateTime) -> String {
    let clock = time.format("%H:%M");
    let date = time.date();
    if date == now.date() {
        clock.to_string()
    } else if date == now.date() + Duration::days(1) {
        format!("明天 {}", clock)
    } else if date.year() == now.year() {
        format!("{}月{}日 {}", date.month(), date.day(), clock)
    } else {
        format!("{} {}", date.format("%Y-%m-%d"), clock)
    }
}

/// 压成一行并截断过长的描述。
fn short_description(value: &str) -> String {
    let single_line = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = single_line.chars();
    let head: String = chars.by_ref().take(DESCRIPTION_MAX_CHARS).collect();
    if chars.next().is_some() {
        format!("{}…", head)
    } else if head.is_empty() {
        "（无标题）".to_string()
    } else {
        head
    }
}

fn next_line(next: Option<&NextReminder>, now: NaiveDateTime) -> String {
    match next {
        Some(item) => format!(
            "下一条：{} · {}",
            short_description(&item.description),
            format_when(item.time, now)
        ),
        None => "暂无待提醒事项".to_string(),
    }
}

/// 托盘提示文字。勿扰时段内附带积压的提醒数量。
pub fn tooltip_text(
    title: &str,
    next: Option<&NextReminder>,
    now: NaiveDateTime,
    quiet: bool,
    pending: usize,
) -> String {
    let mut lines = vec![title.to_string(), next_line(next, now)];
    if quiet {
        lines.push(if pending > 0 {
            format!("勿扰中 · {} 条待查看", pending)
        } else {
            "勿扰中".to_string()
        });
    }
    lines.join("\n")
}

fn app_title() -> String {
    if paths::is_dev_mode() {
        "任务提醒 [开发]".to_string()
    } else {
        "任务提醒".to_string()
    }
}

fn dev_tag() -> &'static str {
    if paths::is_dev_mode() {
        " [开发]"
    } else {
        ""
    }
}

fn build_menu(
    app: &AppHandle,
    next: Option<&NextReminder>,
    now: NaiveDateTime,
) -> tauri::Result<Menu<Wry>> {
    let mut builder = MenuBuilder::new(app).item(
        &MenuItemBuilder::with_id("next_info", next_line(next, now))
            .enabled(false)
            .build(app)?,
    );
    if let Some(item) = next.filter(|item| item.is_task) {
        builder = builder
            .text(format!("{}{}", NEXT_COMPLETE_PREFIX, item.id), "完成此待办")
            .text(
                format!("{}{}", NEXT_SNOOZE_PREFIX, item.id),
                format!("推迟 {} 分钟", TRAY_SNOOZE_MINUTES),
            );
    }
    builder
        .separator()
        .text("open", format!("打开{}", dev_tag()))
        .text("quick_add", "快速添加待办")
        .text("new_note", "新建便签")
        .text("sticky_show", "显示全部便签")
        .text("sticky_hide", "隐藏全部便签")
        .text("check_update", "检查更新")
        .text("sync_now", "立即同步")
        .separator()
        .text("quit", "退出")
        .build()
}

#[derive(Default)]
struct TrayCache {
    tooltip: String,
    menu_key: String,
}

fn tray_cache() -> &'static Mutex<TrayCache> {
    static CACHE: OnceLock<Mutex<TrayCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(TrayCache::default()))
}

/// 按当前数据刷新托盘提示与菜单；内容没变时不重建菜单。
fn refresh(app: &AppHandle) {
    let (Some(state), Some(tray)) = (app.try_state::<AppState>(), app.tray_by_id(TRAY_ID)) else {
        return;
    };
    let now = Local::now().naive_local();
    let tasks = state.db.list_active_tasks().unwrap_or_default();
    let recurring = state.db.list_recurring_tasks().unwrap_or_default();
    let next = find_next_reminder(&tasks, &recurring, now);
    let quiet = state
        .db
        .load_settings()
        .map(|settings| quiet_hours::is_quiet_now(&settings))
        .unwrap_or(false);
    let pending = state.scheduler.queue().snapshot().len();
    let tooltip = tooltip_text(&app_title(), next.as_ref(), now, quiet, pending);
    let menu_key = format!(
        "{}|{}",
        next_line(next.as_ref(), now),
        next.as_ref()
            .filter(|item| item.is_task)
            .map(|item| item.id.as_str())
            .unwrap_or("")
    );

    let mut cache = tray_cache().lock().unwrap_or_else(|e| e.into_inner());
    if cache.tooltip != tooltip && tray.set_tooltip(Some(&tooltip)).is_ok() {
        cache.tooltip = tooltip;
    }
    if cache.menu_key != menu_key {
        match build_menu(app, next.as_ref(), now).and_then(|menu| tray.set_menu(Some(menu))) {
            Ok(()) => cache.menu_key = menu_key,
            Err(err) => eprintln!("[tray] 更新托盘菜单失败: {}", err),
        }
    }
}

/// 在后台刷新托盘，避免在持有锁或主线程中等待菜单更新。
pub fn request_refresh(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || refresh(&app));
}

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

fn create_sticky_note(app: &AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        if let Err(err) = create_custom_sticky_note_via_app(
            app,
            state.inner(),
            &CreateStickyNotePayload::default(),
        ) {
            eprintln!("[tray] 新建便签失败: {}", err);
        }
    } else {
        let _ = app.emit("tray-create-sticky-note", ());
    }
}

fn complete_next(app: &AppHandle, task_id: &str) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if let Err(err) = complete_task_by_id(&state, task_id) {
        eprintln!("[tray] 完成待办失败: {}", err);
    }
}

fn snooze_next(app: &AppHandle, task_id: &str) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let until =
        format_datetime(&(Local::now().naive_local() + Duration::minutes(TRAY_SNOOZE_MINUTES)));
    if let Err(err) = reschedule_task_reminder(app, &state, task_id, &until) {
        eprintln!("[tray] 推迟待办失败: {}", err);
    }
}

pub fn setup_tray(app: &AppHandle) -> Result<(), tauri::Error> {
    let menu = build_menu(app, None, Local::now().naive_local())?;

    let mut tray_builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .tooltip(app_title())
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => {
                show_main(app);
            }
            "quick_add" => {
                if let Err(err) = crate::quick_add::show_window(app) {
                    eprintln!("[tray] 打开快速添加失败: {}", err);
                }
            }
            "new_note" => {
                create_sticky_note(app);
            }
            "sticky_show" => {
                sticky::show_all_sticky_windows(app);
            }
            "sticky_hide" => {
                sticky::hide_all_sticky_windows(app);
            }
            "check_update" => {
                show_main(app);
                let _ = app.emit("tray-check-update", ());
            }
            "sync_now" => {
                if let Some(state) = app.try_state::<AppState>() {
                    let _ = state.sync.request_sync("tray");
                }
            }
            "quit" => {
                app.exit(0);
            }
            id => {
                if let Some(task_id) = id.strip_prefix(NEXT_COMPLETE_PREFIX) {
                    complete_next(app, task_id);
                } else if let Some(task_id) = id.strip_prefix(NEXT_SNOOZE_PREFIX) {
                    snooze_next(app, task_id);
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        tray_builder = tray_builder.icon(icon.clone());
    }

    let _ = tray_builder.build(app)?;

    // 数据变化（本机修改、提醒触发、同步合并都会发出 data-updated）时刷新，另加定时刷新。
    let handle = app.clone();
    app.listen_any("data-updated", move |_| request_refresh(&handle));
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            request_refresh(&handle);
            tokio::time::sleep(std::time::Duration::from_secs(REFRESH_INTERVAL_SECONDS)).await;
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(value: &str) -> NaiveDateTime {
        parse_datetime_any(value).unwrap()
    }

    fn task(id: &str, reminder: Option<&str>) -> Task {
        Task {
            id: id.to_string(),
            description: format!("待办 {}", id),
            task_type: "TASK".to_string(),
            status: "PENDING".to_string(),
            created_at: "2026-09-01T00:00:00".to_string(),
            completed_at: None,
            reminder_time: reminder.map(str::to_string),
            updated_at: None,
            deleted_at: None,
            sticky_content: None,
            tags: Vec::new(),
            priority: 0,
        }
    }

    fn recurring(id: &str, next: &str, paused: bool) -> RecurringTask {
        RecurringTask {
            id: id.to_string(),
            description: format!("循环 {}", id),
            task_type: "RECURRING".to_string(),
            status: "PENDING".to_string(),
            created_at: "2026-09-01T00:00:00".to_string(),
            completed_at: None,
            reminder_time: None,
            updated_at: None,
            deleted_at: None,
            interval_minutes: 60,
            last_triggered: None,
            next_trigger: next.to_string(),
            is_paused: paused,
            start_time: None,
            end_time: None,
            repeat_mode: "DAILY".to_string(),
            schedule_time: Some("09:00".to_string()),
            schedule_weekday: None,
            schedule_weekdays: None,
            schedule_day: None,
            cron_expression: None,
        }
    }

    #[test]
    fn next_reminder_picks_earliest_future_item() {
        let now = dt("2026-09-27T10:00:00");
        let tasks = vec![
            task("past", Some("2026-09-27T09:00:00")),
            task("later", Some("2026-09-27T15:00:00")),
            task("none", None),
        ];
        let recurring_items = vec![
            recurring("paused", "2026-09-27T10:30:00", true),
            recurring("soon", "2026-09-27T11:00:00", false),
        ];
        let next = find_next_reminder(&tasks, &recurring_items, now).unwrap();
        assert_eq!(next.id, "soon");
        assert!(!next.is_task);

        let next = find_next_reminder(&tasks, &[], now).unwrap();
        assert_eq!(next.id, "later");
        assert!(next.is_task);

        let mut done = task("done", Some("2026-09-27T10:10:00"));
        done.status = "COMPLETED".to_string();
        assert_eq!(
            find_next_reminder(&[done], &[], now).map(|item| item.id),
            None
        );
    }

    #[test]
    fn when_is_relative_to_today() {
        let now = dt("2026-09-27T10:00:00");
        assert_eq!(format_when(dt("2026-09-27T15:30:00"), now), "15:30");
        assert_eq!(format_when(dt("2026-09-28T09:00:00"), now), "明天 09:00");
        assert_eq!(format_when(dt("2026-10-08T09:00:00"), now), "10月8日 09:00");
        assert_eq!(
            format_when(dt("2027-01-02T09:00:00"), now),
            "2027-01-02 09:00"
        );
    }

    #[test]
    fn tooltip_truncates_and_shows_quiet_state() {
        let now = dt("2026-09-27T10:00:00");
        let item = NextReminder {
            id: "t".to_string(),
            is_task: true,
            description: "一个非常非常非常非常非常非常非常长的待办描述\n第二行".to_string(),
            time: dt("2026-09-27T15:30:00"),
        };
        // 截断到 18 个字，换行压成空格。
        assert_eq!(
            tooltip_text("任务提醒", Some(&item), now, false, 0),
            "任务提醒\n下一条：一个非常非常非常非常非常非常非常长的… · 15:30"
        );
        assert_eq!(
            tooltip_text("任务提醒", None, now, true, 3),
            "任务提醒\n暂无待提醒事项\n勿扰中 · 3 条待查看"
        );
        assert_eq!(
            tooltip_text("任务提醒", None, now, true, 0),
            "任务提醒\n暂无待提醒事项\n勿扰中"
        );
    }
}
