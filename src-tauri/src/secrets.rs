//! 本机敏感凭据（云端 Refresh Token）的存放位置。
//!
//! Windows 上存入系统凭据管理器，数据库中只留空值；其他平台（以及凭据库写入失败时）仍存本地数据库。
//! v3.0.0 升级时自动清理旧版本遗留的 `webdav_password` 与 `sync_passphrase` 条目。

use std::sync::Arc;

/// `settings.secret_storage` 的取值。
pub const STORAGE_DB: &str = "db";
pub const STORAGE_KEYRING: &str = "keyring";

const KEY_CLOUD_REFRESH_TOKEN: &str = "cloud_refresh_token";
const LEGACY_KEY_WEBDAV_PASSWORD: &str = "webdav_password";
const LEGACY_KEY_SYNC_PASSPHRASE: &str = "sync_passphrase";

/// 凭据存储。`get` 在条目不存在时返回 `Ok(None)`，`delete` 对不存在的条目视为成功。
pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>, String>;
    fn set(&self, account: &str, value: &str) -> Result<(), String>;
    fn delete(&self, account: &str) -> Result<(), String>;
}

/// 条目名带上数据库的设备 ID，数据目录不同（如开发与正式实例）时互不影响。
fn account(scope: &str, key: &str) -> String {
    format!("{}:{}", scope, key)
}

pub fn read_refresh_token(store: &dyn SecretStore, scope: &str) -> Result<String, String> {
    Ok(store
        .get(&account(scope, KEY_CLOUD_REFRESH_TOKEN))?
        .unwrap_or_default())
}

/// 写入或清空云端 Refresh Token。
pub fn write_refresh_token(
    store: &dyn SecretStore,
    scope: &str,
    token: &str,
) -> Result<(), String> {
    let name = account(scope, KEY_CLOUD_REFRESH_TOKEN);
    if token.is_empty() {
        store.delete(&name)
    } else {
        store.set(&name, token)
    }
}

/// 清理 2.2.x 遗留的 WebDAV 凭据条目。
pub fn cleanup_legacy_webdav_secrets(store: &dyn SecretStore, scope: &str) {
    for key in [LEGACY_KEY_WEBDAV_PASSWORD, LEGACY_KEY_SYNC_PASSPHRASE] {
        let _ = store.delete(&account(scope, key));
    }
}

/// 当前平台可用的凭据存储；不支持的平台返回 None（存本地数据库）。
pub fn platform_store() -> Option<Arc<dyn SecretStore>> {
    #[cfg(windows)]
    {
        Some(Arc::new(KeyringStore::new()))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
struct KeyringStore {
    service: &'static str,
}

#[cfg(windows)]
impl KeyringStore {
    fn new() -> Self {
        Self {
            service: if crate::paths::is_dev_mode() {
                "TaskReminderApp-dev"
            } else {
                "TaskReminderApp"
            },
        }
    }

    fn entry(&self, account: &str) -> Result<keyring::Entry, String> {
        keyring::Entry::new(self.service, account).map_err(|e| e.to_string())
    }
}

#[cfg(windows)]
impl SecretStore for KeyringStore {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        match self.entry(account)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(err.to_string()),
        }
    }

    fn set(&self, account: &str, value: &str) -> Result<(), String> {
        self.entry(account)?
            .set_password(value)
            .map_err(|e| e.to_string())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        match self.entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(err.to_string()),
        }
    }
}

/// 测试用的内存凭据库，可模拟读取或写入失败。
#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore {
    pub entries: std::sync::Mutex<std::collections::HashMap<String, String>>,
    pub fail_get: std::sync::atomic::AtomicBool,
    pub fail_set: std::sync::atomic::AtomicBool,
}

#[cfg(test)]
impl SecretStore for MemoryStore {
    fn get(&self, account: &str) -> Result<Option<String>, String> {
        if self.fail_get.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("凭据库不可用".to_string());
        }
        Ok(self.entries.lock().unwrap().get(account).cloned())
    }

    fn set(&self, account: &str, value: &str) -> Result<(), String> {
        if self.fail_set.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("凭据库不可用".to_string());
        }
        self.entries
            .lock()
            .unwrap()
            .insert(account.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<(), String> {
        if self.fail_set.load(std::sync::atomic::Ordering::SeqCst) {
            return Err("凭据库不可用".to_string());
        }
        self.entries.lock().unwrap().remove(account);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;
    use crate::db::DbManager;

    fn temp_path(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "taskreminder-secrets-{}-{}",
            name,
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        (dir.join("test.db"), dir)
    }

    fn open(path: &std::path::Path, store: &Arc<MemoryStore>) -> DbManager {
        DbManager::with_secret_store(
            path.to_path_buf(),
            Some(store.clone() as Arc<dyn SecretStore>),
        )
        .unwrap()
    }

    fn raw_token_columns(path: &std::path::Path) -> (String, String) {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.query_row(
            "SELECT COALESCE(cloud_refresh_token, ''), secret_storage FROM settings WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
    }

    #[test]
    fn without_store_token_stays_in_database() {
        let (path, dir) = temp_path("plain");
        let db = DbManager::with_secret_store(path.clone(), None).unwrap();
        db.save_refresh_token("rt-123").unwrap();
        assert_eq!(
            raw_token_columns(&path),
            ("rt-123".into(), STORAGE_DB.into())
        );
        assert_eq!(db.load_refresh_token().unwrap(), "rt-123");
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn store_keeps_refresh_token_out_of_database() {
        let (path, dir) = temp_path("keyring");
        let store = Arc::new(MemoryStore::default());
        let db = open(&path, &store);
        db.save_refresh_token("rt-secret").unwrap();
        assert_eq!(
            raw_token_columns(&path),
            ("".into(), STORAGE_KEYRING.into())
        );
        assert_eq!(db.load_refresh_token().unwrap(), "rt-secret");

        drop(db);
        let reopened = open(&path, &store);
        assert_eq!(reopened.load_refresh_token().unwrap(), "rt-secret");

        reopened.save_refresh_token("").unwrap();
        assert!(store.entries.lock().unwrap().is_empty());
        drop(reopened);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn write_failure_falls_back_to_database() {
        let (path, dir) = temp_path("fallback");
        let store = Arc::new(MemoryStore::default());
        store.fail_set.store(true, Ordering::SeqCst);
        let db = open(&path, &store);
        db.save_refresh_token("rt-fb").unwrap();
        assert_eq!(
            raw_token_columns(&path),
            ("rt-fb".into(), STORAGE_DB.into())
        );
        assert_eq!(db.load_refresh_token().unwrap(), "rt-fb");

        drop(db);
        store.fail_set.store(false, Ordering::SeqCst);
        let db = open(&path, &store);
        assert_eq!(raw_token_columns(&path).1, STORAGE_KEYRING);
        assert_eq!(db.load_refresh_token().unwrap(), "rt-fb");
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_webdav_secrets_are_cleaned_on_open() {
        let (path, dir) = temp_path("legacy-cleanup");
        let store = Arc::new(MemoryStore::default());
        let db = open(&path, &store);
        let device_id = db.load_settings().unwrap().device_id;
        store
            .set(&account(&device_id, LEGACY_KEY_WEBDAV_PASSWORD), "old-pw")
            .unwrap();
        store
            .set(&account(&device_id, LEGACY_KEY_SYNC_PASSPHRASE), "old-pp")
            .unwrap();
        drop(db);

        let _reopened = open(&path, &store);
        assert!(store
            .get(&account(&device_id, LEGACY_KEY_WEBDAV_PASSWORD))
            .unwrap()
            .is_none());
        assert!(store
            .get(&account(&device_id, LEGACY_KEY_SYNC_PASSPHRASE))
            .unwrap()
            .is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
