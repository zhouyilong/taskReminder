//! 本机敏感设置（WebDAV 密码、同步密码）的存放位置。
//!
//! Windows 上存入凭据管理器，数据库中只留空值；其他平台（以及凭据库写入失败时）仍存本地数据库。
//! Linux 的 Secret Service 可能未运行或开机后未解锁，读取失败会让同步因缺少密码而中断，
//! 所以只在 Windows 上启用。

use std::sync::Arc;

/// `settings.secret_storage` 的取值。
pub const STORAGE_DB: &str = "db";
pub const STORAGE_KEYRING: &str = "keyring";

const KEY_WEBDAV_PASSWORD: &str = "webdav_password";
const KEY_SYNC_PASSPHRASE: &str = "sync_passphrase";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Secrets {
    pub webdav_password: String,
    pub sync_passphrase: String,
}

impl Secrets {
    pub fn is_empty(&self) -> bool {
        self.webdav_password.is_empty() && self.sync_passphrase.is_empty()
    }
}

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

pub fn read_secrets(store: &dyn SecretStore, scope: &str) -> Result<Secrets, String> {
    Ok(Secrets {
        webdav_password: store
            .get(&account(scope, KEY_WEBDAV_PASSWORD))?
            .unwrap_or_default(),
        sync_passphrase: store
            .get(&account(scope, KEY_SYNC_PASSPHRASE))?
            .unwrap_or_default(),
    })
}

/// 写入凭据；空值删除对应条目。
pub fn write_secrets(
    store: &dyn SecretStore,
    scope: &str,
    secrets: &Secrets,
) -> Result<(), String> {
    for (key, value) in [
        (KEY_WEBDAV_PASSWORD, &secrets.webdav_password),
        (KEY_SYNC_PASSPHRASE, &secrets.sync_passphrase),
    ] {
        let name = account(scope, key);
        if value.is_empty() {
            store.delete(&name)?;
        } else {
            store.set(&name, value)?;
        }
    }
    Ok(())
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

    fn raw_columns(path: &std::path::Path) -> (String, String, String) {
        let conn = rusqlite::Connection::open(path).unwrap();
        conn.query_row(
            "SELECT COALESCE(webdav_password, ''), sync_passphrase, secret_storage FROM settings WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap()
    }

    fn set_passwords(db: &DbManager, password: &str, passphrase: &str) {
        let mut settings = db.load_settings().unwrap();
        settings.webdav_password = password.to_string();
        settings.sync_passphrase = passphrase.to_string();
        db.save_settings(&settings).unwrap();
    }

    #[test]
    fn without_store_passwords_stay_in_database() {
        let (path, dir) = temp_path("plain");
        let db = DbManager::with_secret_store(path.clone(), None).unwrap();
        set_passwords(&db, "pw", "passphrase");
        assert_eq!(
            raw_columns(&path),
            ("pw".into(), "passphrase".into(), STORAGE_DB.into())
        );
        assert_eq!(db.load_settings().unwrap().secret_storage, STORAGE_DB);
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn store_keeps_passwords_out_of_database() {
        let (path, dir) = temp_path("keyring");
        let store = Arc::new(MemoryStore::default());
        let db = open(&path, &store);
        set_passwords(&db, "pw", "passphrase");
        assert_eq!(
            raw_columns(&path),
            ("".into(), "".into(), STORAGE_KEYRING.into())
        );
        let loaded = db.load_settings().unwrap();
        assert_eq!(loaded.webdav_password, "pw");
        assert_eq!(loaded.sync_passphrase, "passphrase");
        assert_eq!(loaded.secret_storage, STORAGE_KEYRING);

        // 重新打开（没有缓存）时从凭据库读取。
        drop(db);
        let reopened = open(&path, &store);
        assert_eq!(reopened.load_settings().unwrap().webdav_password, "pw");

        // 清空密码会删除凭据条目。
        set_passwords(&reopened, "", "passphrase");
        assert_eq!(store.entries.lock().unwrap().len(), 1);
        drop(reopened);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn plaintext_passwords_are_migrated_on_open() {
        let (path, dir) = temp_path("migrate");
        let db = DbManager::with_secret_store(path.clone(), None).unwrap();
        set_passwords(&db, "old-pw", "old-passphrase");
        drop(db);

        let store = Arc::new(MemoryStore::default());
        let db = open(&path, &store);
        assert_eq!(
            raw_columns(&path),
            ("".into(), "".into(), STORAGE_KEYRING.into())
        );
        let loaded = db.load_settings().unwrap();
        assert_eq!(loaded.webdav_password, "old-pw");
        assert_eq!(loaded.sync_passphrase, "old-passphrase");
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn write_failure_falls_back_to_database() {
        let (path, dir) = temp_path("fallback");
        let store = Arc::new(MemoryStore::default());
        store.fail_set.store(true, Ordering::SeqCst);
        let db = open(&path, &store);
        set_passwords(&db, "pw", "passphrase");
        assert_eq!(
            raw_columns(&path),
            ("pw".into(), "passphrase".into(), STORAGE_DB.into())
        );
        assert_eq!(db.load_settings().unwrap().webdav_password, "pw");

        // 凭据库恢复后，下次启动会迁移过去。
        drop(db);
        store.fail_set.store(false, Ordering::SeqCst);
        let db = open(&path, &store);
        assert_eq!(raw_columns(&path).2, STORAGE_KEYRING);
        assert_eq!(db.load_settings().unwrap().webdav_password, "pw");
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn read_failure_does_not_erase_stored_passwords() {
        let (path, dir) = temp_path("read-failure");
        let store = Arc::new(MemoryStore::default());
        let db = open(&path, &store);
        set_passwords(&db, "pw", "passphrase");
        drop(db);

        store.fail_get.store(true, Ordering::SeqCst);
        let db = open(&path, &store);
        let settings = db.load_settings().unwrap();
        assert_eq!(settings.webdav_password, "");
        // 其他设置的保存（如同步状态）不能用空密码覆盖凭据库。
        db.save_settings(&settings).unwrap();
        db.mark_local_change().unwrap();
        assert_eq!(store.entries.lock().unwrap().len(), 2);
        assert_eq!(raw_columns(&path).2, STORAGE_KEYRING);

        store.fail_get.store(false, Ordering::SeqCst);
        drop(db);
        let db = open(&path, &store);
        assert_eq!(db.load_settings().unwrap().webdav_password, "pw");
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn empty_device_id_is_fixed_before_migration() {
        let (path, dir) = temp_path("device-id");
        let db = DbManager::with_secret_store(path.clone(), None).unwrap();
        set_passwords(&db, "pw", "");
        drop(db);
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute("UPDATE settings SET webdav_device_id = NULL", [])
            .unwrap();
        drop(conn);

        let store = Arc::new(MemoryStore::default());
        let db = open(&path, &store);
        let first = db.load_settings().unwrap();
        assert!(!first.webdav_device_id.is_empty());
        assert_eq!(first.webdav_password, "pw");
        drop(db);
        let db = open(&path, &store);
        let second = db.load_settings().unwrap();
        assert_eq!(second.webdav_device_id, first.webdav_device_id);
        assert_eq!(second.webdav_password, "pw");
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unchanged_settings_do_not_rewrite_the_store() {
        let (path, dir) = temp_path("no-rewrite");
        let store = Arc::new(MemoryStore::default());
        let db = open(&path, &store);
        set_passwords(&db, "pw", "passphrase");
        // 之后凭据库写入失败：只要密码不变，保存其他设置不受影响，也不会退回数据库。
        store.fail_set.store(true, Ordering::SeqCst);
        db.mark_local_change().unwrap();
        db.update_sync_status("success", None).unwrap();
        assert_eq!(
            raw_columns(&path),
            ("".into(), "".into(), STORAGE_KEYRING.into())
        );
        drop(db);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// 真实的 Windows 凭据管理器读写（CI 的 Windows 任务会执行），用随机条目名并在结束时删除。
    #[cfg(windows)]
    #[test]
    fn windows_credential_manager_roundtrip() {
        let store = platform_store().expect("Windows 上应有凭据库");
        let scope = format!("test-{}", uuid::Uuid::new_v4());
        let secrets = Secrets {
            webdav_password: "密码 pw-1".to_string(),
            sync_passphrase: "passphrase-2".to_string(),
        };
        write_secrets(store.as_ref(), &scope, &secrets).unwrap();
        assert_eq!(read_secrets(store.as_ref(), &scope).unwrap(), secrets);
        write_secrets(store.as_ref(), &scope, &Secrets::default()).unwrap();
        assert_eq!(
            read_secrets(store.as_ref(), &scope).unwrap(),
            Secrets::default()
        );
    }
}
