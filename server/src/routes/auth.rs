//! 认证与本人资料接口。

use axum::{extract::State, http::StatusCode, Json};
use chrono::{Duration, NaiveDateTime, Utc};
use sqlx::Row;
use task_reminder_core::kinds::UserRole;
use task_reminder_core::protocol::{
    AuthTokenResponse, ChangePasswordRequest, LoginRequest, LogoutRequest, RefreshRequest,
    UpdateProfileRequest, UserInfo,
};
use uuid::Uuid;

use crate::auth::{
    generate_refresh_token, hash_password, hash_refresh_token, issue_access_token,
    validate_password, verify_password, AuthUser,
};
use crate::error::{ServerError, ServerResult};
use crate::state::AppState;

pub(crate) fn format_utc_datetime(dt: NaiveDateTime) -> String {
    chrono::DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc).to_rfc3339()
}

pub(crate) async fn fetch_user_info(state: &AppState, user_id: &str) -> ServerResult<UserInfo> {
    let row = sqlx::query(
        "SELECT id, username, nickname, role, status, must_change_password, last_login_at, created_at
         FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ServerError::NotFound("账号不存在".to_string()))?;

    let role_str: String = row.try_get("role")?;
    let status: i8 = row.try_get("status")?;
    let must_change: i8 = row.try_get("must_change_password")?;
    let last_login_at: Option<NaiveDateTime> = row.try_get("last_login_at")?;
    let created_at: NaiveDateTime = row.try_get("created_at")?;

    Ok(UserInfo {
        id: row.try_get("id")?,
        username: row.try_get("username")?,
        nickname: row.try_get("nickname")?,
        role: UserRole::parse(&role_str).unwrap_or(UserRole::User),
        enabled: status == 1,
        must_change_password: must_change != 0,
        last_login_at: last_login_at.map(format_utc_datetime),
        created_at: format_utc_datetime(created_at),
    })
}

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> ServerResult<Json<AuthTokenResponse>> {
    let username = payload.username.trim();
    if username.is_empty() || payload.password.is_empty() {
        return Err(ServerError::BadRequest("请输入用户名和密码".to_string()));
    }
    let device_id = payload.device_id.trim();
    if device_id.is_empty() || device_id.len() > 64 {
        return Err(ServerError::BadRequest("无效的设备标识".to_string()));
    }
    let device_name: String = payload.device_name.trim().chars().take(128).collect();

    let row = sqlx::query(
        "SELECT id, username, password_hash, role, status FROM users WHERE username = ?",
    )
    .bind(username)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ServerError::Unauthorized("用户名或密码错误".to_string()))?;

    let user_id: String = row.try_get("id")?;
    let db_username: String = row.try_get("username")?;
    let password_hash: String = row.try_get("password_hash")?;
    let role_str: String = row.try_get("role")?;
    let status: i8 = row.try_get("status")?;

    if !verify_password(&payload.password, &password_hash) {
        return Err(ServerError::Unauthorized("用户名或密码错误".to_string()));
    }
    if status != 1 {
        return Err(ServerError::Forbidden("该账号已被管理员禁用".to_string()));
    }

    let role = UserRole::parse(&role_str).unwrap_or(UserRole::User);
    let session_id = Uuid::new_v4().to_string();
    let refresh_token = generate_refresh_token();
    let refresh_hash = hash_refresh_token(&refresh_token);
    let now = Utc::now().naive_utc();
    let expires_at = now + Duration::days(state.config.refresh_token_ttl_days);

    sqlx::query(
        "INSERT INTO user_sessions (id, user_id, device_id, device_name, refresh_token_hash, expires_at, last_used_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(&user_id)
    .bind(device_id)
    .bind(&device_name)
    .bind(&refresh_hash)
    .bind(expires_at)
    .bind(now)
    .execute(&state.pool)
    .await?;

    sqlx::query("UPDATE users SET last_login_at = ? WHERE id = ?")
        .bind(now)
        .bind(&user_id)
        .execute(&state.pool)
        .await?;

    let (access_token, expires_in_seconds) = issue_access_token(
        &state.config.jwt_secret,
        &user_id,
        &db_username,
        role,
        &session_id,
        state.config.access_token_ttl_minutes,
    )?;

    let user = fetch_user_info(&state, &user_id).await?;
    Ok(Json(AuthTokenResponse {
        access_token,
        refresh_token,
        expires_in_seconds,
        user,
    }))
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(payload): Json<RefreshRequest>,
) -> ServerResult<Json<AuthTokenResponse>> {
    let raw_token = payload.refresh_token.trim();
    if raw_token.is_empty() {
        return Err(ServerError::Unauthorized("缺少刷新令牌".to_string()));
    }
    let token_hash = hash_refresh_token(raw_token);

    let mut tx = state.pool.begin().await?;
    let row = sqlx::query(
        "SELECT s.id AS session_id, s.user_id, s.expires_at, s.revoked_at,
                u.username, u.role, u.status
         FROM user_sessions s
         INNER JOIN users u ON u.id = s.user_id
         WHERE s.refresh_token_hash = ?
         FOR UPDATE",
    )
    .bind(&token_hash)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| ServerError::Unauthorized("刷新令牌无效，请重新登录".to_string()))?;

    let session_id: String = row.try_get("session_id")?;
    let user_id: String = row.try_get("user_id")?;
    let expires_at: NaiveDateTime = row.try_get("expires_at")?;
    let revoked_at: Option<NaiveDateTime> = row.try_get("revoked_at")?;
    let username: String = row.try_get("username")?;
    let role_str: String = row.try_get("role")?;
    let status: i8 = row.try_get("status")?;

    let now = Utc::now().naive_utc();
    if status != 1 {
        return Err(ServerError::Unauthorized("该账号已被禁用".to_string()));
    }
    if revoked_at.is_some() || expires_at < now {
        return Err(ServerError::Unauthorized(
            "登录会话已失效，请重新登录".to_string(),
        ));
    }

    // 轮换 Refresh Token，延长过期时间。
    let next_refresh_token = generate_refresh_token();
    let next_refresh_hash = hash_refresh_token(&next_refresh_token);
    let next_expires_at = now + Duration::days(state.config.refresh_token_ttl_days);

    sqlx::query(
        "UPDATE user_sessions
         SET refresh_token_hash = ?, expires_at = ?, last_used_at = ?
         WHERE id = ?",
    )
    .bind(&next_refresh_hash)
    .bind(next_expires_at)
    .bind(now)
    .bind(&session_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let role = UserRole::parse(&role_str).unwrap_or(UserRole::User);
    let (access_token, expires_in_seconds) = issue_access_token(
        &state.config.jwt_secret,
        &user_id,
        &username,
        role,
        &session_id,
        state.config.access_token_ttl_minutes,
    )?;

    let user = fetch_user_info(&state, &user_id).await?;
    Ok(Json(AuthTokenResponse {
        access_token,
        refresh_token: next_refresh_token,
        expires_in_seconds,
        user,
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    auth: AuthUser,
    body: Option<Json<LogoutRequest>>,
) -> ServerResult<StatusCode> {
    let now = Utc::now().naive_utc();
    sqlx::query("UPDATE user_sessions SET revoked_at = ? WHERE id = ? AND user_id = ?")
        .bind(now)
        .bind(&auth.session_id)
        .bind(&auth.user_id)
        .execute(&state.pool)
        .await?;

    if let Some(Json(req)) = body {
        if let Some(rt) = req.refresh_token.filter(|v| !v.trim().is_empty()) {
            let hash = hash_refresh_token(rt.trim());
            sqlx::query(
                "UPDATE user_sessions SET revoked_at = ? WHERE refresh_token_hash = ? AND user_id = ?",
            )
            .bind(now)
            .bind(&hash)
            .bind(&auth.user_id)
            .execute(&state.pool)
            .await?;
        }
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn me(State(state): State<AppState>, auth: AuthUser) -> ServerResult<Json<UserInfo>> {
    let user = fetch_user_info(&state, &auth.user_id).await?;
    Ok(Json(user))
}

pub async fn update_profile(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<UpdateProfileRequest>,
) -> ServerResult<Json<UserInfo>> {
    let nickname: String = payload.nickname.trim().chars().take(64).collect();
    sqlx::query("UPDATE users SET nickname = ? WHERE id = ?")
        .bind(&nickname)
        .bind(&auth.user_id)
        .execute(&state.pool)
        .await?;
    let user = fetch_user_info(&state, &auth.user_id).await?;
    Ok(Json(user))
}

pub async fn change_password(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<ChangePasswordRequest>,
) -> ServerResult<Json<UserInfo>> {
    validate_password(&payload.new_password)?;

    let row = sqlx::query("SELECT password_hash FROM users WHERE id = ?")
        .bind(&auth.user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ServerError::NotFound("账号不存在".to_string()))?;

    let current_hash: String = row.try_get("password_hash")?;
    if !verify_password(&payload.current_password, &current_hash) {
        return Err(ServerError::BadRequest("当前密码不正确".to_string()));
    }

    let new_hash = hash_password(&payload.new_password)?;
    let now = Utc::now().naive_utc();

    let mut tx = state.pool.begin().await?;
    sqlx::query("UPDATE users SET password_hash = ?, must_change_password = 0 WHERE id = ?")
        .bind(&new_hash)
        .bind(&auth.user_id)
        .execute(&mut *tx)
        .await?;

    // 修改密码后吊销其他设备会话，保留当前会话。
    sqlx::query(
        "UPDATE user_sessions SET revoked_at = ?
         WHERE user_id = ? AND id <> ? AND revoked_at IS NULL",
    )
    .bind(now)
    .bind(&auth.user_id)
    .bind(&auth.session_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let user = fetch_user_info(&state, &auth.user_id).await?;
    Ok(Json(user))
}
