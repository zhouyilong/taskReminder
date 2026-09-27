//! 前端通过 `invoke` 调用的 Tauri 命令，按业务拆分到各子模块；注册见 `main.rs`。

pub mod data;
pub mod notification;
pub mod recurring;
pub mod settings;
pub mod sticky;
pub mod system;
pub mod tasks;
pub mod trash;

use crate::errors::AppError;

pub type ApiResult<T> = Result<T, String>;

pub fn into_api<T>(result: Result<T, AppError>) -> ApiResult<T> {
    result.map_err(|e| e.to_string())
}
