//! 客户端云端同步引擎（方案 B：统一本地 SQLite + 账号镜像库 + 增量推拉 + SSE 实时通知）。

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::{Method, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use task_reminder_core::kinds::AppMode;
use task_reminder_core::protocol::{
    AdminCreateUserRequest, AdminResetPasswordRequest, AdminUpdateUserRequest, AdminUserSummary,
    ApiErrorBody, AuthTokenResponse, ChangePasswordRequest, HealthResponse, LoginRequest,
    LogoutRequest, RefreshRequest, ServerTimeResponse, SyncMergeRequest, SyncMergeResponse,
    SyncMergeSummary, SyncPullResponse, SyncPushRequest, SyncPushResponse, UpdateProfileRequest,
    UserInfo,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use crate::backup;
use crate::db::DbManager;
use crate::errors::AppError;
use crate::http;
use crate::models::{BusinessDataCounts, CloudStatus};
use crate::state::AppState;
use crate::time;
use crate::tray;
use crate::windows::sticky;

/// 默认官方云端服务地址（可在编译期经 `TASK_REMINDER_DEFAULT_SERVER_URL` 覆盖，亦可在设置中自定义）。
pub const DEFAULT_SERVER_URL: &str = match option_env!("TASK_REMINDER_DEFAULT_SERVER_URL") {
    Some(url) if !url.is_empty() => url,
    _ => "http://104.168.101.151:8080",
};

/// 写穿推送防抖时长（毫秒），合并高频连续编辑。
const PUSH_DEBOUNCE_MS: u64 = 350;
/// 单次 Pull 最大拉取条数。
const PULL_BATCH_LIMIT: i64 = 500;

/// 规范化服务端地址：空值回退至内置官方地址，校验 `http://` / `https://` 前缀并去掉末尾 `/`。
pub fn resolve_server_url(configured: &str) -> Result<String, AppError> {
    let raw = configured.trim();
    let target = if raw.is_empty() {
        DEFAULT_SERVER_URL
    } else {
        raw
    };
    let trimmed = target.trim_end_matches('/');
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err(AppError::Invalid(
            "服务端地址必须以 http:// 或 https:// 开头".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

#[derive(Clone, Debug, Default)]
struct AccessTokenCache {
    token: String,
    expires_at: Option<Instant>,
}

impl AccessTokenCache {
    fn valid_token(&self) -> Option<String> {
        if self.token.is_empty() {
            return None;
        }
        let exp = self.expires_at?;
        // 提前 30 秒判定过期，主动刷新。
        if exp > Instant::now() + Duration::from_secs(30) {
            Some(self.token.clone())
        } else {
            None
        }
    }

    fn set(&mut self, token: String, expires_in_seconds: i64) {
        self.token = token;
        let ttl = expires_in_seconds.max(60) as u64;
        self.expires_at = Some(Instant::now() + Duration::from_secs(ttl));
    }

    fn clear(&mut self) {
        self.token.clear();
        self.expires_at = None;
    }
}

#[derive(Clone, Debug)]
struct CloudRuntime {
    state: String,
    error: Option<String>,
    last_sync_time: Option<String>,
}

impl Default for CloudRuntime {
    fn default() -> Self {
        Self {
            state: "local".to_string(),
            error: None,
            last_sync_time: None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchToCloudPreview {
    pub logged_in: bool,
    pub user_id: String,
    pub username: String,
    pub nickname: String,
    pub local_counts: BusinessDataCounts,
    pub last_merged_account_id: String,
    pub last_merged_username: String,
    pub cross_account_warning: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchToCloudResult {
    pub status: CloudStatus,
    pub merge_summary: Option<SyncMergeSummary>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchToLocalPreview {
    pub is_online: bool,
    pub pending_count: usize,
    pub cloud_counts: BusinessDataCounts,
    pub local_counts: BusinessDataCounts,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchToLocalResult {
    pub status: CloudStatus,
    pub merged: bool,
}

#[derive(Clone)]
pub struct CloudSyncService {
    app: AppHandle,
    db: DbManager,
    client: reqwest::Client,
    sse_client: reqwest::Client,
    access_token: Arc<Mutex<AccessTokenCache>>,
    runtime: Arc<Mutex<CloudRuntime>>,
    push_notify: Arc<Notify>,
    sse_generation: Arc<AtomicU64>,
    sync_lock: Arc<tokio::sync::Mutex<()>>,
}

impl CloudSyncService {
    pub fn new(app: AppHandle, db: DbManager) -> Self {
        let client = http::async_client_builder()
            .timeout(Duration::from_secs(20))
            .build()
            .expect("构建 HTTP 客户端失败");
        let sse_client = http::async_client_builder()
            .connect_timeout(Duration::from_secs(15))
            .build()
            .expect("构建 SSE HTTP 客户端失败");
        Self {
            app,
            db,
            client,
            sse_client,
            access_token: Arc::new(Mutex::new(AccessTokenCache::default())),
            runtime: Arc::new(Mutex::new(CloudRuntime::default())),
            push_notify: Arc::new(Notify::new()),
            sse_generation: Arc::new(AtomicU64::new(0)),
            sync_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// 应用启动时按持久化运行模式初始化活动库并拉起后台推送与 SSE 监听。
    pub fn start(&self) -> Result<(), AppError> {
        let settings = self.db.load_settings()?;
        self.spawn_push_worker();

        if settings.app_mode == AppMode::Cloud {
            if !settings.cloud_user_id.trim().is_empty() {
                self.db.switch_to_account(&settings.cloud_user_id)?;
                if let Ok(sync_state) = self.db.get_cloud_sync_state() {
                    let mut rt = self.runtime.lock().unwrap_or_else(|e| e.into_inner());
                    rt.last_sync_time = sync_state.last_sync_time;
                }
                let has_token = !self.db.load_refresh_token().unwrap_or_default().is_empty();
                if has_token {
                    self.set_runtime_state("syncing", None);
                    self.start_sse_loop();
                    let svc = self.clone();
                    tauri::async_runtime::spawn(async move {
                        svc.calibrate_clock().await;
                        if let Err(err) = svc.sync_now_async().await {
                            eprintln!("[cloud] 启动同步失败: {}", err);
                        }
                    });
                } else {
                    self.set_runtime_state("auth_required", Some("请登录云端账号".to_string()));
                }
            } else {
                self.set_runtime_state("auth_required", Some("请登录云端账号".to_string()));
            }
        } else {
            self.db.switch_to_local();
            self.set_runtime_state("local", None);
        }
        self.emit_status();
        Ok(())
    }

    /// 本地数据写入后通知云端引擎（本地模式为空操作；云端模式触发防抖增量 Push）。
    pub fn notify_local_change(&self) -> Result<(), AppError> {
        if !self.db.is_cloud_active() {
            return Ok(());
        }
        self.push_notify.notify_one();
        self.emit_status();
        Ok(())
    }

    /// 手动或托盘触发立即同步。
    pub fn request_sync(&self, _reason: &str) -> Result<(), AppError> {
        if !self.db.is_cloud_active() {
            return Ok(());
        }
        let svc = self.clone();
        tauri::async_runtime::spawn(async move {
            let _ = svc.sync_now_async().await;
        });
        Ok(())
    }

    pub fn get_status(&self) -> Result<CloudStatus, AppError> {
        let settings = self.db.load_settings()?;
        let rt = self
            .runtime
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let pending_count = self.db.pending_mutation_count().unwrap_or(0);
        let effective_url = resolve_server_url(&settings.cloud_server_url)
            .unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string());
        let logged_in = !settings.cloud_user_id.trim().is_empty()
            && !self.db.load_refresh_token().unwrap_or_default().is_empty();
        let state = if settings.app_mode == AppMode::Local {
            "local".to_string()
        } else if !logged_in {
            "auth_required".to_string()
        } else {
            rt.state
        };

        Ok(CloudStatus {
            mode: settings.app_mode,
            state,
            logged_in,
            user_id: settings.cloud_user_id,
            username: settings.cloud_username,
            nickname: settings.cloud_nickname,
            role: settings.cloud_user_role,
            must_change_password: settings.cloud_must_change_password,
            server_url: effective_url,
            default_server_url: DEFAULT_SERVER_URL.to_string(),
            pending_count,
            last_sync_time: rt.last_sync_time,
            error: rt.error,
        })
    }

    pub fn emit_status(&self) {
        if let Ok(status) = self.get_status() {
            let _ = self.app.emit("cloud-status", status);
        }
    }

    fn set_runtime_state(&self, state: &str, error: Option<String>) {
        let mut rt = self.runtime.lock().unwrap_or_else(|e| e.into_inner());
        rt.state = state.to_string();
        rt.error = error;
        if state == "connected" {
            rt.last_sync_time = Some(time::now_string());
        }
    }

    fn server_base_url(&self) -> Result<String, AppError> {
        let settings = self.db.load_settings()?;
        resolve_server_url(&settings.cloud_server_url)
    }

    fn device_name() -> String {
        let os = if cfg!(target_os = "windows") {
            "Windows"
        } else if cfg!(target_os = "macos") {
            "macOS"
        } else {
            "Linux"
        };
        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_default();
        if host.trim().is_empty() {
            format!("TaskReminder Desktop ({})", os)
        } else {
            format!("{} ({})", host.trim(), os)
        }
    }

    /// 测试指定服务地址连通性。
    pub async fn test_connection(&self, url: &str) -> Result<HealthResponse, AppError> {
        let base = resolve_server_url(url)?;
        let resp = self
            .client
            .get(format!("{}/api/v1/health", base))
            .send()
            .await
            .map_err(|e| AppError::Sync(format!("无法连接到服务端: {}", e)))?;
        parse_response(resp).await
    }

    /// 修改服务端地址；若目标环境变化则自动登出当前会话。
    pub async fn set_server_url(&self, new_url: &str) -> Result<CloudStatus, AppError> {
        let settings = self.db.load_settings()?;
        let old_effective = resolve_server_url(&settings.cloud_server_url)
            .unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string());
        let trimmed = new_url.trim();
        let next_effective = resolve_server_url(trimmed)?;

        if old_effective != next_effective && !settings.cloud_user_id.trim().is_empty() {
            let _ = self.logout().await;
        }
        self.db.set_cloud_server_url(trimmed)?;
        self.emit_status();
        self.get_status()
    }

    /// 账号登录（可选同时更新服务端地址）。
    pub async fn login(
        &self,
        server_url: Option<String>,
        username: &str,
        password: &str,
    ) -> Result<CloudStatus, AppError> {
        if let Some(url) = server_url {
            let trimmed = url.trim();
            let _ = resolve_server_url(trimmed)?;
            let current = self.db.load_settings()?;
            if current.cloud_server_url.trim() != trimmed {
                self.db.set_cloud_server_url(trimmed)?;
            }
        }
        let settings = self.db.load_settings()?;
        let base = resolve_server_url(&settings.cloud_server_url)?;
        let req = LoginRequest {
            username: username.trim().to_string(),
            password: password.to_string(),
            device_id: settings.device_id.clone(),
            device_name: Self::device_name(),
        };
        let resp = self
            .client
            .post(format!("{}/api/v1/auth/login", base))
            .json(&req)
            .send()
            .await
            .map_err(|e| AppError::Sync(format!("连接服务端失败: {}", e)))?;
        let auth: AuthTokenResponse = parse_response(resp).await?;

        self.apply_auth_response(&auth)?;
        self.calibrate_clock().await;

        // 若已在云端模式，立即切换到该账号镜像库并拉取最新数据与重连 SSE。
        if self.db.load_settings()?.app_mode == AppMode::Cloud {
            self.db.switch_to_account(&auth.user.id)?;
            self.set_runtime_state("syncing", None);
            self.emit_status();
            self.start_sse_loop();
            let _ = self.sync_now_async().await;
            self.refresh_runtime_views();
        } else {
            self.emit_status();
        }

        self.get_status()
    }

    /// 退出登录：吊销服务端会话、清理本地 Token 与该账号缓存镜像库。
    pub async fn logout(&self) -> Result<CloudStatus, AppError> {
        let settings = self.db.load_settings()?;
        let user_id = settings.cloud_user_id.clone();
        let refresh_token = self.db.load_refresh_token().unwrap_or_default();

        if let Ok(base) = resolve_server_url(&settings.cloud_server_url) {
            if let Ok(access) = self.ensure_access_token().await {
                let _ = self
                    .client
                    .post(format!("{}/api/v1/auth/logout", base))
                    .bearer_auth(access)
                    .json(&LogoutRequest {
                        refresh_token: if refresh_token.is_empty() {
                            None
                        } else {
                            Some(refresh_token)
                        },
                    })
                    .send()
                    .await;
            }
        }

        self.sse_generation.fetch_add(1, Ordering::SeqCst);
        self.access_token
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.db.save_refresh_token("")?;
        self.db.clear_cloud_account()?;
        if !user_id.trim().is_empty() {
            self.db.remove_account_cache(&user_id);
        }

        let was_cloud = settings.app_mode == AppMode::Cloud;
        if was_cloud {
            self.db.switch_to_local();
            self.db.set_app_mode(AppMode::Local)?;
            self.set_runtime_state("local", None);
            self.refresh_runtime_views();
        } else {
            self.set_runtime_state("local", None);
        }
        self.emit_status();
        self.get_status()
    }

    pub async fn update_profile(&self, nickname: &str) -> Result<UserInfo, AppError> {
        let user: UserInfo = self
            .authed_json(
                Method::PUT,
                "/api/v1/auth/profile",
                Some(&UpdateProfileRequest {
                    nickname: nickname.trim().to_string(),
                }),
            )
            .await?;
        self.db.save_cloud_account(
            &user.id,
            &user.username,
            &user.nickname,
            user.role.as_str(),
            user.must_change_password,
        )?;
        self.emit_status();
        Ok(user)
    }

    pub async fn change_password(
        &self,
        current_password: &str,
        new_password: &str,
    ) -> Result<UserInfo, AppError> {
        let user: UserInfo = self
            .authed_json(
                Method::PUT,
                "/api/v1/auth/password",
                Some(&ChangePasswordRequest {
                    current_password: current_password.to_string(),
                    new_password: new_password.to_string(),
                }),
            )
            .await?;
        self.db.save_cloud_account(
            &user.id,
            &user.username,
            &user.nickname,
            user.role.as_str(),
            user.must_change_password,
        )?;
        self.emit_status();
        Ok(user)
    }

    // ---- 管理员账号管理代理 ----

    pub async fn admin_list_users(&self) -> Result<Vec<AdminUserSummary>, AppError> {
        self.authed_json::<(), Vec<AdminUserSummary>>(Method::GET, "/api/v1/admin/users", None)
            .await
    }

    pub async fn admin_create_user(
        &self,
        req: &AdminCreateUserRequest,
    ) -> Result<AdminUserSummary, AppError> {
        self.authed_json(Method::POST, "/api/v1/admin/users", Some(req))
            .await
    }

    pub async fn admin_update_user(
        &self,
        user_id: &str,
        req: &AdminUpdateUserRequest,
    ) -> Result<AdminUserSummary, AppError> {
        self.authed_json(
            Method::PATCH,
            &format!("/api/v1/admin/users/{}", user_id),
            Some(req),
        )
        .await
    }

    pub async fn admin_reset_password(
        &self,
        user_id: &str,
        req: &AdminResetPasswordRequest,
    ) -> Result<AdminUserSummary, AppError> {
        self.authed_json(
            Method::POST,
            &format!("/api/v1/admin/users/{}/reset-password", user_id),
            Some(req),
        )
        .await
    }

    pub async fn admin_delete_user(&self, user_id: &str) -> Result<(), AppError> {
        self.authed_empty(Method::DELETE, &format!("/api/v1/admin/users/{}", user_id))
            .await
    }

    // ---- 模式切换与双向合并 ----

    pub async fn preview_switch_to_cloud(&self) -> Result<SwitchToCloudPreview, AppError> {
        let settings = self.db.load_settings()?;
        let local_counts = DbManager::count_business_data_in(&self.db.local_db_path())?;
        let logged_in = !settings.cloud_user_id.trim().is_empty()
            && !self.db.load_refresh_token().unwrap_or_default().is_empty();
        let cross_account_warning = logged_in
            && !settings.last_merged_account_id.trim().is_empty()
            && settings.last_merged_account_id != settings.cloud_user_id;

        Ok(SwitchToCloudPreview {
            logged_in,
            user_id: settings.cloud_user_id,
            username: settings.cloud_username,
            nickname: settings.cloud_nickname,
            local_counts,
            last_merged_account_id: settings.last_merged_account_id,
            last_merged_username: settings.last_merged_username,
            cross_account_warning,
        })
    }

    pub async fn switch_to_cloud(
        &self,
        merge_local: bool,
    ) -> Result<SwitchToCloudResult, AppError> {
        let _guard = self.sync_lock.lock().await;
        let settings = self.db.load_settings()?;
        if settings.cloud_user_id.trim().is_empty() {
            return Err(AppError::Auth("请先登录云端账号".to_string()));
        }
        // 确保网络连通且登录态有效
        let _ = self.ensure_access_token().await?;

        // 切换前先对本地主库做一次自动快照备份
        let local_path = self.db.local_db_path();
        let _ = backup::create_backup(&local_path, &time::now(), backup::keep_count(&self.db));

        // 切换到账号镜像库
        self.db.switch_to_account(&settings.cloud_user_id)?;
        self.set_runtime_state("syncing", None);
        self.emit_status();

        let merge_summary = if merge_local {
            // 原 tasks.db 保持不动，导出其完整业务数据提交合并
            let local_dataset = DbManager::export_all_sync_rows_from(&local_path)?;
            let resp: SyncMergeResponse = self
                .authed_json(
                    Method::POST,
                    "/api/v1/sync/merge",
                    Some(&SyncMergeRequest {
                        device_id: settings.device_id.clone(),
                        changes: local_dataset.clone(),
                    }),
                )
                .await?;
            // 先把本地行与远端胜出/独有行合入当前账号镜像库
            self.db.apply_remote_changes(&local_dataset)?;
            self.db.apply_remote_changes(&resp.remote_changes)?;
            self.db.set_last_server_seq(resp.server_seq)?;
            self.db
                .record_last_merged_account(&settings.cloud_user_id, &settings.cloud_username)?;
            Some(resp.summary)
        } else {
            self.pull_incremental_locked().await?;
            None
        };

        self.db.set_app_mode(AppMode::Cloud)?;
        self.db.set_cloud_sync_status("connected", None)?;
        self.set_runtime_state("connected", None);
        self.calibrate_clock().await;
        self.start_sse_loop();
        self.refresh_runtime_views();
        self.emit_status();

        Ok(SwitchToCloudResult {
            status: self.get_status()?,
            merge_summary,
        })
    }

    pub async fn preview_switch_to_local(&self) -> Result<SwitchToLocalPreview, AppError> {
        let pending_count = self.db.pending_mutation_count().unwrap_or(0);
        let local_counts = DbManager::count_business_data_in(&self.db.local_db_path())?;
        let cloud_counts = if self.db.is_cloud_active() {
            DbManager::count_business_data_in(&self.db.db_path())?
        } else {
            BusinessDataCounts::default()
        };
        let is_online = match self.server_base_url() {
            Ok(base) => self.test_connection(&base).await.is_ok(),
            Err(_) => false,
        };
        Ok(SwitchToLocalPreview {
            is_online,
            pending_count,
            cloud_counts,
            local_counts,
        })
    }

    pub async fn switch_to_local(
        &self,
        merge_cloud: bool,
    ) -> Result<SwitchToLocalResult, AppError> {
        let _guard = self.sync_lock.lock().await;
        let settings = self.db.load_settings()?;

        // 网络正常时先尽力把待推送变更推上云并拉取一次最新增量
        if self.db.is_cloud_active() {
            let _ = self.push_pending_locked().await;
            let _ = self.pull_incremental_locked().await;
        }

        // 停止 SSE 订阅
        self.sse_generation.fetch_add(1, Ordering::SeqCst);

        let local_path = self.db.local_db_path();
        let account_path = self.db.db_path();
        let mut merged = false;

        if merge_cloud && self.db.is_cloud_active() && account_path != local_path {
            let _ = backup::create_backup(&local_path, &time::now(), backup::keep_count(&self.db));
            crate::sync::merge_databases(&local_path, &account_path)?;
            if !settings.cloud_user_id.trim().is_empty() {
                self.db.record_last_merged_account(
                    &settings.cloud_user_id,
                    &settings.cloud_username,
                )?;
            }
            merged = true;
        }

        // 切回本地库，默认保留云端登录态以便后续一键切回
        self.db.switch_to_local();
        self.db.set_app_mode(AppMode::Local)?;
        self.set_runtime_state("local", None);
        self.refresh_runtime_views();
        self.emit_status();

        Ok(SwitchToLocalResult {
            status: self.get_status()?,
            merged,
        })
    }

    // ---- 增量 Push / Pull / SSE 核心循环 ----

    fn spawn_push_worker(&self) {
        let svc = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                svc.push_notify.notified().await;
                // 防抖 350ms，合并短时间内的连续编辑
                tokio::time::sleep(Duration::from_millis(PUSH_DEBOUNCE_MS)).await;
                if !svc.db.is_cloud_active() {
                    continue;
                }
                if let Err(err) = svc.sync_now_async().await {
                    eprintln!("[cloud] 写穿推送失败: {}", err);
                }
            }
        });
    }

    fn start_sse_loop(&self) {
        let generation = self.sse_generation.fetch_add(1, Ordering::SeqCst) + 1;
        let svc = self.clone();
        tauri::async_runtime::spawn(async move {
            let mut backoff_secs = 2u64;
            loop {
                if svc.sse_generation.load(Ordering::SeqCst) != generation
                    || !svc.db.is_cloud_active()
                {
                    break;
                }
                match svc.run_sse_once(generation).await {
                    Ok(()) => {
                        backoff_secs = 2;
                    }
                    Err(AppError::Auth(msg)) => {
                        svc.set_runtime_state("auth_required", Some(msg));
                        svc.emit_status();
                        break;
                    }
                    Err(err) => {
                        svc.set_runtime_state("offline", Some(err.to_string()));
                        svc.emit_status();
                        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                        backoff_secs = (backoff_secs * 2).min(30);
                    }
                }
            }
        });
    }

    async fn run_sse_once(&self, generation: u64) -> Result<(), AppError> {
        let token = self.ensure_access_token().await?;
        let base = self.server_base_url()?;
        let device_id = self.db.load_settings()?.device_id;

        let resp = self
            .sse_client
            .get(format!("{}/api/v1/sync/events", base))
            .bearer_auth(&token)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .send()
            .await
            .map_err(|e| AppError::Sync(format!("SSE 连接失败: {}", e)))?;

        if resp.status() == StatusCode::UNAUTHORIZED || resp.status() == StatusCode::FORBIDDEN {
            // 尝试刷新一次令牌，若刷新也失败则判定认证失效
            let _ = self.refresh_token_now().await?;
            return Ok(());
        }
        if !resp.status().is_success() {
            return Err(AppError::Sync(format!(
                "SSE 订阅失败 (HTTP {})",
                resp.status().as_u16()
            )));
        }

        // 重连成功后立即补推并拉取断网期间遗漏的增量
        if let Err(err) = self.sync_now_async().await {
            eprintln!("[cloud] SSE 重连补同步失败: {}", err);
        }

        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut current_event = String::new();

        while let Some(chunk) = stream.next().await {
            if self.sse_generation.load(Ordering::SeqCst) != generation
                || !self.db.is_cloud_active()
            {
                return Ok(());
            }
            let bytes = chunk.map_err(|e| AppError::Sync(format!("SSE 流中断: {}", e)))?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim_end_matches('\r').to_string();
                buffer.drain(..=pos);
                if line.is_empty() {
                    current_event.clear();
                    continue;
                }
                if let Some(ev) = line.strip_prefix("event:") {
                    current_event = ev.trim().to_string();
                } else if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim();
                    if matches!(current_event.as_str(), "sync" | "connected" | "resync" | "") {
                        let should_pull = if current_event == "resync" {
                            true
                        } else if let Ok(payload) = serde_json::from_str::<
                            task_reminder_core::protocol::SyncEventPayload,
                        >(data)
                        {
                            let local_seq = self
                                .db
                                .get_cloud_sync_state()
                                .map(|s| s.last_server_seq)
                                .unwrap_or(0);
                            payload.server_seq > local_seq && payload.device_id != device_id
                        } else {
                            false
                        };
                        if should_pull {
                            let _ = self.sync_now_async().await;
                        }
                    }
                }
            }
        }
        Err(AppError::Sync("SSE 连接已关闭，正在重连".to_string()))
    }

    pub async fn sync_now_async(&self) -> Result<(), AppError> {
        if !self.db.is_cloud_active() {
            return Ok(());
        }
        let _guard = self.sync_lock.lock().await;
        self.set_runtime_state("syncing", None);
        self.emit_status();

        let result: Result<bool, AppError> = async {
            let pushed_updates = self.push_pending_locked().await?;
            let pulled_updates = self.pull_incremental_locked().await?;
            Ok(pushed_updates || pulled_updates)
        }
        .await;

        match result {
            Ok(changed) => {
                let _ = self.db.set_cloud_sync_status("connected", None);
                self.set_runtime_state("connected", None);
                self.emit_status();
                if changed {
                    self.refresh_runtime_views();
                }
                Ok(())
            }
            Err(AppError::Auth(msg)) => {
                let _ = self.db.set_cloud_sync_status("auth_required", Some(&msg));
                self.set_runtime_state("auth_required", Some(msg.clone()));
                self.emit_status();
                Err(AppError::Auth(msg))
            }
            Err(err) => {
                let msg = err.to_string();
                let _ = self.db.set_cloud_sync_status("offline", Some(&msg));
                self.set_runtime_state("offline", Some(msg));
                self.emit_status();
                Err(err)
            }
        }
    }

    async fn push_pending_locked(&self) -> Result<bool, AppError> {
        let pending = self.db.list_pending_mutations()?;
        if pending.is_empty() {
            return Ok(false);
        }
        let changes = self.db.load_sync_rows_for_keys(&pending)?;
        if changes.is_empty() {
            self.db.clear_pending_mutations(&pending)?;
            return Ok(false);
        }
        let device_id = self.db.load_settings()?.device_id;
        let resp: SyncPushResponse = self
            .authed_json(
                Method::POST,
                "/api/v1/sync/push",
                Some(&SyncPushRequest { device_id, changes }),
            )
            .await?;

        let applied_wins = self.db.apply_remote_changes(&resp.server_wins)?;
        self.db.clear_pending_mutations(&pending)?;
        self.db.set_last_server_seq(resp.server_seq)?;
        Ok(applied_wins > 0)
    }

    async fn pull_incremental_locked(&self) -> Result<bool, AppError> {
        let mut since = self.db.get_cloud_sync_state()?.last_server_seq;
        let mut total_applied = 0usize;
        loop {
            let path = format!(
                "/api/v1/sync/pull?since={}&limit={}",
                since, PULL_BATCH_LIMIT
            );
            let resp: SyncPullResponse =
                self.authed_json::<(), _>(Method::GET, &path, None).await?;
            total_applied += self.db.apply_remote_changes(&resp.changes)?;
            since = resp.server_seq.max(since);
            self.db.set_last_server_seq(since)?;
            if !resp.has_more {
                break;
            }
        }
        Ok(total_applied > 0)
    }

    async fn calibrate_clock(&self) {
        let Ok(base) = self.server_base_url() else {
            return;
        };
        let t0 = time::unix_millis();
        let Ok(resp) = self
            .client
            .get(format!("{}/api/v1/time", base))
            .send()
            .await
        else {
            return;
        };
        let t1 = time::unix_millis();
        if let Ok(parsed) = parse_response::<ServerTimeResponse>(resp).await {
            let local_mid = (t0 + t1) / 2;
            let offset = parsed.unix_millis - local_mid;
            let _ = self.db.set_clock_offset_ms(offset);
        }
    }

    // ---- 认证令牌刷新与重试封装 ----

    fn apply_auth_response(&self, auth: &AuthTokenResponse) -> Result<(), AppError> {
        self.access_token
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .set(auth.access_token.clone(), auth.expires_in_seconds);
        self.db.save_refresh_token(&auth.refresh_token)?;
        self.db.save_cloud_account(
            &auth.user.id,
            &auth.user.username,
            &auth.user.nickname,
            auth.user.role.as_str(),
            auth.user.must_change_password,
        )?;
        Ok(())
    }

    async fn ensure_access_token(&self) -> Result<String, AppError> {
        if let Some(token) = self
            .access_token
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .valid_token()
        {
            return Ok(token);
        }
        self.refresh_token_now().await
    }

    async fn refresh_token_now(&self) -> Result<String, AppError> {
        let refresh_token = self.db.load_refresh_token()?;
        if refresh_token.trim().is_empty() {
            return Err(AppError::Auth(
                "未登录或登录凭据已过期，请重新登录".to_string(),
            ));
        }
        let base = self.server_base_url()?;
        let resp = self
            .client
            .post(format!("{}/api/v1/auth/refresh", base))
            .json(&RefreshRequest { refresh_token })
            .send()
            .await
            .map_err(|e| AppError::Sync(format!("无法连接云端服务: {}", e)))?;

        if resp.status() == StatusCode::UNAUTHORIZED || resp.status() == StatusCode::FORBIDDEN {
            self.access_token
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
            let _ = self.db.save_refresh_token("");
            let msg = extract_error_message(resp)
                .await
                .unwrap_or_else(|| "登录会话已失效，请重新登录".to_string());
            return Err(AppError::Auth(msg));
        }

        let auth: AuthTokenResponse = parse_response(resp).await?;
        self.apply_auth_response(&auth)?;
        Ok(auth.access_token)
    }

    async fn authed_json<B: Serialize, R: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&B>,
    ) -> Result<R, AppError> {
        let base = self.server_base_url()?;
        let url = format!("{}{}", base, path);
        let token = self.ensure_access_token().await?;

        let mut req = self
            .client
            .request(method.clone(), &url)
            .bearer_auth(&token);
        if let Some(b) = body {
            req = req.json(b);
        }
        let resp = req
            .send()
            .await
            .map_err(|e| AppError::Sync(format!("网络请求失败: {}", e)))?;

        if resp.status() == StatusCode::UNAUTHORIZED {
            // Access Token 失效：尝试用 Refresh Token 刷新一次并重发
            let fresh = self.refresh_token_now().await?;
            let mut retry = self.client.request(method, &url).bearer_auth(&fresh);
            if let Some(b) = body {
                retry = retry.json(b);
            }
            let retry_resp = retry
                .send()
                .await
                .map_err(|e| AppError::Sync(format!("网络请求失败: {}", e)))?;
            return parse_response(retry_resp).await;
        }

        parse_response(resp).await
    }

    async fn authed_empty(&self, method: Method, path: &str) -> Result<(), AppError> {
        let base = self.server_base_url()?;
        let url = format!("{}{}", base, path);
        let token = self.ensure_access_token().await?;

        let resp = self
            .client
            .request(method.clone(), &url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| AppError::Sync(format!("网络请求失败: {}", e)))?;

        let resp = if resp.status() == StatusCode::UNAUTHORIZED {
            let fresh = self.refresh_token_now().await?;
            self.client
                .request(method, &url)
                .bearer_auth(&fresh)
                .send()
                .await
                .map_err(|e| AppError::Sync(format!("网络请求失败: {}", e)))?
        } else {
            resp
        };

        if resp.status().is_success() {
            return Ok(());
        }
        Err(status_to_error(resp).await)
    }

    /// 活动库切换或收到远端同步变更后，重建调度器、刷新托盘、重置便签并通知全部窗口刷新。
    fn refresh_runtime_views(&self) {
        if let Some(state) = self.app.try_state::<AppState>() {
            let _ = state.scheduler.reset_and_reschedule();
        }
        sticky::reset_sticky_windows_for_active_db(&self.app, &self.db);
        tray::request_refresh(&self.app);
        let _ = self.app.emit("data-updated", ());
    }
}

async fn parse_response<T: DeserializeOwned>(resp: reqwest::Response) -> Result<T, AppError> {
    if resp.status().is_success() {
        resp.json::<T>()
            .await
            .map_err(|e| AppError::Sync(format!("解析服务端响应失败: {}", e)))
    } else {
        Err(status_to_error(resp).await)
    }
}

async fn extract_error_message(resp: reqwest::Response) -> Option<String> {
    let bytes = resp.bytes().await.ok()?;
    if let Ok(body) = serde_json::from_slice::<ApiErrorBody>(&bytes) {
        if !body.message.trim().is_empty() {
            return Some(body.message);
        }
    }
    None
}

async fn status_to_error(resp: reqwest::Response) -> AppError {
    let status = resp.status();
    let msg = extract_error_message(resp)
        .await
        .unwrap_or_else(|| format!("服务端返回错误 (HTTP {})", status.as_u16()));
    if status == StatusCode::UNAUTHORIZED {
        AppError::Auth(msg)
    } else {
        AppError::Sync(msg)
    }
}
