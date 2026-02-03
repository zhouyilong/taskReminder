use std::path::PathBuf;

use tauri::AppHandle;

use crate::errors::AppError;

const APP_DIR_NAME: &str = "TaskReminderApp";
const DATA_DIR_NAME: &str = "data";

pub fn resolve_data_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    if let Ok(override_dir) = std::env::var("TASKREMINDER_DATA_DIR") {
        let path = PathBuf::from(override_dir);
        std::fs::create_dir_all(&path)?;
        return Ok(path);
    }

    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let path = PathBuf::from(appdata).join(APP_DIR_NAME).join(DATA_DIR_NAME);
            std::fs::create_dir_all(&path)?;
            return Ok(path);
        }
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let path = PathBuf::from(profile)
                .join("AppData")
                .join("Roaming")
                .join(APP_DIR_NAME)
                .join(DATA_DIR_NAME);
            std::fs::create_dir_all(&path)?;
            return Ok(path);
        }
    }

    if let Some(base) = tauri::api::path::app_data_dir(&app.config()) {
        let path = base.join(APP_DIR_NAME).join(DATA_DIR_NAME);
        std::fs::create_dir_all(&path)?;
        return Ok(path);
    }

    Err(AppError::System("无法确定数据目录".to_string()))
}

pub fn db_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join("taskreminder.db")
}

pub fn lock_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join(".taskreminder.lock")
}
