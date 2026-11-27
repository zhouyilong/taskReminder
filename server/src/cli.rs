//! 应急兜底运维 CLI 子命令（`docker exec` 直接调用）。

use chrono::Utc;
use clap::{Args, Subcommand};
use sqlx::{MySqlPool, Row};
use task_reminder_core::kinds::UserRole;
use uuid::Uuid;

use crate::auth::{generate_random_password, hash_password, validate_password, validate_username};

#[derive(Subcommand, Debug)]
pub enum AdminCommand {
    /// 列出全部账号元数据
    List,
    /// 创建新账号
    Create(CreateUserArgs),
    /// 重置指定账号密码并强制下线其全部设备
    ResetPassword(ResetPasswordArgs),
    /// 启用或禁用账号
    SetStatus(SetStatusArgs),
}

#[derive(Args, Debug)]
pub struct CreateUserArgs {
    #[arg(long)]
    pub username: String,
    #[arg(long, default_value = "")]
    pub nickname: String,
    #[arg(long, default_value = "user")]
    pub role: String,
    /// 不指定时自动生成 16 位随机密码并打印
    #[arg(long)]
    pub password: Option<String>,
    /// 是否要求首次登录修改密码
    #[arg(long, default_value_t = false)]
    pub must_change_password: bool,
}

#[derive(Args, Debug)]
pub struct ResetPasswordArgs {
    #[arg(long)]
    pub username: String,
    /// 不指定时自动生成 16 位随机密码并打印
    #[arg(long)]
    pub password: Option<String>,
}

#[derive(Args, Debug)]
pub struct SetStatusArgs {
    #[arg(long)]
    pub username: String,
    #[arg(long)]
    pub enabled: bool,
}

pub async fn run_admin_command(pool: &MySqlPool, cmd: AdminCommand) -> Result<(), String> {
    match cmd {
        AdminCommand::List => {
            let rows = sqlx::query(
                "SELECT username, nickname, role, status, must_change_password, last_login_at, created_at
                 FROM users ORDER BY created_at ASC",
            )
            .fetch_all(pool)
            .await
            .map_err(|e| format!("查询账号失败: {}", e))?;

            println!(
                "{:<20} {:<16} {:<8} {:<8} {:<24}",
                "USERNAME", "NICKNAME", "ROLE", "STATUS", "LAST_LOGIN"
            );
            for row in rows {
                let username: String = row.try_get("username").unwrap_or_default();
                let nickname: String = row.try_get("nickname").unwrap_or_default();
                let role: String = row.try_get("role").unwrap_or_default();
                let status: i8 = row.try_get("status").unwrap_or(0);
                let last_login: Option<chrono::NaiveDateTime> =
                    row.try_get("last_login_at").unwrap_or(None);
                let status_label = if status == 1 { "enabled" } else { "disabled" };
                let last_login_str = last_login
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                    .unwrap_or_else(|| "-".to_string());
                println!(
                    "{:<20} {:<16} {:<8} {:<8} {:<24}",
                    username, nickname, role, status_label, last_login_str
                );
            }
            Ok(())
        }
        AdminCommand::Create(args) => {
            let username = validate_username(&args.username).map_err(|e| e.to_string())?;
            let role = UserRole::parse(&args.role)
                .ok_or_else(|| "角色仅支持 admin 或 user".to_string())?;
            let (password, generated) = match args.password {
                Some(pw) if !pw.is_empty() => {
                    validate_password(&pw).map_err(|e| e.to_string())?;
                    (pw, false)
                }
                _ => (generate_random_password(16), true),
            };
            let hash = hash_password(&password).map_err(|e| e.to_string())?;
            let id = Uuid::new_v4().to_string();

            sqlx::query(
                "INSERT INTO users (id, username, nickname, password_hash, role, status, must_change_password)
                 VALUES (?, ?, ?, ?, ?, 1, ?)",
            )
            .bind(&id)
            .bind(&username)
            .bind(args.nickname.trim())
            .bind(&hash)
            .bind(role.as_str())
            .bind(if args.must_change_password { 1 } else { 0 })
            .execute(pool)
            .await
            .map_err(|e| format!("创建账号失败: {}", e))?;

            println!("已创建账号 {} (role={})", username, role.as_str());
            if generated {
                println!("初始随机密码: {}", password);
            }
            Ok(())
        }
        AdminCommand::ResetPassword(args) => {
            let (password, generated) = match args.password {
                Some(pw) if !pw.is_empty() => {
                    validate_password(&pw).map_err(|e| e.to_string())?;
                    (pw, false)
                }
                _ => (generate_random_password(16), true),
            };
            let hash = hash_password(&password).map_err(|e| e.to_string())?;
            let now = Utc::now().naive_utc();

            let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
            let row = sqlx::query("SELECT id FROM users WHERE username = ?")
                .bind(args.username.trim())
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("账号 {} 不存在", args.username))?;
            let user_id: String = row.try_get("id").map_err(|e| e.to_string())?;

            sqlx::query(
                "UPDATE users SET password_hash = ?, must_change_password = 1 WHERE id = ?",
            )
            .bind(&hash)
            .bind(&user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

            sqlx::query(
                "UPDATE user_sessions SET revoked_at = ? WHERE user_id = ? AND revoked_at IS NULL",
            )
            .bind(now)
            .bind(&user_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;

            tx.commit().await.map_err(|e| e.to_string())?;

            println!("已重置账号 {} 的密码并吊销全部在线会话", args.username);
            if generated {
                println!("临时新密码: {}", password);
            }
            Ok(())
        }
        AdminCommand::SetStatus(args) => {
            let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
            let row =
                sqlx::query("SELECT id, role, status FROM users WHERE username = ? FOR UPDATE")
                    .bind(args.username.trim())
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("账号 {} 不存在", args.username))?;
            let user_id: String = row.try_get("id").map_err(|e| e.to_string())?;
            let role_str: String = row.try_get("role").map_err(|e| e.to_string())?;
            let current_enabled = row.try_get::<i8, _>("status").unwrap_or(0) == 1;

            if !args.enabled
                && current_enabled
                && UserRole::parse(&role_str).is_some_and(UserRole::is_admin)
            {
                let cnt: i64 = sqlx::query(
                    "SELECT COUNT(*) AS cnt FROM users WHERE role = 'admin' AND status = 1",
                )
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| e.to_string())?
                .try_get("cnt")
                .map_err(|e| e.to_string())?;
                if cnt <= 1 {
                    return Err("至少保留一个启用状态的管理员账号".to_string());
                }
            }

            sqlx::query("UPDATE users SET status = ? WHERE id = ?")
                .bind(if args.enabled { 1 } else { 0 })
                .bind(&user_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;

            if !args.enabled {
                let now = Utc::now().naive_utc();
                sqlx::query(
                    "UPDATE user_sessions SET revoked_at = ? WHERE user_id = ? AND revoked_at IS NULL",
                )
                .bind(now)
                .bind(&user_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            }

            tx.commit().await.map_err(|e| e.to_string())?;
            println!(
                "已将账号 {} 状态设为 {}",
                args.username,
                if args.enabled { "启用" } else { "禁用" }
            );
            Ok(())
        }
    }
}
