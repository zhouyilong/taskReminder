//! WebDAV 云同步：`service`（调度与入口）、`remote`（同步主流程与加密）、`merge`（快照导出与按行合并）、`webdav`（客户端与同步锁）。

use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

use base64::Engine;
use chrono::NaiveDateTime;
use reqwest::StatusCode;
use rusqlite::{params_from_iter, types::Value, Connection};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::time::sleep;

use crate::db::{DbManager, TOMBSTONE_RETENTION_DAYS_SYNC};
use crate::errors::AppError;
use crate::models::{AppSettings, SyncStatus};
use crate::sync_crypto::{self, KdfParams};
use crate::sync_schema::{
    adopt_remote_columns, ensure_sync_columns, quote_ident, table_columns, SyncTable, SYNC_TABLES,
};
use crate::time::{self, parse_datetime_any};

const REMOTE_DB_NAME: &str = "taskreminder.db";
/// 开启端到端加密后的远端文件；`REMOTE_DB_NAME` 处改放占位说明，旧版本读到后同步失败而不会上传明文。
const REMOTE_ENC_NAME: &str = "taskreminder.db.enc";
const PLACEHOLDER_MARKER: &[u8] = b"TaskReminder-Encrypted-Placeholder";
const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";
const LOCK_FILE_NAME: &str = "taskreminder.lock";
const LOCK_TTL_SECONDS: i64 = 120;

// 自动同步策略：本地变更后延迟同步（debounce）+ 最小同步间隔（throttle）。
const LOCAL_CHANGE_DEBOUNCE_SECONDS: u64 = 5 * 60;
const MIN_AUTOMATIC_SYNC_INTERVAL_SECONDS: i64 = 15 * 60;
const STARTUP_SYNC_DELAY_SECONDS: u64 = 15;

mod merge;
mod remote;
mod service;
mod webdav;

pub(crate) use merge::merge_databases;
use merge::*;
pub use remote::test_webdav;
use remote::*;
pub use service::CloudSyncService;
use webdav::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SyncOutcome {
    Success,
    Skipped,
    Failed,
}

/// 同步状态码：存库与发给前端的都是状态码，文案只在前端（`src/syncStatus.ts`）映射。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    Never,
    Syncing,
    Success,
    FirstSync,
    LockBusy,
    Failed,
    /// 远端加密数据无法用本机的同步密码解密，通常是在其他设备上更换了同步密码。
    /// 此状态下暂停自动同步，直到修改同步密码或手动同步。
    PassphraseMismatch,
}

impl SyncState {
    pub fn code(self) -> &'static str {
        match self {
            SyncState::Never => "never",
            SyncState::Syncing => "syncing",
            SyncState::Success => "success",
            SyncState::FirstSync => "first_sync",
            SyncState::LockBusy => "lock_busy",
            SyncState::Failed => "failed",
            SyncState::PassphraseMismatch => "passphrase_mismatch",
        }
    }

    /// 解析存库的状态；兼容 2.0.1 之前直接存中文文案的旧值。
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "" | "never" | "未同步" => Some(SyncState::Never),
            "syncing" | "同步中" => Some(SyncState::Syncing),
            "success" | "同步成功" => Some(SyncState::Success),
            "first_sync" | "首次同步完成" => Some(SyncState::FirstSync),
            "lock_busy" | "锁被占用，稍后重试" => Some(SyncState::LockBusy),
            "failed" | "同步失败" => Some(SyncState::Failed),
            "passphrase_mismatch" => Some(SyncState::PassphraseMismatch),
            _ => None,
        }
    }

    fn is_success(self) -> bool {
        matches!(self, SyncState::Success | SyncState::FirstSync)
    }
}

/// 把存库的状态转成状态码；无法识别的值原样返回，前端按原文显示。
pub(super) fn status_code(stored: Option<&str>) -> String {
    let raw = stored.unwrap_or("");
    SyncState::parse(raw)
        .map(|state| state.code().to_string())
        .unwrap_or_else(|| raw.to_string())
}

pub(super) fn failure_state(err: &AppError) -> SyncState {
    match err {
        AppError::SyncPassphrase(_) => SyncState::PassphraseMismatch,
        _ => SyncState::Failed,
    }
}

/// 同步密码不匹配时暂停自动同步：每次都会失败，还要白白做一次 Argon2 派生。
pub(super) fn auto_sync_paused(settings: &AppSettings) -> bool {
    settings
        .webdav_last_sync_status
        .as_deref()
        .and_then(SyncState::parse)
        == Some(SyncState::PassphraseMismatch)
}

/// 保存设置时，同步密码不匹配状态下改了同步密码：需要立即同步一次以确认并恢复自动同步。
pub fn should_resync_after_passphrase_edit(previous: &AppSettings, saved: &AppSettings) -> bool {
    auto_sync_paused(previous)
        && saved.webdav_enabled
        && saved.sync_encryption_enabled
        && saved.sync_passphrase != previous.sync_passphrase
}

pub(super) fn is_success_status(status: &Option<String>) -> bool {
    status
        .as_deref()
        .and_then(SyncState::parse)
        .is_some_and(SyncState::is_success)
}

pub(super) fn compute_dirty_from_settings(settings: &AppSettings) -> bool {
    let Some(local_change) = settings.webdav_last_local_change_time.as_deref() else {
        return false;
    };

    // 没有成功同步过，或者上一次处于失败/进行中状态：视为还有未同步变更。
    if !is_success_status(&settings.webdav_last_sync_status) {
        return true;
    }

    let Some(last_sync) = settings.webdav_last_sync_time.as_deref() else {
        return true;
    };

    match (
        parse_datetime_any(local_change),
        parse_datetime_any(last_sync),
    ) {
        (Some(local_dt), Some(sync_dt)) => local_dt > sync_dt,
        _ => true,
    }
}

pub(super) fn next_allowed_auto_sync_time(
    settings: &AppSettings,
    now: NaiveDateTime,
) -> Option<NaiveDateTime> {
    let last_sync = settings
        .webdav_last_sync_time
        .as_deref()
        .and_then(parse_datetime_any)?;
    let next_allowed = last_sync + chrono::Duration::seconds(MIN_AUTOMATIC_SYNC_INTERVAL_SECONDS);
    if next_allowed > now {
        Some(next_allowed)
    } else {
        None
    }
}

#[cfg(test)]
mod compat_tests;
#[cfg(test)]
mod tests;
