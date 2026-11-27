//! 云端模式、认证、模式切换合并与管理员账号管理 Tauri 命令。

use serde::Deserialize;
use task_reminder_core::kinds::UserRole;
use task_reminder_core::protocol::{
    AdminCreateUserRequest, AdminResetPasswordRequest, AdminUpdateUserRequest, AdminUserSummary,
    HealthResponse, UserInfo,
};
use tauri::State;

use crate::cloud::{
    SwitchToCloudPreview, SwitchToCloudResult, SwitchToLocalPreview, SwitchToLocalResult,
};
use crate::commands::{into_api, ApiResult};
use crate::models::CloudStatus;
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudLoginPayload {
    #[serde(default)]
    pub server_url: Option<String>,
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudChangePasswordPayload {
    pub current_password: String,
    pub new_password: String,
}

#[tauri::command]
pub fn get_cloud_status(state: State<AppState>) -> ApiResult<CloudStatus> {
    into_api(state.sync.get_status())
}

#[tauri::command]
pub async fn cloud_test_connection(
    state: State<'_, AppState>,
    server_url: String,
) -> ApiResult<HealthResponse> {
    into_api(state.sync.test_connection(&server_url).await)
}

#[tauri::command]
pub async fn cloud_set_server_url(
    state: State<'_, AppState>,
    server_url: String,
) -> ApiResult<CloudStatus> {
    into_api(state.sync.set_server_url(&server_url).await)
}

#[tauri::command]
pub async fn cloud_login(
    state: State<'_, AppState>,
    payload: CloudLoginPayload,
) -> ApiResult<CloudStatus> {
    into_api(
        state
            .sync
            .login(payload.server_url, &payload.username, &payload.password)
            .await,
    )
}

#[tauri::command]
pub async fn cloud_logout(state: State<'_, AppState>) -> ApiResult<CloudStatus> {
    into_api(state.sync.logout().await)
}

#[tauri::command]
pub async fn cloud_sync_now(state: State<'_, AppState>) -> ApiResult<CloudStatus> {
    into_api(state.sync.sync_now_async().await)?;
    into_api(state.sync.get_status())
}

#[tauri::command]
pub async fn cloud_update_profile(
    state: State<'_, AppState>,
    nickname: String,
) -> ApiResult<UserInfo> {
    into_api(state.sync.update_profile(&nickname).await)
}

#[tauri::command]
pub async fn cloud_change_password(
    state: State<'_, AppState>,
    payload: CloudChangePasswordPayload,
) -> ApiResult<UserInfo> {
    into_api(
        state
            .sync
            .change_password(&payload.current_password, &payload.new_password)
            .await,
    )
}

#[tauri::command]
pub async fn preview_switch_to_cloud(
    state: State<'_, AppState>,
) -> ApiResult<SwitchToCloudPreview> {
    into_api(state.sync.preview_switch_to_cloud().await)
}

#[tauri::command]
pub async fn switch_to_cloud(
    state: State<'_, AppState>,
    merge_local: bool,
) -> ApiResult<SwitchToCloudResult> {
    into_api(state.sync.switch_to_cloud(merge_local).await)
}

#[tauri::command]
pub async fn preview_switch_to_local(
    state: State<'_, AppState>,
) -> ApiResult<SwitchToLocalPreview> {
    into_api(state.sync.preview_switch_to_local().await)
}

#[tauri::command]
pub async fn switch_to_local(
    state: State<'_, AppState>,
    merge_cloud: bool,
) -> ApiResult<SwitchToLocalResult> {
    into_api(state.sync.switch_to_local(merge_cloud).await)
}

// ---- 管理员账号管理命令 ----

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminCreateUserPayload {
    pub username: String,
    #[serde(default)]
    pub nickname: String,
    #[serde(default)]
    pub role: UserRole,
    pub password: String,
    #[serde(default)]
    pub must_change_password: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminUpdateUserPayload {
    pub user_id: String,
    pub nickname: Option<String>,
    pub role: Option<UserRole>,
    pub enabled: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminResetPasswordPayload {
    pub user_id: String,
    pub new_password: String,
    #[serde(default = "default_true")]
    pub must_change_password: bool,
}

fn default_true() -> bool {
    true
}

#[tauri::command]
pub async fn admin_list_users(state: State<'_, AppState>) -> ApiResult<Vec<AdminUserSummary>> {
    into_api(state.sync.admin_list_users().await)
}

#[tauri::command]
pub async fn admin_create_user(
    state: State<'_, AppState>,
    payload: AdminCreateUserPayload,
) -> ApiResult<AdminUserSummary> {
    into_api(
        state
            .sync
            .admin_create_user(&AdminCreateUserRequest {
                username: payload.username,
                nickname: payload.nickname,
                role: payload.role,
                password: payload.password,
                must_change_password: payload.must_change_password,
            })
            .await,
    )
}

#[tauri::command]
pub async fn admin_update_user(
    state: State<'_, AppState>,
    payload: AdminUpdateUserPayload,
) -> ApiResult<AdminUserSummary> {
    into_api(
        state
            .sync
            .admin_update_user(
                &payload.user_id,
                &AdminUpdateUserRequest {
                    nickname: payload.nickname,
                    role: payload.role,
                    enabled: payload.enabled,
                },
            )
            .await,
    )
}

#[tauri::command]
pub async fn admin_reset_password(
    state: State<'_, AppState>,
    payload: AdminResetPasswordPayload,
) -> ApiResult<AdminUserSummary> {
    into_api(
        state
            .sync
            .admin_reset_password(
                &payload.user_id,
                &AdminResetPasswordRequest {
                    new_password: payload.new_password,
                    must_change_password: payload.must_change_password,
                },
            )
            .await,
    )
}

#[tauri::command]
pub async fn admin_delete_user(state: State<'_, AppState>, user_id: String) -> ApiResult<()> {
    into_api(state.sync.admin_delete_user(&user_id).await)
}
