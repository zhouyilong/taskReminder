//! HTTP 客户端的统一入口（WebDAV 同步、节假日数据下载）。
//!
//! reqwest 使用 rustls + ring，与 `tauri-plugin-updater` 共用同一套 TLS 依赖；证书按系统信任库校验
//! （`rustls-platform-verifier`），企业代理安装的根证书也能识别。

/// 返回阻塞客户端的构建器。必须在 `spawn_blocking` 或普通线程中使用，不要放进 `async` 任务。
pub fn blocking_client_builder() -> reqwest::blocking::ClientBuilder {
    // rustls 需要进程级的加密实现；已安装过（例如更新器先装了）时返回 Err，忽略即可。
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::blocking::Client::builder()
}

#[cfg(test)]
mod tests {
    #[test]
    fn builds_client_repeatedly() {
        super::blocking_client_builder().build().unwrap();
        super::blocking_client_builder().build().unwrap();
    }
}
