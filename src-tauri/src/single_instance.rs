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
            .open(path)?;
        match file.try_lock_exclusive() {
            Ok(_) => Ok(Some(Self { _file: file })),
            Err(_) => Ok(None),
        }
    }
}
