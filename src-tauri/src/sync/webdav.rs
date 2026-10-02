//! WebDAV 客户端与远端同步锁。

use super::*;

#[derive(Serialize, Deserialize, Clone)]
pub(super) struct LockInfo {
    #[serde(rename = "deviceId", alias = "device_id")]
    device_id: String,
    #[serde(rename = "expiresAt", alias = "expires_at")]
    expires_at: i64,
}

impl LockInfo {
    pub(super) fn new(device_id: &str) -> Self {
        let expires_at = time::unix_millis() + LOCK_TTL_SECONDS * 1000;
        Self {
            device_id: device_id.to_string(),
            expires_at,
        }
    }

    pub(super) fn is_expired(&self) -> bool {
        self.expires_at_millis() <= time::unix_millis()
    }

    pub(super) fn expires_at_millis(&self) -> i64 {
        if self.expires_at > 1_000_000_000_000 {
            self.expires_at
        } else {
            self.expires_at * 1000
        }
    }
}

pub(super) struct WebDavClient {
    base_url: String,
    auth_header: Option<String>,
    client: reqwest::blocking::Client,
}

impl WebDavClient {
    pub(super) fn new(settings: &AppSettings) -> Result<Self, AppError> {
        let base_url = build_base_url(&settings.webdav_url, &settings.webdav_root_path);
        let auth_header = build_auth_header(&settings.webdav_username, &settings.webdav_password);
        Ok(Self {
            base_url,
            auth_header,
            client: crate::http::blocking_client_builder()
                .build()
                .map_err(|e| AppError::Sync(e.to_string()))?,
        })
    }

    pub(super) fn test_connection(&self) -> Result<(bool, String), AppError> {
        let mut req = self
            .client
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").unwrap(),
                &self.base_url,
            )
            .header("Depth", "0");
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let resp = req.send().map_err(|e| AppError::Sync(e.to_string()))?;
        let status = resp.status();
        if status == StatusCode::MULTI_STATUS || status == StatusCode::OK {
            return Ok((true, "连接成功".to_string()));
        }
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Ok((false, "认证失败".to_string()));
        }
        Ok((false, format!("连接失败，状态码: {}", status)))
    }

    pub(super) fn try_acquire_lock(&self, info: &LockInfo) -> Result<bool, AppError> {
        if let Some(existing) = self.get_lock()? {
            if !existing.is_expired() && existing.device_id != info.device_id {
                return Ok(false);
            }
        }
        let url = build_url(&self.base_url, LOCK_FILE_NAME);
        let body = serde_json::to_vec(info).map_err(|e| AppError::Sync(e.to_string()))?;
        let mut req = self
            .client
            .put(url)
            .body(body)
            .header("Content-Type", "application/json");
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let resp = req.send().map_err(|e| AppError::Sync(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(AppError::Sync(format!(
                "写入锁失败，状态码: {}",
                resp.status()
            )));
        }
        Ok(true)
    }

    pub(super) fn get_lock(&self) -> Result<Option<LockInfo>, AppError> {
        let url = build_url(&self.base_url, LOCK_FILE_NAME);
        let mut req = self.client.get(url);
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let resp = match req.send() {
            Ok(resp) => resp,
            Err(_) => return Ok(None),
        };
        if resp.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Ok(None);
        }
        let bytes = match resp.bytes() {
            Ok(bytes) => bytes,
            Err(_) => return Ok(None),
        };
        let lock: LockInfo = match serde_json::from_slice(&bytes) {
            Ok(lock) => lock,
            Err(_) => return Ok(None),
        };
        Ok(Some(lock))
    }

    pub(super) fn release_lock(&self) {
        let url = build_url(&self.base_url, LOCK_FILE_NAME);
        let mut req = self.client.delete(url);
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let _ = req.send();
    }
}

impl RemoteStore for WebDavClient {
    fn exists(&self, name: &str) -> Result<bool, AppError> {
        let url = build_url(&self.base_url, name);
        let mut req = self.client.head(url);
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let resp = req.send().map_err(|e| AppError::Sync(e.to_string()))?;
        Ok(resp.status() == StatusCode::OK)
    }

    fn get(&self, name: &str) -> Result<Vec<u8>, AppError> {
        let url = build_url(&self.base_url, name);
        let mut req = self.client.get(url);
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let resp = req.send().map_err(|e| AppError::Sync(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(AppError::Sync(format!(
                "下载失败，状态码: {}",
                resp.status()
            )));
        }
        let bytes = resp.bytes().map_err(|e| AppError::Sync(e.to_string()))?;
        Ok(bytes.to_vec())
    }

    fn put(&self, name: &str, data: Vec<u8>) -> Result<(), AppError> {
        let url = build_url(&self.base_url, name);
        let mut req = self
            .client
            .put(url)
            .body(data)
            .header("Content-Type", "application/octet-stream");
        if let Some(auth) = &self.auth_header {
            req = req.header("Authorization", auth);
        }
        let resp = req.send().map_err(|e| AppError::Sync(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(AppError::Sync(format!(
                "上传失败，状态码: {}",
                resp.status()
            )));
        }
        Ok(())
    }
}

pub(super) fn build_base_url(url: &str, root: &str) -> String {
    let mut base = url.trim().trim_end_matches('/').to_string();
    let mut root = root.trim().to_string();
    if !root.is_empty() {
        if !root.starts_with('/') {
            root = format!("/{}", root);
        }
        root = root.trim_end_matches('/').to_string();
        base.push_str(&root);
    }
    base
}

pub(super) fn build_url(base: &str, name: &str) -> String {
    if name.is_empty() {
        return base.to_string();
    }
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        name.trim_start_matches('/')
    )
}

pub(super) fn build_auth_header(username: &str, password: &str) -> Option<String> {
    if username.trim().is_empty() {
        return None;
    }
    let token =
        base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", username, password));
    Some(format!("Basic {}", token))
}
