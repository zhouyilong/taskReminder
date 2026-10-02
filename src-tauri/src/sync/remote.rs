//! 与远端存储同步的主流程：下载（必要时解密）、合并、上传（必要时加密），以及更换同步密码。

use super::*;

pub fn test_webdav(settings: &AppSettings) -> Result<(bool, String), AppError> {
    let client = WebDavClient::new(settings)?;
    let (ok, message) = client.test_connection()?;
    if !ok {
        return Ok((ok, message));
    }
    Ok(check_encryption(&client, settings))
}

/// 连接成功后检查加密设置与远端数据是否匹配，给出提示。
pub(super) fn check_encryption(store: &dyn RemoteStore, settings: &AppSettings) -> (bool, String) {
    let remote_encrypted = match store.exists(REMOTE_ENC_NAME) {
        Ok(value) => value,
        Err(err) => return (false, format!("连接成功，但读取远端数据失败：{}", err)),
    };
    if !settings.sync_encryption_enabled {
        return if remote_encrypted {
            (
                false,
                "连接成功，但远端数据已加密：请开启端到端加密并填写同步密码".to_string(),
            )
        } else {
            (true, "连接成功".to_string())
        };
    }
    if let Err(err) = sync_crypto::validate_passphrase(&settings.sync_passphrase) {
        return (false, format!("连接成功，但{}", err));
    }
    if !remote_encrypted {
        return (true, "连接成功，下次同步时将加密上传".to_string());
    }
    match store
        .get(REMOTE_ENC_NAME)
        .map_err(|e| e.to_string())
        .and_then(|data| {
            sync_crypto::decrypt(&data, &settings.sync_passphrase).map_err(|e| e.to_string())
        }) {
        Ok(_) => (true, "连接成功，同步密码正确".to_string()),
        Err(message) => (false, format!("连接成功，但{}", message)),
    }
}

/// 远端存储的最小接口：WebDAV 客户端实现它，测试中用内存实现替代。
pub(super) trait RemoteStore {
    fn exists(&self, name: &str) -> Result<bool, AppError>;
    fn get(&self, name: &str) -> Result<Vec<u8>, AppError>;
    fn put(&self, name: &str, data: Vec<u8>) -> Result<(), AppError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RemoteSyncResult {
    /// 远端没有数据，上传了本地快照。
    FirstUpload,
    /// 与远端合并后上传。
    Merged,
}

pub(super) fn placeholder_content() -> Vec<u8> {
    let mut data = PLACEHOLDER_MARKER.to_vec();
    data.extend_from_slice(
        "\n此目录的任务提醒同步数据已开启端到端加密，保存在 taskreminder.db.enc 中。\n\
         请把所有设备升级到 2.0 及以上版本，并在“云同步设置”中开启加密、填写相同的同步密码。\n"
            .as_bytes(),
    );
    data
}

/// 解密认证失败单独归类：界面据此提示“同步密码已在其他设备更换”，并暂停自动同步。
pub(super) fn crypto_error(err: sync_crypto::CryptoError) -> AppError {
    match err {
        sync_crypto::CryptoError::Decrypt => AppError::SyncPassphrase(err.to_string()),
        other => AppError::Sync(other.to_string()),
    }
}

/// 加锁之后的同步主体：下载（必要时解密）远端快照并合并，再上传本地快照（必要时加密）。
///
/// - 远端已加密而本机未开启加密：报错，不上传，避免把明文传回服务器。
/// - 同步密码错误：解密失败即报错，不会用另一个密码覆盖远端数据。
/// - 本机开启加密而远端还是明文：合并明文后改为上传加密文件，并把明文文件替换为占位说明。
pub(super) fn sync_with_remote(
    store: &dyn RemoteStore,
    db: &DbManager,
    settings: &AppSettings,
    params: KdfParams,
) -> Result<RemoteSyncResult, AppError> {
    sync_with_remote_as(store, db, settings, &settings.sync_passphrase, params)
}

/// `sync_with_remote` 的主体：用 `settings.sync_passphrase` 解密远端，用 `upload_passphrase`
/// 加密上传。平时两者相同；更换同步密码时用新密码上传（见 `change_passphrase_on_remote`）。
pub(super) fn sync_with_remote_as(
    store: &dyn RemoteStore,
    db: &DbManager,
    settings: &AppSettings,
    upload_passphrase: &str,
    params: KdfParams,
) -> Result<RemoteSyncResult, AppError> {
    let encrypt = settings.sync_encryption_enabled;
    if encrypt {
        sync_crypto::validate_passphrase(&settings.sync_passphrase).map_err(crypto_error)?;
        sync_crypto::validate_passphrase(upload_passphrase).map_err(crypto_error)?;
    }

    let remote_snapshot = if store.exists(REMOTE_ENC_NAME)? {
        if !encrypt {
            return Err(AppError::Sync(
                "远端数据已开启端到端加密：请在云同步设置中开启加密并填写同步密码".to_string(),
            ));
        }
        let data = store.get(REMOTE_ENC_NAME)?;
        Some(sync_crypto::decrypt(&data, &settings.sync_passphrase).map_err(crypto_error)?)
    } else if store.exists(REMOTE_DB_NAME)? {
        let data = store.get(REMOTE_DB_NAME)?;
        if data.starts_with(PLACEHOLDER_MARKER) {
            // 加密文件被手动删除，只剩占位说明：按首次同步处理。
            None
        } else if data.starts_with(SQLITE_MAGIC) {
            Some(data)
        } else {
            return Err(AppError::Sync("远端同步文件格式无效".to_string()));
        }
    } else {
        None
    };

    let merged = remote_snapshot.is_some();
    if let Some(snapshot) = remote_snapshot {
        merge_snapshot_bytes(&db.db_path(), &snapshot)?;
        // 合并后再清理过期墓碑，随后上传的快照中也不再包含它们。
        db.purge_expired_tombstones(TOMBSTONE_RETENTION_DAYS_SYNC)?;
    }

    let local_snapshot = export_local_snapshot_bytes(&db.db_path())?;
    if encrypt {
        let data = sync_crypto::encrypt(&local_snapshot, upload_passphrase, params)
            .map_err(crypto_error)?;
        store.put(REMOTE_ENC_NAME, data)?;
        // 先传加密文件再替换明文：中途失败时远端仍有一份可用的数据。
        store.put(REMOTE_DB_NAME, placeholder_content())?;
    } else {
        store.put(REMOTE_DB_NAME, local_snapshot)?;
    }

    Ok(if merged {
        RemoteSyncResult::Merged
    } else {
        RemoteSyncResult::FirstUpload
    })
}

/// 更换同步密码前的检查（不访问网络）：已开启加密、当前密码与本机保存的一致、新密码有效且不同。
pub(super) fn validate_passphrase_change(
    settings: &AppSettings,
    current: &str,
    new: &str,
) -> Result<(), AppError> {
    if !settings.sync_encryption_enabled {
        return Err(AppError::Invalid(
            "请先开启端到端加密并保存设置".to_string(),
        ));
    }
    if current != settings.sync_passphrase {
        return Err(AppError::Invalid("当前同步密码不正确".to_string()));
    }
    sync_crypto::validate_passphrase(new).map_err(|e| AppError::Invalid(e.to_string()))?;
    if new == current {
        return Err(AppError::Invalid("新密码与当前密码相同".to_string()));
    }
    Ok(())
}

/// 更换同步密码（需已持有同步锁）：用当前密码下载、解密并合并远端数据，再用新密码加密上传；
/// **上传成功后**才把新密码保存到本机。任一步失败都保留旧密码：
/// - 当前密码解密失败：不上传，远端不变；
/// - 上传失败：本机仍是旧密码，远端的 `.enc` 要么没变、要么已是新密码（此时本机同步会提示
///   密码不匹配，填入新密码即可）。
pub(super) fn change_passphrase_on_remote(
    store: &dyn RemoteStore,
    db: &DbManager,
    settings: &AppSettings,
    new_passphrase: &str,
    params: KdfParams,
) -> Result<RemoteSyncResult, AppError> {
    validate_passphrase_change(settings, &settings.sync_passphrase, new_passphrase)?;
    let result = sync_with_remote_as(store, db, settings, new_passphrase, params)?;
    let mut updated = db.load_settings()?;
    updated.sync_passphrase = new_passphrase.to_string();
    db.save_settings(&updated).map_err(|err| {
        AppError::Sync(format!(
            "云端已改用新密码，但本机保存失败（{}）：请在同步密码中填写新密码后保存",
            err
        ))
    })?;
    Ok(result)
}
