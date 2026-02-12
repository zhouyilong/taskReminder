use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{Local, NaiveDateTime};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::time::sleep;

use crate::db::DbManager;
use crate::errors::AppError;
use crate::models::{NotificationPayload, RecurringTask, Task};
use crate::recurrence::{compute_next_trigger, sanitize_recurring_task, should_trigger_now};
use crate::sync::CloudSyncService;

#[derive(Clone)]
pub struct ReminderScheduler {
    app: AppHandle,
    db: DbManager,
    sync: CloudSyncService,
    recurring_jobs: Arc<Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>>,
    task_jobs: Arc<Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>>,
    snapshot: Arc<Mutex<Option<NotificationPayload>>>,
}

impl ReminderScheduler {
    pub fn new(
        app: AppHandle,
        db: DbManager,
        sync: CloudSyncService,
        snapshot: Arc<Mutex<Option<NotificationPayload>>>,
    ) -> Self {
        Self {
            app,
            db,
            sync,
            recurring_jobs: Arc::new(Mutex::new(HashMap::new())),
            task_jobs: Arc::new(Mutex::new(HashMap::new())),
            snapshot,
        }
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
        let Some(mut task) = self.db.get_recurring_task(&task_id)? else {
            return Ok(());
        };
        if task.deleted_at.is_some() || task.is_paused {
            return Ok(());
        }
        let now = Local::now().naive_local();
        if !should_trigger_now(&task, now)? {
            task.next_trigger = compute_next_trigger(&task, Some(now))?;
            self.db.update_recurring_task(&task)?;
            self.sync.notify_local_change()?;
            self.schedule_recurring(task)?;
            return Ok(());
        }
        sanitize_recurring_task(&mut task)?;
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
        };
        *self.snapshot.lock().unwrap() = Some(payload.clone());
        emit_notification(&self.app, &payload)?;

        self.schedule_recurring(task)?;
        Ok(())
    }

    fn handle_task(&self, task_id: String) -> Result<(), AppError> {
        let Some(task) = self.db.get_task(&task_id)? else {
            return Ok(());
        };
        if task.deleted_at.is_some() || task.status == "COMPLETED" {
            return Ok(());
        }
        if let Some(reminder_time) = &task.reminder_time {
            if !is_future(reminder_time)? {
                return Ok(());
            }
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
        };
        *self.snapshot.lock().unwrap() = Some(payload.clone());
        emit_notification(&self.app, &payload)?;
        Ok(())
    }
}

fn emit_notification(app: &AppHandle, payload: &NotificationPayload) -> Result<(), AppError> {
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
        .always_on_top(true)
        .resizable(false)
        .inner_size(380.0, 180.0)
        .build()
        .map_err(|e| AppError::System(e.to_string()))?
    };

    if let Ok(monitor) = window.current_monitor() {
        if let Some(monitor) = monitor {
            let size = monitor.size();
            let pos_x = size.width.saturating_sub(400) as f64;
            let pos_y = size.height.saturating_sub(220) as f64;
            let _ = window.set_position(tauri::LogicalPosition { x: pos_x, y: pos_y });
        }
    }

    window.emit("notification", payload).ok();
    window.show().ok();
    window.set_focus().ok();
    Ok(())
}

fn seconds_until(value: &str) -> Result<u64, AppError> {
    let target = parse_datetime(value)?;
    let now = Local::now().naive_local();
    let diff = target.signed_duration_since(now).num_seconds();
    Ok(diff.max(0) as u64)
}

fn parse_datetime(value: &str) -> Result<NaiveDateTime, AppError> {
    parse_datetime_any(value).ok_or_else(|| AppError::Invalid(format!("无法解析时间: {}", value)))
}

fn now_string() -> String {
    Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

pub fn is_future(value: &str) -> Result<bool, AppError> {
    let target = parse_datetime(value)?;
    Ok(target > Local::now().naive_local())
}

fn parse_datetime_any(value: &str) -> Option<NaiveDateTime> {
    let candidates = [
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M",
    ];
    for fmt in candidates {
        if let Ok(dt) = NaiveDateTime::parse_from_str(value, fmt) {
            return Some(dt);
        }
    }
    None
}
