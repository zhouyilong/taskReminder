use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Database(String),
    #[error("IO错误: {0}")]
    Io(String),
    #[error("参数错误: {0}")]
    Invalid(String),
    #[error("同步错误: {0}")]
    Sync(String),
    /// 远端加密数据无法用本机的同步密码解密（密码错误、在其他设备上更换过，或数据损坏）。
    #[error("同步错误: {0}")]
    SyncPassphrase(String),
    #[error("系统错误: {0}")]
    System(String),
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Database(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err.to_string())
    }
}
