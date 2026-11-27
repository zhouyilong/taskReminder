//! 密码哈希、JWT 签发/校验、Refresh Token 哈希与 Axum 认证提取器。

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use task_reminder_core::kinds::UserRole;

use crate::error::{ServerError, ServerResult};
use crate::state::AppState;

pub const MIN_PASSWORD_LEN: usize = 6;
pub const MAX_PASSWORD_LEN: usize = 128;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub sub: String,
    pub username: String,
    pub role: UserRole,
    pub sid: String,
    pub exp: usize,
    pub iat: usize,
}

pub fn hash_password(password: &str) -> ServerResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| ServerError::Internal(format!("密码哈希失败: {}", e)))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// 生成随机强密码（不含易混淆字符 0/O/1/l/I）。
pub fn generate_random_password(len: usize) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%^&*";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let idx = rng.gen_range(0..ALPHABET.len());
            ALPHABET[idx] as char
        })
        .collect()
}

pub fn generate_refresh_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill(&mut bytes);
    hex::encode(bytes)
}

pub fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn validate_username(username: &str) -> ServerResult<String> {
    let trimmed = username.trim();
    if trimmed.len() < 2 || trimmed.len() > 64 {
        return Err(ServerError::BadRequest(
            "用户名长度需在 2 到 64 个字符之间".to_string(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        return Err(ServerError::BadRequest(
            "用户名仅允许字母、数字、下划线、连字符和点".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

pub fn validate_password(password: &str) -> ServerResult<()> {
    let len = password.chars().count();
    if !(MIN_PASSWORD_LEN..=MAX_PASSWORD_LEN).contains(&len) {
        return Err(ServerError::BadRequest(format!(
            "密码长度需在 {} 到 {} 个字符之间",
            MIN_PASSWORD_LEN, MAX_PASSWORD_LEN
        )));
    }
    Ok(())
}

pub fn issue_access_token(
    secret: &str,
    user_id: &str,
    username: &str,
    role: UserRole,
    session_id: &str,
    ttl_minutes: i64,
) -> ServerResult<(String, i64)> {
    let now = Utc::now();
    let expires_in = ttl_minutes * 60;
    let exp = (now + Duration::seconds(expires_in)).timestamp() as usize;
    let claims = AccessTokenClaims {
        sub: user_id.to_string(),
        username: username.to_string(),
        role,
        sid: session_id.to_string(),
        exp,
        iat: now.timestamp() as usize,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| ServerError::Internal(format!("签发令牌失败: {}", e)))?;
    Ok((token, expires_in))
}

pub fn decode_access_token(secret: &str, token: &str) -> ServerResult<AccessTokenClaims> {
    decode::<AccessTokenClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| ServerError::Unauthorized("登录凭据无效或已过期".to_string()))
}

/// 已登录用户上下文（从 Authorization: Bearer <token> 解析并实时验证账号与会话有效性）。
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: String,
    pub username: String,
    pub role: UserRole,
    pub session_id: String,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ServerError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        let token = header
            .strip_prefix("Bearer ")
            .or_else(|| header.strip_prefix("bearer "))
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| ServerError::Unauthorized("缺少访问令牌".to_string()))?;

        let claims = decode_access_token(&state.config.jwt_secret, token)?;

        // 实时检查用户状态与会话是否已被吊销（管理员禁用/重置密码后立即生效）。
        let row = sqlx::query(
            "SELECT u.username, u.role, u.status, s.revoked_at, s.expires_at
             FROM users u
             INNER JOIN user_sessions s ON s.user_id = u.id
             WHERE u.id = ? AND s.id = ?",
        )
        .bind(&claims.sub)
        .bind(&claims.sid)
        .fetch_optional(&state.pool)
        .await?;

        let Some(row) = row else {
            return Err(ServerError::Unauthorized(
                "登录会话已失效，请重新登录".to_string(),
            ));
        };

        let status: i8 = row.try_get("status")?;
        if status != 1 {
            return Err(ServerError::Unauthorized("该账号已被禁用".to_string()));
        }
        let revoked_at: Option<chrono::NaiveDateTime> = row.try_get("revoked_at")?;
        if revoked_at.is_some() {
            return Err(ServerError::Unauthorized(
                "登录会话已被注销，请重新登录".to_string(),
            ));
        }
        let expires_at: chrono::NaiveDateTime = row.try_get("expires_at")?;
        if expires_at < Utc::now().naive_utc() {
            return Err(ServerError::Unauthorized(
                "登录会话已过期，请重新登录".to_string(),
            ));
        }

        let role_str: String = row.try_get("role")?;
        let role = UserRole::parse(&role_str).unwrap_or(UserRole::User);
        let username: String = row.try_get("username")?;

        Ok(AuthUser {
            user_id: claims.sub,
            username,
            role,
            session_id: claims.sid,
        })
    }
}

/// 管理员权限提取器：非 admin 直接返回 403。
#[derive(Clone, Debug)]
pub struct AdminUser(pub AuthUser);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = ServerError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if !user.role.is_admin() {
            return Err(ServerError::Forbidden("需要管理员权限".to_string()));
        }
        Ok(AdminUser(user))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_roundtrip() {
        let hash = hash_password("Secret#12345").unwrap();
        assert!(verify_password("Secret#12345", &hash));
        assert!(!verify_password("wrong", &hash));
    }

    #[test]
    fn jwt_roundtrip() {
        let (token, ttl) = issue_access_token(
            "test-secret-key-32-bytes-minimum!!",
            "u-1",
            "alice",
            UserRole::Admin,
            "s-1",
            30,
        )
        .unwrap();
        assert_eq!(ttl, 1800);
        let claims = decode_access_token("test-secret-key-32-bytes-minimum!!", &token).unwrap();
        assert_eq!(claims.sub, "u-1");
        assert_eq!(claims.role, UserRole::Admin);
        assert_eq!(claims.sid, "s-1");
    }

    #[test]
    fn random_password_has_requested_length() {
        let pw = generate_random_password(16);
        assert_eq!(pw.chars().count(), 16);
    }
}
