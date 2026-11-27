//! HTTP 客户端的统一入口（云端同步 REST/SSE、节假日数据下载）。
//!
//! reqwest 使用 rustls + ring，与 `tauri-plugin-updater` 共用同一套 TLS 依赖；证书按系统信任库校验。

fn ensure_rustls_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// 返回阻塞客户端的构建器。必须在 `spawn_blocking` 或普通线程中使用，不要放进 `async` 任务。
pub fn blocking_client_builder() -> reqwest::blocking::ClientBuilder {
    ensure_rustls_provider();
    reqwest::blocking::Client::builder()
}

/// 返回异步客户端的构建器（用于云端 REST 与 SSE 长连接）。
pub fn async_client_builder() -> reqwest::ClientBuilder {
    ensure_rustls_provider();
    reqwest::Client::builder()
}

#[cfg(test)]
mod tests {
    #[test]
    fn builds_client_repeatedly() {
        super::blocking_client_builder().build().unwrap();
        super::async_client_builder().build().unwrap();
    }
}
