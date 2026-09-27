use std::fs::OpenOptions;
use std::path::Path;

use fs2::FileExt;

use crate::errors::AppError;

pub struct InstanceLock {
    _file: std::fs::File,
}

impl InstanceLock {
    pub fn try_lock(path: &Path) -> Result<Option<Self>, AppError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            // 锁文件没有内容，只用来加独占锁；不截断，避免抢锁前改动另一实例的文件。
            .truncate(false)
            .open(path)?;
        match file.try_lock_exclusive() {
            Ok(_) => Ok(Some(Self { _file: file })),
            Err(_) => Ok(None),
        }
    }
}
