//! 服务端环境变量配置契约。

use std::env;

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub server_addr: String,
    pub database_url: String,
    pub jwt_secret: String,
    pub init_admin_username: String,
    pub init_admin_password: Option<String>,
    pub access_token_ttl_minutes: i64,
    pub refresh_token_ttl_days: i64,
}

impl ServerConfig {
    pub fn from_env() -> Result<Self, String> {
        let server_addr = env::var("SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let database_url =
            env::var("DATABASE_URL").map_err(|_| "缺少必需环境变量 DATABASE_URL".to_string())?;
        let jwt_secret = env::var("JWT_SECRET")
            .unwrap_or_else(|_| "dev-insecure-jwt-secret-please-override-32bytes".to_string());
        let init_admin_username = env::var("INIT_ADMIN_USERNAME")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "admin".to_string());
        let init_admin_password = env::var("INIT_ADMIN_PASSWORD")
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());
        let access_token_ttl_minutes = env::var("ACCESS_TOKEN_TTL_MINUTES")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(30)
            .clamp(5, 1440);
        let refresh_token_ttl_days = env::var("REFRESH_TOKEN_TTL_DAYS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(30)
            .clamp(1, 365);

        Ok(Self {
            server_addr,
            database_url,
            jwt_secret,
            init_admin_username,
            init_admin_password,
            access_token_ttl_minutes,
            refresh_token_ttl_days,
        })
    }
}
