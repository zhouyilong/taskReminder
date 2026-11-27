//! Tauri 命令模块，按业务域拆分。

pub mod cloud;
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

pub(crate) fn into_api<T>(result: Result<T, AppError>) -> ApiResult<T> {
    result.map_err(|err| err.to_string())
}
