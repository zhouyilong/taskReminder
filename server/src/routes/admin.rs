//! 管理员最小维护账号管理接口（仅 `role = 'admin'` 可访问；不暴露任何其他用户的业务内容）。

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{NaiveDateTime, Utc};
use sqlx::Row;
use task_reminder_core::kinds::UserRole;
use task_reminder_core::protocol::{
    AdminCreateUserRequest, AdminResetPasswordRequest, AdminUpdateUserRequest, AdminUserSummary,
};
use uuid::Uuid;

use crate::auth::{hash_password, validate_password, validate_username, AdminUser};
use crate::error::{ServerError, ServerResult};
use crate::routes::auth::format_utc_datetime;
use crate::state::AppState;

async fn fetch_admin_user_summary(
    state: &AppState,
    user_id: &str,
) -> ServerResult<AdminUserSummary> {
    let row = sqlx::query(
        "SELECT u.id, u.username, u.nickname, u.role, u.status, u.must_change_password,
                u.last_login_at, u.created_at,
                (SELECT COUNT(*) FROM tasks t WHERE t.user_id = u.id AND t.deleted_at IS NULL) AS task_count,
                (SELECT COUNT(*) FROM recurring_tasks r WHERE r.user_id = u.id AND r.deleted_at IS NULL) AS recurring_count,
                (SELECT COUNT(*) FROM reminder_records rr WHERE rr.user_id = u.id AND rr.deleted_at IS NULL) AS record_count
         FROM users u
         WHERE u.id = ?",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| ServerError::NotFound("目标账号不存在".to_string()))?;

    row_to_summary(&row)
}

fn row_to_summary(row: &sqlx::mysql::MySqlRow) -> ServerResult<AdminUserSummary> {
    let role_str: String = row.try_get("role")?;
    let status: i8 = row.try_get("status")?;
    let must_change: i8 = row.try_get("must_change_password")?;
    let last_login_at: Option<NaiveDateTime> = row.try_get("last_login_at")?;
    let created_at: NaiveDateTime = row.try_get("created_at")?;
    Ok(AdminUserSummary {
        id: row.try_get("id")?,
        username: row.try_get("username")?,
        nickname: row.try_get("nickname")?,
        role: UserRole::parse(&role_str).unwrap_or(UserRole::User),
        enabled: status == 1,
        must_change_password: must_change != 0,
        last_login_at: last_login_at.map(format_utc_datetime),
        created_at: format_utc_datetime(created_at),
        task_count: row.try_get("task_count")?,
        recurring_count: row.try_get("recurring_count")?,
        record_count: row.try_get("record_count")?,
    })
}

async fn active_admin_count(tx: &mut sqlx::Transaction<'_, sqlx::MySql>) -> ServerResult<i64> {
    let cnt: i64 =
        sqlx::query("SELECT COUNT(*) AS cnt FROM users WHERE role = 'admin' AND status = 1")
            .fetch_one(&mut **tx)
            .await?
            .try_get("cnt")?;
    Ok(cnt)
}

pub async fn list_users(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> ServerResult<Json<Vec<AdminUserSummary>>> {
    let rows = sqlx::query(
        "SELECT u.id, u.username, u.nickname, u.role, u.status, u.must_change_password,
                u.last_login_at, u.created_at,
                (SELECT COUNT(*) FROM tasks t WHERE t.user_id = u.id AND t.deleted_at IS NULL) AS task_count,
                (SELECT COUNT(*) FROM recurring_tasks r WHERE r.user_id = u.id AND r.deleted_at IS NULL) AS recurring_count,
                (SELECT COUNT(*) FROM reminder_records rr WHERE rr.user_id = u.id AND rr.deleted_at IS NULL) AS record_count
         FROM users u
         ORDER BY u.created_at ASC",
    )
    .fetch_all(&state.pool)
    .await?;

    let mut users = Vec::with_capacity(rows.len());
    for row in &rows {
        users.push(row_to_summary(row)?);
    }
    Ok(Json(users))
}

pub async fn create_user(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(payload): Json<AdminCreateUserRequest>,
) -> ServerResult<(StatusCode, Json<AdminUserSummary>)> {
    let username = validate_username(&payload.username)?;
    validate_password(&payload.password)?;
    let nickname: String = payload.nickname.trim().chars().take(64).collect();
    let password_hash = hash_password(&payload.password)?;
    let id = Uuid::new_v4().to_string();

    let result = sqlx::query(
        "INSERT INTO users (id, username, nickname, password_hash, role, status, must_change_password)
         VALUES (?, ?, ?, ?, ?, 1, ?)",
    )
    .bind(&id)
    .bind(&username)
    .bind(&nickname)
    .bind(&password_hash)
    .bind(payload.role.as_str())
    .bind(if payload.must_change_password { 1 } else { 0 })
    .execute(&state.pool)
    .await;

    match result {
        Ok(_) => {
            let summary = fetch_admin_user_summary(&state, &id).await?;
            Ok((StatusCode::CREATED, Json(summary)))
        }
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => Err(
            ServerError::Conflict(format!("用户名「{}」已存在", username)),
        ),
        Err(err) => Err(ServerError::Database(err)),
    }
}

pub async fn update_user(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(target_id): Path<String>,
    Json(payload): Json<AdminUpdateUserRequest>,
) -> ServerResult<Json<AdminUserSummary>> {
    let mut tx = state.pool.begin().await?;

    let row = sqlx::query("SELECT id, nickname, role, status FROM users WHERE id = ? FOR UPDATE")
        .bind(&target_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ServerError::NotFound("目标账号不存在".to_string()))?;

    let current_nickname: String = row.try_get("nickname")?;
    let current_role_str: String = row.try_get("role")?;
    let current_role = UserRole::parse(&current_role_str).unwrap_or(UserRole::User);
    let current_enabled = row.try_get::<i8, _>("status")? == 1;

    let next_nickname: String = payload
        .nickname
        .map(|n| n.trim().chars().take(64).collect())
        .unwrap_or(current_nickname);
    let next_role = payload.role.unwrap_or(current_role);
    let next_enabled = payload.enabled.unwrap_or(current_enabled);

    if target_id == admin.user_id && !next_enabled {
        return Err(ServerError::BadRequest(
            "不能禁用当前登录的管理员账号".to_string(),
        ));
    }
    if target_id == admin.user_id && !next_role.is_admin() {
        return Err(ServerError::BadRequest(
            "不能取消自己的管理员角色".to_string(),
        ));
    }

    let losing_active_admin =
        current_role.is_admin() && current_enabled && (!next_role.is_admin() || !next_enabled);
    if losing_active_admin && active_admin_count(&mut tx).await? <= 1 {
        return Err(ServerError::BadRequest(
            "至少保留一个启用状态的管理员账号".to_string(),
        ));
    }

    sqlx::query("UPDATE users SET nickname = ?, role = ?, status = ? WHERE id = ?")
        .bind(&next_nickname)
        .bind(next_role.as_str())
        .bind(if next_enabled { 1 } else { 0 })
        .bind(&target_id)
        .execute(&mut *tx)
        .await?;

    // 禁用账号时立即吊销其全部会话。
    if !next_enabled {
        let now = Utc::now().naive_utc();
        sqlx::query(
            "UPDATE user_sessions SET revoked_at = ? WHERE user_id = ? AND revoked_at IS NULL",
        )
        .bind(now)
        .bind(&target_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let summary = fetch_admin_user_summary(&state, &target_id).await?;
    Ok(Json(summary))
}

pub async fn reset_password(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(target_id): Path<String>,
    Json(payload): Json<AdminResetPasswordRequest>,
) -> ServerResult<Json<AdminUserSummary>> {
    validate_password(&payload.new_password)?;
    let password_hash = hash_password(&payload.new_password)?;
    let now = Utc::now().naive_utc();

    let mut tx = state.pool.begin().await?;
    let affected =
        sqlx::query("UPDATE users SET password_hash = ?, must_change_password = ? WHERE id = ?")
            .bind(&password_hash)
            .bind(if payload.must_change_password { 1 } else { 0 })
            .bind(&target_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();

    if affected == 0 {
        return Err(ServerError::NotFound("目标账号不存在".to_string()));
    }

    // 立即吊销目标用户全部在线会话。
    sqlx::query("UPDATE user_sessions SET revoked_at = ? WHERE user_id = ? AND revoked_at IS NULL")
        .bind(now)
        .bind(&target_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    let summary = fetch_admin_user_summary(&state, &target_id).await?;
    Ok(Json(summary))
}

pub async fn delete_user(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(target_id): Path<String>,
) -> ServerResult<StatusCode> {
    if target_id == admin.user_id {
        return Err(ServerError::BadRequest(
            "不能删除当前登录的管理员账号".to_string(),
        ));
    }

    let mut tx = state.pool.begin().await?;
    let row = sqlx::query("SELECT role, status FROM users WHERE id = ? FOR UPDATE")
        .bind(&target_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ServerError::NotFound("目标账号不存在".to_string()))?;

    let role_str: String = row.try_get("role")?;
    let enabled = row.try_get::<i8, _>("status")? == 1;
    if UserRole::parse(&role_str).is_some_and(UserRole::is_admin)
        && enabled
        && active_admin_count(&mut tx).await? <= 1
    {
        return Err(ServerError::BadRequest(
            "不能删除最后一个有效管理员".to_string(),
        ));
    }

    // 外键 ON DELETE CASCADE 会原子级联删除 user_sessions / tasks / recurring_tasks / reminder_records。
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(&target_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
