//! 提醒触达：到点时在每块屏幕上播放的全屏注意效果、提醒弹窗所在屏幕的定位，
//! 以及提醒未处理时“再次提醒”的判断。
//!
//! 全屏效果窗口透明、置顶、鼠标穿透、不可聚焦，只负责视觉效果；处理提醒仍在弹窗中完成，
//! 队列清空时（`scheduler::publish_queue`）立即关闭。几何与时间计算为纯函数，便于测试。

use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{Duration, NaiveDateTime};
use serde::Serialize;
use tauri::{AppHandle, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewUrl};

use crate::errors::AppError;
use crate::models::NotificationPayload;

pub const EFFECT_OFF: &str = "off";
pub const EFFECT_EDGE: &str = "edge";
pub const EFFECT_BANNER: &str = "banner";

pub const SCREEN_CURSOR: &str = "cursor";
pub const SCREEN_PRIMARY: &str = "primary";

/// 全屏效果窗口标签前缀：`attention-<批次>-<序号>`。
pub const LABEL_PREFIX: &str = "attention-";
/// 一次效果的时长（毫秒），与前端 `ATTENTION_DURATION_MS` 一致；之后淡出并关闭窗口。
pub const EFFECT_DURATION_MS: u64 = 6_000;
/// 关闭窗口前多等一会儿，让前端的淡出动画播完。
const CLOSE_GRACE_MS: u64 = 600;
/// 预览“直到处理”时额外展示的时长，让设置中能看到收起后的样子。
const PREVIEW_PERSIST_EXTRA_MS: u64 = 5_000;

/// 不认识的效果按默认（光晕 + 横幅）处理。
pub fn normalize_effect(value: &str) -> String {
    match value.trim() {
        EFFECT_OFF => EFFECT_OFF,
        EFFECT_EDGE => EFFECT_EDGE,
        _ => EFFECT_BANNER,
    }
    .to_string()
}

/// 不认识的屏幕选项按默认（鼠标所在屏幕）处理。
pub fn normalize_screen(value: &str) -> String {
    match value.trim() {
        SCREEN_PRIMARY => SCREEN_PRIMARY,
        _ => SCREEN_CURSOR,
    }
    .to_string()
}

/// 全屏效果目前只在 Windows 上启用：Linux WebKitGTK 的透明窗口首帧后不重绘。
pub fn supported() -> bool {
    cfg!(target_os = "windows")
}

/// 物理像素下的矩形（屏幕或工作区）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl PhysRect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn center(&self) -> (f64, f64) {
        (
            self.x as f64 + self.width as f64 / 2.0,
            self.y as f64 + self.height as f64 / 2.0,
        )
    }
}

/// 一块屏幕：整块区域、工作区（扣除任务栏）与缩放比例。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub bounds: PhysRect,
    pub work_area: PhysRect,
    pub scale: f64,
}

impl Screen {
    fn from_monitor(monitor: &Monitor) -> Self {
        let position = monitor.position();
        let size = monitor.size();
        let work = monitor.work_area();
        let bounds = PhysRect::new(position.x, position.y, size.width, size.height);
        let work_area = if work.size.width == 0 || work.size.height == 0 {
            bounds
        } else {
            PhysRect::new(
                work.position.x,
                work.position.y,
                work.size.width,
                work.size.height,
            )
        };
        let scale = monitor.scale_factor();
        Self {
            bounds,
            work_area,
            scale: if scale.is_finite() && scale > 0.0 {
                scale
            } else {
                1.0
            },
        }
    }
}

/// 提醒弹窗左上角的位置（物理像素）：贴在工作区右下角，四周留 `margin`（逻辑像素）。
/// 尺寸按目标屏幕的缩放换算，工作区比弹窗还小时贴左上角。
pub fn popup_position(screen: &Screen, width: f64, height: f64, margin: f64) -> (i32, i32) {
    let area = screen.work_area;
    let scale = screen.scale;
    let width = (width * scale).round() as i64;
    let height = (height * scale).round() as i64;
    let margin = (margin * scale).round() as i64;
    let right = area.x as i64 + area.width as i64;
    let bottom = area.y as i64 + area.height as i64;
    let x = (right - width - margin).max(area.x as i64);
    let y = (bottom - height - margin).max(area.y as i64);
    (x as i32, y as i32)
}

/// 弹窗所在屏幕相对于某块屏幕的方向，供横幅提示“提醒弹窗在左侧屏幕”。
pub fn popup_direction(screen: &PhysRect, popup: &PhysRect) -> &'static str {
    if screen == popup {
        return "here";
    }
    let (sx, sy) = screen.center();
    let (px, py) = popup.center();
    let (dx, dy) = (px - sx, py - sy);
    if dx.abs() >= dy.abs() {
        if dx < 0.0 {
            "left"
        } else {
            "right"
        }
    } else if dy < 0.0 {
        "up"
    } else {
        "down"
    }
}

/// 未处理的提醒是否该再次提醒：开启了间隔，且距上次提醒（弹出、勿扰结束、再次提醒）已满间隔。
pub fn nudge_due(
    last_alert: Option<NaiveDateTime>,
    now: NaiveDateTime,
    repeat_minutes: i64,
) -> bool {
    if repeat_minutes <= 0 {
        return false;
    }
    match last_alert {
        Some(last) => now - last >= Duration::minutes(repeat_minutes),
        None => true,
    }
}

/// 一次效果中各屏幕共用的内容；`popup_direction` 按屏幕分别填写。
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AttentionPayload {
    pub effect: String,
    pub until_handled: bool,
    pub reminder_type: String,
    pub description: String,
    pub scheduled_time: Option<String>,
    /// 队列中的提醒总数（含展示的这一条）。
    pub count: usize,
    /// 再次提醒（而不是新到的提醒）。
    pub repeat: bool,
    pub preview: bool,
    /// 提醒弹窗的主题设置（`system` / `app` / `light` / `dark`），横幅跟随它。
    pub theme: String,
    pub popup_direction: String,
}

/// 效果的设置，来自 `AppSettings` 或设置界面中的草稿（预览）。
#[derive(Clone, Debug)]
pub struct EffectOptions {
    pub effect: String,
    pub until_handled: bool,
    pub theme: String,
}

impl AttentionPayload {
    pub fn for_queue(
        queue: &[NotificationPayload],
        options: &EffectOptions,
        repeat: bool,
    ) -> Option<Self> {
        let head = queue.first()?;
        Some(Self {
            effect: normalize_effect(&options.effect),
            until_handled: options.until_handled,
            reminder_type: head.reminder_type.as_str().to_string(),
            description: head.description.clone(),
            scheduled_time: head.scheduled_time.clone(),
            count: queue.len(),
            repeat,
            preview: false,
            theme: options.theme.clone(),
            popup_direction: "here".to_string(),
        })
    }

    pub fn preview(options: &EffectOptions) -> Self {
        Self {
            effect: normalize_effect(&options.effect),
            until_handled: options.until_handled,
            reminder_type: "TASK".to_string(),
            description: "这是全屏提醒效果的预览".to_string(),
            scheduled_time: None,
            count: 1,
            repeat: false,
            preview: true,
            theme: options.theme.clone(),
            popup_direction: "here".to_string(),
        }
    }
}

static BATCH: AtomicU64 = AtomicU64::new(0);

fn window_label(batch: u64, index: usize) -> String {
    format!("{}{}-{}", LABEL_PREFIX, batch, index)
}

/// 页面加载前注入效果内容，避免窗口出现后再等事件。
fn bootstrap_script(payload: &AttentionPayload) -> Option<String> {
    let json = serde_json::to_string(payload).ok()?;
    Some(format!("window.__ATTENTION__ = {};", json))
}

/// 提醒弹窗应出现的屏幕：按设置取鼠标所在屏幕或主屏幕，取不到时回退到主屏幕、第一块屏幕。
pub fn target_screen(app: &AppHandle, screen_setting: &str) -> Option<Screen> {
    let from_cursor = || {
        let cursor = app.cursor_position().ok()?;
        app.monitor_from_point(cursor.x, cursor.y).ok().flatten()
    };
    let monitor = if normalize_screen(screen_setting) == SCREEN_CURSOR {
        from_cursor()
    } else {
        None
    }
    .or_else(|| app.primary_monitor().ok().flatten())
    .or_else(|| {
        app.available_monitors()
            .ok()
            .and_then(|list| list.into_iter().next())
    })?;
    Some(Screen::from_monitor(&monitor))
}

/// 关闭全部全屏效果窗口（提醒都处理完、或新一批效果开始前）。
pub fn dismiss(app: &AppHandle) {
    BATCH.fetch_add(1, Ordering::SeqCst);
    close_windows(app, None);
}

/// 关闭效果窗口；`keep_batch` 为某个批次时只关闭其他批次的窗口。
fn close_windows(app: &AppHandle, keep_batch: Option<u64>) {
    let keep_prefix = keep_batch.map(|batch| format!("{}{}-", LABEL_PREFIX, batch));
    for (label, window) in app.webview_windows() {
        if !label.starts_with(LABEL_PREFIX) {
            continue;
        }
        if keep_prefix
            .as_deref()
            .is_some_and(|prefix| label.starts_with(prefix))
        {
            continue;
        }
        let _ = window.destroy();
    }
}

/// 在每块屏幕上播放一次效果。窗口在后台线程中创建（在主线程同步创建窗口会死锁），
/// 调用方不用等待。`popup` 为提醒弹窗所在屏幕，用于提示弹窗方向。
pub fn play(app: &AppHandle, payload: AttentionPayload, popup: Option<Screen>) {
    if !supported() || payload.effect == EFFECT_OFF {
        return;
    }
    let batch = BATCH.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(err) = open_windows(&app, batch, &payload, popup) {
            eprintln!("[attention] 打开全屏提醒效果失败: {}", err);
        }
        let persist = payload.until_handled && !payload.preview;
        if persist {
            return;
        }
        let mut wait = EFFECT_DURATION_MS + CLOSE_GRACE_MS;
        if payload.preview && payload.until_handled {
            wait += PREVIEW_PERSIST_EXTRA_MS;
        }
        std::thread::sleep(std::time::Duration::from_millis(wait));
        // 期间又开始了新一批（或已处理完）时，不要关掉新的窗口。
        if BATCH.load(Ordering::SeqCst) == batch {
            close_windows(&app, None);
        }
    });
}

/// 创建过程中提醒已处理完或开始了新一批：关掉这一批已打开的窗口（保留最新一批的）。
fn superseded(app: &AppHandle, batch: u64) -> bool {
    let current = BATCH.load(Ordering::SeqCst);
    if current == batch {
        return false;
    }
    close_windows(app, Some(current));
    true
}

fn open_windows(
    app: &AppHandle,
    batch: u64,
    payload: &AttentionPayload,
    popup: Option<Screen>,
) -> Result<(), AppError> {
    close_windows(app, Some(batch));
    let monitors = app
        .available_monitors()
        .map_err(|e| AppError::System(e.to_string()))?;
    for (index, monitor) in monitors.iter().enumerate() {
        if superseded(app, batch) {
            return Ok(());
        }
        let screen = Screen::from_monitor(monitor);
        let mut screen_payload = payload.clone();
        screen_payload.popup_direction = popup
            .map(|popup| popup_direction(&screen.bounds, &popup.bounds))
            .unwrap_or("here")
            .to_string();
        let mut builder = tauri::WebviewWindowBuilder::new(
            app,
            window_label(batch, index),
            WebviewUrl::App("attention.html".into()),
        )
        .title("提醒")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .focusable(false)
        .visible(false);
        if let Some(script) = bootstrap_script(&screen_payload) {
            builder = builder.initialization_script(script);
        }
        let window = builder
            .build()
            .map_err(|e| AppError::System(e.to_string()))?;
        // 先移到目标屏幕再设尺寸：跨缩放比例移动时系统会按新屏幕调整窗口大小。
        let bounds = screen.bounds;
        let _ = window.set_position(PhysicalPosition::new(bounds.x, bounds.y));
        let _ = window.set_size(PhysicalSize::new(bounds.width, bounds.height));
        let _ = window.set_position(PhysicalPosition::new(bounds.x, bounds.y));
        let _ = window.set_ignore_cursor_events(true);
        let _ = window.show();
    }
    if superseded(app, batch) {
        return Ok(());
    }
    // 效果窗口与提醒弹窗都置顶，后打开的会盖在弹窗上；把弹窗重新提到最上层。
    if let Some(popup_window) = app.get_webview_window("notification") {
        if popup_window.is_visible().unwrap_or(false) {
            let _ = popup_window.set_always_on_top(true);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kinds::ReminderKind;
    use crate::time::parse_datetime_any;

    fn screen(bounds: PhysRect, work_area: PhysRect, scale: f64) -> Screen {
        Screen {
            bounds,
            work_area,
            scale,
        }
    }

    #[test]
    fn normalizes_unknown_options_to_defaults() {
        assert_eq!(normalize_effect("off"), "off");
        assert_eq!(normalize_effect(" edge "), "edge");
        assert_eq!(normalize_effect(""), "banner");
        assert_eq!(normalize_effect("fireworks"), "banner");
        assert_eq!(normalize_screen("primary"), "primary");
        assert_eq!(normalize_screen(""), "cursor");
        assert_eq!(normalize_screen("left"), "cursor");
    }

    #[test]
    fn popup_sits_in_the_work_area_corner_at_100_percent() {
        // 1920×1080，任务栏在底部（40 像素）。
        let main = screen(
            PhysRect::new(0, 0, 1920, 1080),
            PhysRect::new(0, 0, 1920, 1040),
            1.0,
        );
        assert_eq!(popup_position(&main, 392.0, 248.0, 16.0), (1512, 776));
    }

    #[test]
    fn popup_scales_with_the_target_screen() {
        // 150% 缩放：弹窗与边距都按物理像素换算，不会偏出屏幕。
        let scaled = screen(
            PhysRect::new(0, 0, 2880, 1620),
            PhysRect::new(0, 0, 2880, 1560),
            1.5,
        );
        assert_eq!(popup_position(&scaled, 392.0, 248.0, 16.0), (2268, 1164));
    }

    #[test]
    fn popup_on_a_secondary_screen_uses_its_origin() {
        // 副屏在主屏左侧（负坐标），任务栏在顶部。
        let left = screen(
            PhysRect::new(-1920, 0, 1920, 1080),
            PhysRect::new(-1920, 48, 1920, 1032),
            1.0,
        );
        assert_eq!(popup_position(&left, 392.0, 248.0, 16.0), (-408, 816));
        // 任务栏在左侧：右下角不受影响。
        let side = screen(
            PhysRect::new(1920, -200, 1280, 1024),
            PhysRect::new(1982, -200, 1218, 1024),
            1.0,
        );
        assert_eq!(popup_position(&side, 392.0, 248.0, 16.0), (2792, 560));
    }

    #[test]
    fn popup_never_leaves_a_tiny_work_area() {
        let tiny = screen(
            PhysRect::new(100, 100, 300, 200),
            PhysRect::new(100, 100, 300, 200),
            1.0,
        );
        assert_eq!(popup_position(&tiny, 392.0, 248.0, 16.0), (100, 100));
    }

    #[test]
    fn direction_points_from_each_screen_to_the_popup() {
        let main = PhysRect::new(0, 0, 1920, 1080);
        let left = PhysRect::new(-1920, 0, 1920, 1080);
        let top = PhysRect::new(0, -1080, 1920, 1080);
        assert_eq!(popup_direction(&main, &main), "here");
        assert_eq!(popup_direction(&left, &main), "right");
        assert_eq!(popup_direction(&main, &left), "left");
        assert_eq!(popup_direction(&main, &top), "up");
        assert_eq!(popup_direction(&top, &main), "down");
    }

    #[test]
    fn nudge_waits_for_the_full_interval() {
        let at = |value: &str| parse_datetime_any(value).unwrap();
        let last = Some(at("2026-10-13T10:00:00"));
        assert!(!nudge_due(last, at("2026-10-13T10:04:59"), 5));
        assert!(nudge_due(last, at("2026-10-13T10:05:00"), 5));
        assert!(nudge_due(last, at("2026-10-13T10:12:00"), 5));
        // 关闭再次提醒时从不提醒。
        assert!(!nudge_due(last, at("2026-10-13T12:00:00"), 0));
        assert!(!nudge_due(None, at("2026-10-13T12:00:00"), 0));
        // 没有记录到上次提醒时间（如应用刚启动）时立即提醒。
        assert!(nudge_due(None, at("2026-10-13T12:00:00"), 2));
    }

    fn notification(record_id: &str, kind: ReminderKind) -> NotificationPayload {
        NotificationPayload {
            record_id: record_id.to_string(),
            reminder_id: format!("task-{}", record_id),
            reminder_type: kind,
            description: format!("提醒 {}", record_id),
            snooze_minutes: 5,
            scheduled_time: Some("2026-10-13T08:30:00".to_string()),
            sound: true,
        }
    }

    #[test]
    fn payload_shows_the_queue_head_and_count() {
        let options = EffectOptions {
            effect: "unknown".to_string(),
            until_handled: true,
            theme: "dark".to_string(),
        };
        assert!(AttentionPayload::for_queue(&[], &options, false).is_none());
        let queue = vec![
            notification("a", ReminderKind::Recurring),
            notification("b", ReminderKind::Task),
        ];
        let payload = AttentionPayload::for_queue(&queue, &options, true).unwrap();
        assert_eq!(payload.effect, "banner");
        assert!(payload.until_handled);
        assert_eq!(payload.reminder_type, "RECURRING");
        assert_eq!(payload.description, "提醒 a");
        assert_eq!(
            payload.scheduled_time.as_deref(),
            Some("2026-10-13T08:30:00")
        );
        assert_eq!(payload.count, 2);
        assert!(payload.repeat);
        assert!(!payload.preview);
        let preview = AttentionPayload::preview(&options);
        assert!(preview.preview);
        assert_eq!(preview.count, 1);
    }

    #[test]
    fn bootstrap_script_injects_camel_case_payload() {
        let payload = AttentionPayload::preview(&EffectOptions {
            effect: "edge".to_string(),
            until_handled: false,
            theme: "app".to_string(),
        });
        let script = bootstrap_script(&payload).unwrap();
        assert!(script.starts_with("window.__ATTENTION__ = {"));
        assert!(script.contains("\"effect\":\"edge\""));
        assert!(script.contains("\"untilHandled\":false"));
        assert!(script.contains("\"popupDirection\":\"here\""));
        assert_eq!(window_label(3, 1), "attention-3-1");
    }
}
