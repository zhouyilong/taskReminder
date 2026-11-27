//! 服务端启动迁移与初始管理员自动创建。

use sqlx::{MySqlPool, Row};
use uuid::Uuid;

use crate::auth::{generate_random_password, hash_password, validate_username};
use crate::config::ServerConfig;
use crate::error::ServerResult;

pub async fn run_migrations(pool: &MySqlPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

/// 首次启动检查是否存在 `role = 'admin'` 的账号；若不存在则自动创建。
pub async fn ensure_initial_admin(pool: &MySqlPool, config: &ServerConfig) -> ServerResult<()> {
    let count: i64 = sqlx::query("SELECT COUNT(*) AS cnt FROM users WHERE role = 'admin'")
        .fetch_one(pool)
        .await?
        .try_get("cnt")?;

    if count > 0 {
        return Ok(());
    }

    let username = validate_username(&config.init_admin_username)?;
    let (password, generated) = match &config.init_admin_password {
        Some(pw) if !pw.is_empty() => (pw.clone(), false),
        _ => (generate_random_password(16), true),
    };
    let password_hash = hash_password(&password)?;
    let user_id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO users (id, username, nickname, password_hash, role, status, must_change_password)
         VALUES (?, ?, ?, ?, 'admin', 1, ?)",
    )
    .bind(&user_id)
    .bind(&username)
    .bind("管理员")
    .bind(&password_hash)
    .bind(if generated { 1 } else { 0 })
    .execute(pool)
    .await?;

    if generated {
        tracing::warn!(
            username = %username,
            initial_password = %password,
            "已自动创建初始管理员账号（此随机密码仅在首次启动日志中打印一次，请妥善保存并在登录后及时修改）"
        );
    } else {
        tracing::info!(username = %username, "已根据环境变量创建初始管理员账号");
    }

    Ok(())
}
