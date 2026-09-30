use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{Duration, Local, NaiveDateTime};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::time::sleep;

use crate::db::DbManager;
use crate::errors::AppError;
use crate::models::{AppSettings, NotificationPayload, RecurringTask, Task};
use crate::notification_queue::NotificationQueue;
use crate::quiet_hours;
use crate::recurrence::{
    compute_next_trigger, refreshed_workday_trigger, sanitize_recurring_task, should_trigger_now,
};
use crate::sync::CloudSyncService;
use crate::time::{now_string, parse_datetime_any};

/// 校准巡检间隔：兜底系统休眠/唤醒、修改系统时间、云同步带来的新提醒等
/// 单次 sleep 计时器覆盖不到的情况。
const WATCHDOG_INTERVAL_SECONDS: u64 = 30;
/// 启动后首次巡检的延迟，等主窗口与状态初始化完成后再补发错过的提醒。
const WATCHDOG_STARTUP_DELAY_SECONDS: u64 = 5;
/// 一次性提醒的补发回溯窗口：超过该天数仍未触发的提醒不再补发，
/// 避免长期未使用后一次性弹出大量陈旧提醒。
const MISSED_REMINDER_LOOKBACK_DAYS: i64 = 7;
/// 弹窗队列更新事件。
pub const NOTIFICATION_QUEUE_EVENT: &str = "notification-queue";

#[derive(Clone)]
pub struct ReminderScheduler {
    app: AppHandle,
    db: DbManager,
    sync: CloudSyncService,
    recurring_jobs: Arc<Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>>,
    task_jobs: Arc<Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>>,
    queue: NotificationQueue,
    /// 串行化“判断是否到点 + 写记录 + 推进下次时间”，
    /// 防止计时器与巡检同时命中同一条提醒而重复触发。
    fire_lock: Arc<Mutex<()>>,
    /// 勿扰时段内有提醒进入队列但没有弹窗；勿扰结束后由巡检弹出。
    quiet_held: Arc<AtomicBool>,
}

impl ReminderScheduler {
    pub fn new(
        app: AppHandle,
        db: DbManager,
        sync: CloudSyncService,
        queue: NotificationQueue,
    ) -> Self {
        Self {
            app,
            db,
            sync,
            recurring_jobs: Arc::new(Mutex::new(HashMap::new())),
            task_jobs: Arc::new(Mutex::new(HashMap::new())),
            queue,
            fire_lock: Arc::new(Mutex::new(())),
            quiet_held: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 启动校准巡检：首次巡检会补发应用未运行期间错过的一次性提醒，
    /// 之后每隔一段时间检查一次已到点但未触发的提醒。
    pub fn start_watchdog(&self) {
        let scheduler = self.clone();
        tauri::async_runtime::spawn(async move {
            sleep(std::time::Duration::from_secs(
                WATCHDOG_STARTUP_DELAY_SECONDS,
            ))
            .await;
            loop {
                if let Err(err) = scheduler.fire_due() {
                    eprintln!("[scheduler] 巡检提醒失败: {}", err);
                }
                if let Err(err) = scheduler.release_quiet_hold() {
                    eprintln!("[scheduler] 勿扰结束弹出提醒失败: {}", err);
                }
                sleep(std::time::Duration::from_secs(WATCHDOG_INTERVAL_SECONDS)).await;
            }
        });
    }

    /// 触发所有已到点但尚未触发的提醒。`handle_*` 自身是幂等的，
    /// 与计时器重复命中时不会产生重复提醒。
    pub fn fire_due(&self) -> Result<(), AppError> {
        let now = Local::now().naive_local();
        let lookback = now - Duration::days(MISSED_REMINDER_LOOKBACK_DAYS);
        for task in self.db.list_active_tasks()? {
            let Some(reminder) = task.reminder_time.as_deref().and_then(parse_datetime_any) else {
                continue;
            };
            if reminder <= now && reminder >= lookback {
                if let Err(err) = self.handle_task(task.id) {
                    eprintln!("[scheduler] 补发待办提醒失败: {}", err);
                }
            }
        }
        for task in self.db.list_recurring_tasks()? {
            if task.is_paused {
                continue;
            }
            let due = parse_datetime_any(&task.next_trigger).is_some_and(|next| next <= now);
            if due {
                if let Err(err) = self.handle_recurring(task.id) {
                    eprintln!("[scheduler] 补发循环提醒失败: {}", err);
                }
            }
        }
        Ok(())
    }

    pub fn queue(&self) -> &NotificationQueue {
        &self.queue
    }

    /// 勿扰时段已结束（或被关闭）时，弹出勿扰期间积压的提醒。
    pub fn release_quiet_hold(&self) -> Result<(), AppError> {
        if !self.quiet_held.load(Ordering::SeqCst) {
            return Ok(());
        }
        let settings = self.db.load_settings()?;
        if quiet_hours::is_quiet_now(&settings) {
            return Ok(());
        }
        self.quiet_held.store(false, Ordering::SeqCst);
        let queue = self.queue.snapshot();
        if !queue.is_empty() {
            emit_notification(&self.app, &queue)?;
            if settings.native_notification_enabled {
                send_native_notification(
                    &self.app,
                    "勿扰已结束",
                    &format!("勿扰期间有 {} 条提醒待处理", queue.len()),
                );
            }
        }
        Ok(())
    }

    /// 提醒入队后展示：勿扰时段内只更新已打开的弹窗（不主动弹出），否则弹窗，
    /// 并按设置同时发送系统原生通知。
    fn present(
        &self,
        payload: NotificationPayload,
        settings: &AppSettings,
    ) -> Result<(), AppError> {
        let title = native_notification_title(&payload, Local::now().naive_local());
        let body = payload.description.clone();
        let queue = self.queue.push(payload);
        if quiet_hours::is_quiet_now(settings) {
            self.quiet_held.store(true, Ordering::SeqCst);
            publish_queue(&self.app, &queue);
            return Ok(());
        }
        emit_notification(&self.app, &queue)?;
        if settings.native_notification_enabled {
            send_native_notification(&self.app, &title, &body);
        }
        Ok(())
    }

    /// 从弹窗队列中撤下某个任务的提醒（例如在主窗口中完成或删除了该任务），
    /// 并把对应的提醒记录标记为 `action`。
    pub fn withdraw_notifications(&self, reminder_id: &str, action: &str) -> Result<(), AppError> {
        let removed = self.queue.remove_reminder(reminder_id);
        if removed.is_empty() {
            return Ok(());
        }
        for item in &removed {
            self.db
                .update_reminder_record_action(&item.record_id, action)?;
        }
        publish_queue(&self.app, &self.queue.snapshot());
        Ok(())
    }

    pub fn schedule_existing(&self) -> Result<(), AppError> {
        let recurring = self.db.list_recurring_tasks()?;
        for task in recurring {
            if !task.is_paused {
                self.schedule_recurring(task)?;
            }
        }
        let tasks = self.db.list_active_tasks()?;
        for task in tasks {
            if let Some(reminder) = &task.reminder_time {
                if is_future(reminder)? {
                    self.schedule_task(task)?;
                }
            }
        }
        Ok(())
    }

    /// 节假日数据更新后，重新计算运行中“法定工作日”提醒的下次触发并重新计时。
    /// 只写入时间确实变化的提醒，返回更新的数量。
    pub fn reschedule_workday_tasks(&self) -> Result<usize, AppError> {
        let _guard = self.fire_lock.lock().unwrap_or_else(|e| e.into_inner());
        let now = Local::now().naive_local();
        let mut updated = 0;
        for mut task in self.db.list_recurring_tasks()? {
            let Some(next) = refreshed_workday_trigger(&task, now)? else {
                continue;
            };
            task.next_trigger = next;
            self.db.update_recurring_task(&task)?;
            self.schedule_recurring(task)?;
            updated += 1;
        }
        if updated > 0 {
            self.sync.notify_local_change()?;
        }
        Ok(updated)
    }

    pub fn schedule_recurring(&self, task: RecurringTask) -> Result<(), AppError> {
        self.cancel_recurring(&task.id);
        if task.is_paused {
            return Ok(());
        }
        let delay = seconds_until(&task.next_trigger)?;
        let scheduler = self.clone();
        let task_id = task.id.clone();
        let handle = tauri::async_runtime::spawn(async move {
            sleep(std::time::Duration::from_secs(delay)).await;
            let _ = scheduler.handle_recurring(task_id);
        });
        self.recurring_jobs.lock().unwrap().insert(task.id, handle);
        Ok(())
    }

    pub fn schedule_task(&self, task: Task) -> Result<(), AppError> {
        self.cancel_task(&task.id);
        let Some(reminder_time) = task.reminder_time.clone() else {
            return Ok(());
        };
        let delay = seconds_until(&reminder_time)?;
        let scheduler = self.clone();
        let task_id = task.id.clone();
        let handle = tauri::async_runtime::spawn(async move {
            sleep(std::time::Duration::from_secs(delay)).await;
            let _ = scheduler.handle_task(task_id);
        });
        self.task_jobs.lock().unwrap().insert(task.id, handle);
        Ok(())
    }

    pub fn cancel_recurring(&self, task_id: &str) {
        if let Some(handle) = self.recurring_jobs.lock().unwrap().remove(task_id) {
            handle.abort();
        }
    }

    pub fn cancel_task(&self, task_id: &str) {
        if let Some(handle) = self.task_jobs.lock().unwrap().remove(task_id) {
            handle.abort();
        }
    }

    fn handle_recurring(&self, task_id: String) -> Result<(), AppError> {
        let _guard = self.fire_lock.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut task) = self.db.get_recurring_task(&task_id)? else {
            return Ok(());
        };
        if task.deleted_at.is_some() || task.is_paused {
            return Ok(());
        }
        let now = Local::now().naive_local();
        // 计时器可能因精度或系统休眠/唤醒而提前触发；若尚未到达本次计划触发时间，
        // 则仅重新排程而不触发提醒，避免在同一秒内反复触发产生大量重复记录。
        if let Ok(scheduled) = parse_datetime(&task.next_trigger) {
            if now < scheduled {
                self.schedule_recurring(task)?;
                return Ok(());
            }
        }
        if !should_trigger_now(&task, now)? {
            task.next_trigger = compute_next_trigger(&task, Some(now))?;
            self.db.update_recurring_task(&task)?;
            self.sync.notify_local_change()?;
            self.schedule_recurring(task)?;
            return Ok(());
        }
        sanitize_recurring_task(&mut task)?;
        let scheduled_time = task.next_trigger.clone();
        task.last_triggered = Some(now_string());
        task.next_trigger = compute_next_trigger(&task, Some(now))?;
        self.db.update_recurring_task(&task)?;

        let record = self
            .db
            .create_reminder_record(&task.id, &task.description, "RECURRING")?;
        self.sync.notify_local_change()?;
        let settings = self.db.load_settings()?;
        let payload = NotificationPayload {
            record_id: record.id.clone(),
            reminder_id: task.id.clone(),
            reminder_type: "RECURRING".to_string(),
            description: task.description.clone(),
            snooze_minutes: settings.snooze_minutes,
            scheduled_time: Some(scheduled_time),
        };
        self.present(payload, &settings)?;

        self.schedule_recurring(task)?;
        Ok(())
    }

    fn handle_task(&self, task_id: String) -> Result<(), AppError> {
        let _guard = self.fire_lock.lock().unwrap_or_else(|e| e.into_inner());
        let Some(task) = self.db.get_task(&task_id)? else {
            return Ok(());
        };
        if task.deleted_at.is_some() || task.status == "COMPLETED" {
            return Ok(());
        }
        let Some(reminder_time) = task.reminder_time.clone() else {
            return Ok(());
        };
        // 与循环提醒一致：计时器若提前醒来，只重新排程。
        // 等待时间向上取整后，到点或略过目标时间才会真正弹出。
        if is_future(&reminder_time)? {
            self.schedule_task(task)?;
            return Ok(());
        }
        // 本次提醒时间之后已有提醒记录，说明已经触发过（计时器与巡检重复命中、
        // 或其他设备已触发并同步过来），不再重复弹出。
        let scheduled = parse_datetime(&reminder_time)?;
        if self.db.has_reminder_record_since(&task.id, &scheduled)? {
            return Ok(());
        }

        let record = self
            .db
            .create_reminder_record(&task.id, &task.description, "TASK")?;
        self.sync.notify_local_change()?;
        let settings = self.db.load_settings()?;
        let payload = NotificationPayload {
            record_id: record.id.clone(),
            reminder_id: task.id.clone(),
            reminder_type: "TASK".to_string(),
            description: task.description.clone(),
            snooze_minutes: settings.snooze_minutes,
            scheduled_time: Some(reminder_time),
        };
        self.present(payload, &settings)?;
        Ok(())
    }
}

/// 系统通知标题：晚于原定时间 2 分钟以上的（错过的提醒）标注原定时间。
fn native_notification_title(payload: &NotificationPayload, now: NaiveDateTime) -> String {
    let scheduled = payload
        .scheduled_time
        .as_deref()
        .and_then(parse_datetime_any);
    match scheduled {
        Some(time) if now - time > Duration::minutes(2) => {
            format!("错过的提醒 · 原定 {}", time.format("%H:%M"))
        }
        _ => "任务提醒".to_string(),
    }
}

/// 发送系统原生通知（全屏程序中也能看到）；失败只记日志，不影响应用内弹窗。
fn send_native_notification(app: &AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    if let Err(err) = app.notification().builder().title(title).body(body).show() {
        eprintln!("[scheduler] 发送系统通知失败: {}", err);
    }
}

/// 把当前队列推送给提醒弹窗。队列为空时隐藏弹窗。
pub fn publish_queue(app: &AppHandle, queue: &[NotificationPayload]) {
    let Some(window) = app.get_webview_window("notification") else {
        return;
    };
    window.emit(NOTIFICATION_QUEUE_EVENT, queue).ok();
    if queue.is_empty() {
        window.hide().ok();
    }
}

fn emit_notification(app: &AppHandle, queue: &[NotificationPayload]) -> Result<(), AppError> {
    let notification_width = 392.0;
    let notification_height = if cfg!(target_os = "linux") {
        228.0
    } else {
        248.0
    };
    let window = if let Some(existing) = app.get_webview_window("notification") {
        existing
    } else {
        WebviewWindowBuilder::new(
            app,
            "notification",
            WebviewUrl::App("notification.html".into()),
        )
        .title("提醒通知")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .resizable(false)
        .inner_size(notification_width, notification_height)
        .build()
        .map_err(|e| AppError::System(e.to_string()))?
    };
    let _ = window.set_size(tauri::Size::Logical(tauri::LogicalSize::new(
        notification_width,
        notification_height,
    )));
    let _ = window.set_shadow(false);

    if let Ok(Some(monitor)) = window.current_monitor() {
        let size = monitor.size();
        let bottom_margin = if cfg!(target_os = "windows") { 84 } else { 48 };
        let pos_x =
            size.width
                .saturating_sub((notification_width as u32).saturating_add(16)) as f64;
        let pos_y = size
            .height
            .saturating_sub(notification_height as u32 + bottom_margin) as f64;
        let _ = window.set_position(tauri::LogicalPosition { x: pos_x, y: pos_y });
    }

    window.emit(NOTIFICATION_QUEUE_EVENT, queue).ok();
    window.show().ok();
    window.set_focus().ok();
    Ok(())
}

fn seconds_until(value: &str) -> Result<u64, AppError> {
    let target = parse_datetime(value)?;
    let now = Local::now().naive_local();
    let millis = target.signed_duration_since(now).num_milliseconds();
    if millis <= 0 {
        return Ok(0);
    }
    // 向上取整到整秒，避免毫秒被截断导致计时器在目标时间前被唤醒，
    // 进而在同一秒内反复触发、产生大量重复记录。
    Ok(((millis + 999) / 1000) as u64)
}

fn parse_datetime(value: &str) -> Result<NaiveDateTime, AppError> {
    parse_datetime_any(value).ok_or_else(|| AppError::Invalid(format!("无法解析时间: {}", value)))
}

pub fn is_future(value: &str) -> Result<bool, AppError> {
    let target = parse_datetime(value)?;
    Ok(target > Local::now().naive_local())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(scheduled: Option<&str>) -> NotificationPayload {
        NotificationPayload {
            record_id: "r".to_string(),
            reminder_id: "t".to_string(),
            reminder_type: "TASK".to_string(),
            description: "开会".to_string(),
            snooze_minutes: 5,
            scheduled_time: scheduled.map(str::to_string),
        }
    }

    #[test]
    fn native_title_marks_missed_reminders() {
        let now = parse_datetime("2026-09-27T10:00:00").unwrap();
        assert_eq!(
            native_notification_title(&payload(Some("2026-09-27T10:00:00")), now),
            "任务提醒"
        );
        assert_eq!(
            native_notification_title(&payload(Some("2026-09-27T09:59:00")), now),
            "任务提醒"
        );
        assert_eq!(
            native_notification_title(&payload(Some("2026-09-27T08:30:00")), now),
            "错过的提醒 · 原定 08:30"
        );
        assert_eq!(native_notification_title(&payload(None), now), "任务提醒");
    }
}
