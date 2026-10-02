//! 云同步服务：自动同步的调度（防抖、节流、启动同步）、手动同步与更换同步密码。

use super::*;

#[derive(Clone)]
pub struct CloudSyncService {
    app: AppHandle,
    db: DbManager,
    sync_in_progress: Arc<AtomicBool>,
    scheduled: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
    pending: Arc<Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
    next_auto_sync_due: Arc<Mutex<Option<NaiveDateTime>>>,
    local_change_seq: Arc<AtomicU64>,
    dirty: Arc<AtomicBool>,
}

impl CloudSyncService {
    pub fn new(app: AppHandle, db: DbManager) -> Self {
        Self {
            app,
            db,
            sync_in_progress: Arc::new(AtomicBool::new(false)),
            scheduled: Arc::new(Mutex::new(None)),
            pending: Arc::new(Mutex::new(None)),
            next_auto_sync_due: Arc::new(Mutex::new(None)),
            local_change_seq: Arc::new(AtomicU64::new(0)),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&self) -> Result<(), AppError> {
        self.refresh_dirty_from_settings()?;
        self.schedule_if_needed()?;
        self.schedule_startup_sync_if_needed()?;
        Ok(())
    }

    pub fn update_settings(&self) -> Result<(), AppError> {
        self.refresh_dirty_from_settings()?;
        self.schedule_if_needed()?;
        self.schedule_startup_sync_if_needed()?;
        Ok(())
    }

    pub fn request_sync(&self, reason: &str) -> Result<(), AppError> {
        let settings = self.db.load_settings()?;
        if !settings.webdav_enabled || settings.webdav_url.trim().is_empty() {
            return Ok(());
        }
        if self
            .sync_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }

        let service = self.clone();
        let reason = reason.to_string();
        let sync_start_seq = service.local_change_seq.load(Ordering::SeqCst);
        // perform_sync 用 reqwest::blocking 并做 Argon2 派生，必须放在阻塞线程池：在异步任务里
        // 会占住运行时的工作线程，debug 构建中 reqwest 还会直接 panic（同步从未执行）。
        tauri::async_runtime::spawn_blocking(move || {
            let outcome = service.perform_sync(&reason);
            if matches!(outcome, Ok(SyncOutcome::Success)) {
                let current_seq = service.local_change_seq.load(Ordering::SeqCst);
                if current_seq == sync_start_seq {
                    service.dirty.store(false, Ordering::SeqCst);
                } else {
                    // 同步过程中出现新的本地变更，继续安排下一次自动同步。
                    let _ = service.schedule_debounced_sync();
                }
            }
            service.sync_in_progress.store(false, Ordering::SeqCst);
        });
        Ok(())
    }

    pub fn notify_local_change(&self) -> Result<(), AppError> {
        self.db.mark_local_change()?;
        self.local_change_seq.fetch_add(1, Ordering::SeqCst);
        self.dirty.store(true, Ordering::SeqCst);
        let _ = self.app.emit("data-updated", ());
        self.schedule_debounced_sync()?;
        Ok(())
    }

    pub fn get_status(&self) -> Result<SyncStatus, AppError> {
        let settings = self.db.load_settings()?;
        Ok(SyncStatus {
            status: status_code(settings.webdav_last_sync_status.as_deref()),
            error: settings.webdav_last_sync_error,
            time: settings.webdav_last_sync_time,
        })
    }

    /// 更换同步密码：与普通同步互斥（同一个进行中标记与远端锁），用已保存的连接设置执行
    /// `change_passphrase_on_remote`。阻塞调用（网络与 Argon2），命令中放到后台线程执行。
    pub fn change_passphrase(&self, current: &str, new: &str) -> Result<(), AppError> {
        let settings = self.db.load_settings()?;
        if !settings.webdav_enabled || settings.webdav_url.trim().is_empty() {
            return Err(AppError::Invalid(
                "请先启用并保存 WebDAV 同步设置".to_string(),
            ));
        }
        validate_passphrase_change(&settings, current, new)?;
        if self
            .sync_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(AppError::Invalid("正在同步，请稍后再试".to_string()));
        }

        let _ = self.update_sync_status(SyncState::Syncing, None);
        let mut lock_busy = false;
        let result = (|| -> Result<(), AppError> {
            let client = WebDavClient::new(&settings)?;
            let lock = LockInfo::new(&settings.webdav_device_id);
            if !client.try_acquire_lock(&lock)? {
                lock_busy = true;
                return Err(AppError::Sync("其他设备正在同步，请稍后再试".to_string()));
            }
            let result =
                change_passphrase_on_remote(&client, &self.db, &settings, new, KdfParams::DEFAULT);
            client.release_lock();
            result.map(|_| ())
        })();

        match &result {
            Ok(()) => {
                let _ = self.update_sync_status(SyncState::Success, None);
                let _ = self.refresh_dirty_from_settings();
                let _ = self.app.emit("data-updated", ());
            }
            Err(_) if lock_busy => {
                let _ = self.update_sync_status(SyncState::LockBusy, None);
            }
            Err(err) => {
                let _ = self.update_sync_status(failure_state(err), Some(err.to_string()));
            }
        }
        self.sync_in_progress.store(false, Ordering::SeqCst);
        result
    }

    fn schedule_if_needed(&self) -> Result<(), AppError> {
        if let Some(handle) = self.scheduled.lock().unwrap().take() {
            handle.abort();
        }
        let settings = self.db.load_settings()?;
        if !settings.webdav_enabled || settings.webdav_url.trim().is_empty() {
            return Ok(());
        }
        let interval = settings.webdav_sync_interval_minutes.max(1) as u64;
        let service = self.clone();
        let handle = tauri::async_runtime::spawn(async move {
            loop {
                sleep(std::time::Duration::from_secs(interval * 60)).await;
                let _ = service.request_sync_on_interval();
            }
        });
        *self.scheduled.lock().unwrap() = Some(handle);
        Ok(())
    }

    fn schedule_startup_sync_if_needed(&self) -> Result<(), AppError> {
        if !self.dirty.load(Ordering::SeqCst) {
            return Ok(());
        }
        let settings = self.db.load_settings()?;
        if !settings.webdav_enabled || settings.webdav_url.trim().is_empty() {
            return Ok(());
        }
        let now = time::now();
        let mut due = now + chrono::Duration::seconds(STARTUP_SYNC_DELAY_SECONDS as i64);
        if let Some(throttle_due) = next_allowed_auto_sync_time(&settings, now) {
            if throttle_due > due {
                due = throttle_due;
            }
        }
        self.schedule_auto_sync_at(due, "startup")?;
        Ok(())
    }

    fn schedule_debounced_sync(&self) -> Result<(), AppError> {
        let settings = self.db.load_settings()?;
        if !settings.webdav_enabled || settings.webdav_url.trim().is_empty() {
            return Ok(());
        }
        let now = time::now();
        let mut due = now + chrono::Duration::seconds(LOCAL_CHANGE_DEBOUNCE_SECONDS as i64);
        if let Some(throttle_due) = next_allowed_auto_sync_time(&settings, now) {
            if throttle_due > due {
                due = throttle_due;
            }
        }
        self.schedule_auto_sync_at(due, "debounce")?;
        Ok(())
    }

    fn schedule_auto_sync_at(&self, due: NaiveDateTime, reason: &str) -> Result<(), AppError> {
        let now = time::now();
        if due <= now {
            return self.request_sync_if_needed(reason);
        }

        {
            let mut next_due = self.next_auto_sync_due.lock().unwrap();
            if let Some(existing) = *next_due {
                // 已有更“晚”的同步计划（通常是新的本地变更触发），避免被更早的计划覆盖。
                if existing >= due {
                    return Ok(());
                }
            }
            *next_due = Some(due);
        }

        if let Some(handle) = self.pending.lock().unwrap().take() {
            handle.abort();
        }

        let delay = (due - now)
            .to_std()
            .unwrap_or_else(|_| std::time::Duration::from_secs(0));
        let service = self.clone();
        let reason = reason.to_string();
        let handle = tauri::async_runtime::spawn(async move {
            sleep(delay).await;
            *service.next_auto_sync_due.lock().unwrap() = None;
            let _ = service.request_sync_if_needed(&reason);
        });
        *self.pending.lock().unwrap() = Some(handle);
        Ok(())
    }

    fn request_sync_on_interval(&self) -> Result<(), AppError> {
        let now = time::now();
        if let Some(due) = *self.next_auto_sync_due.lock().unwrap() {
            let remaining = due - now;
            // 如果 debounce 很快就会触发，就避免 interval “抢跑”；否则 interval 作为兜底依然可以触发同步。
            if remaining.num_seconds() > 0 && remaining.num_seconds() <= 30 {
                return Ok(());
            }
        }
        self.request_sync_if_needed("interval")
    }

    fn request_sync_if_needed(&self, reason: &str) -> Result<(), AppError> {
        if !self.dirty.load(Ordering::SeqCst) {
            return Ok(());
        }
        let settings = self.db.load_settings()?;
        if !settings.webdav_enabled || settings.webdav_url.trim().is_empty() {
            return Ok(());
        }
        // 手动同步（`request_sync`）不受影响；修改同步密码后由 `save_settings` 立即触发一次。
        if auto_sync_paused(&settings) {
            return Ok(());
        }

        // throttle：距离上一次同步太近则延后。
        let now = time::now();
        if let Some(throttle_due) = next_allowed_auto_sync_time(&settings, now) {
            self.schedule_auto_sync_at(throttle_due, "throttle")?;
            return Ok(());
        }

        self.request_sync(reason)
    }

    fn refresh_dirty_from_settings(&self) -> Result<(), AppError> {
        let settings = self.db.load_settings()?;
        let dirty = compute_dirty_from_settings(&settings);
        self.dirty.store(dirty, Ordering::SeqCst);
        Ok(())
    }

    fn perform_sync(&self, _reason: &str) -> Result<SyncOutcome, AppError> {
        let settings = self.db.load_settings()?;
        let client = WebDavClient::new(&settings)?;
        let lock = LockInfo::new(&settings.webdav_device_id);

        let _ = self.update_sync_status(SyncState::Syncing, None);
        let mut lock_acquired = false;

        let result = (|| -> Result<SyncOutcome, AppError> {
            lock_acquired = client.try_acquire_lock(&lock)?;
            if !lock_acquired {
                let _ = self.update_sync_status(SyncState::LockBusy, None);
                return Ok(SyncOutcome::Skipped);
            }

            let result = sync_with_remote(&client, &self.db, &settings, KdfParams::DEFAULT)?;
            match result {
                RemoteSyncResult::FirstUpload => {
                    let _ = self.update_sync_status(SyncState::FirstSync, None);
                }
                RemoteSyncResult::Merged => {
                    let _ = self.update_sync_status(SyncState::Success, None);
                    let _ = self.app.emit("data-updated", ());
                }
            }
            Ok(SyncOutcome::Success)
        })();

        let outcome = match result {
            Ok(outcome) => outcome,
            Err(err) => {
                let _ = self.update_sync_status(failure_state(&err), Some(err.to_string()));
                SyncOutcome::Failed
            }
        };

        if lock_acquired {
            client.release_lock();
        }
        Ok(outcome)
    }

    fn update_sync_status(&self, state: SyncState, error: Option<String>) -> Result<(), AppError> {
        let settings = self.db.update_sync_status(state.code(), error)?;
        let payload = SyncStatus {
            status: status_code(settings.webdav_last_sync_status.as_deref()),
            error: settings.webdav_last_sync_error.clone(),
            time: settings.webdav_last_sync_time.clone(),
        };
        let _ = self.app.emit("sync-status", payload);
        Ok(())
    }
}
