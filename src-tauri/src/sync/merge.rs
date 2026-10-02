//! 本地快照的导出与两个数据库的按行合并。

use super::*;

pub(super) fn temp_db_path(kind: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("taskreminder-{}-{}.db", kind, uuid::Uuid::new_v4()))
}

/// 导出本地数据库快照并清空其中的敏感设置（WebDAV 密码、同步密码）与本机专用的同步水位；
/// 远端只用到数据表。
#[cfg(test)]
pub(super) fn export_local_snapshot_bytes(db_path: &std::path::Path) -> Result<Vec<u8>, AppError> {
    export_local_snapshot(db_path).map(|(bytes, _)| bytes)
}

/// 同 `export_local_snapshot_bytes`，并返回快照中每张同步表的行 id（成功上传后记为同步水位）。
pub(super) fn export_local_snapshot(
    db_path: &std::path::Path,
) -> Result<(Vec<u8>, watermark::RowIds), AppError> {
    let snapshot = temp_db_path("snapshot");
    let result = (|| {
        {
            let conn = Connection::open(db_path)?;
            let escaped = snapshot.to_string_lossy().replace('\'', "''");
            conn.execute_batch(&format!("VACUUM INTO '{}'", escaped))?;
        }
        let ids = {
            let conn = Connection::open(&snapshot)?;
            conn.execute(
                "UPDATE settings SET webdav_password = '', sync_passphrase = ''",
                [],
            )?;
            for table in ["sync_watermark", "sync_watermark_rows"] {
                if table_exists(&conn, table)? {
                    conn.execute(&format!("DELETE FROM {}", table), [])?;
                }
            }
            watermark::snapshot_ids(&conn)?
        };
        Ok((std::fs::read(&snapshot)?, ids))
    })();
    let _ = std::fs::remove_file(&snapshot);
    result
}

fn table_exists(conn: &Connection, table: &str) -> Result<bool, AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
        [table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

#[cfg(test)]
pub(super) fn merge_snapshot_bytes(
    local_path: &std::path::Path,
    data: &[u8],
) -> Result<(), AppError> {
    merge_snapshot_bytes_with(local_path, data, None)
}

/// 合并远端快照；给出可用的同步水位时，先删除本机中已在其他设备删除并清理的行。
pub(super) fn merge_snapshot_bytes_with(
    local_path: &std::path::Path,
    data: &[u8],
    watermark: Option<&watermark::Watermark>,
) -> Result<(), AppError> {
    let remote = temp_db_path("remote");
    let result = std::fs::write(&remote, data)
        .map_err(AppError::from)
        .and_then(|_| merge_databases_with(local_path, &remote, watermark));
    let _ = std::fs::remove_file(&remote);
    result
}

pub(crate) fn merge_databases(
    local_path: &std::path::Path,
    remote_path: &std::path::Path,
) -> Result<(), AppError> {
    merge_databases_with(local_path, remote_path, None)
}

fn merge_databases_with(
    local_path: &std::path::Path,
    remote_path: &std::path::Path,
    watermark: Option<&watermark::Watermark>,
) -> Result<(), AppError> {
    let mut local = Connection::open(local_path)?;
    let remote = Connection::open(remote_path)?;
    ensure_sync_columns(&local)?;
    // 远端库不补列：合并只读它实际拥有的列，旧版本没有的列保留本机的值。
    // 若给远端补上（值为默认值），远端行胜出时会把本机的新字段清空。

    let tx = local.transaction()?;
    if let Some(watermark) = watermark {
        let dropped = watermark::drop_rows_deleted_elsewhere(&tx, &remote, watermark)?;
        if dropped > 0 {
            eprintln!("[sync] 删除了 {} 行已在其他设备删除并清理的数据", dropped);
        }
    }
    for table in SYNC_TABLES {
        merge_table(&tx, &remote, table)?;
    }
    tx.commit()?;
    Ok(())
}

/// 按行合并一张表，较新的一方胜出（见 `choose_row`）。
///
/// 参与合并的是本机表的全部同步列，包括更新版本新增、本版本不认识的列（`adopt_remote_columns`）。
/// 胜出的行只写入它所在的库实际拥有的列：远端来自不认识某列的旧版本时，该列保留本机的值，
/// 不会被清成默认值。本次才补上的列本机还没有值，本机行胜出时这些列取远端的值。
pub(super) fn merge_table(
    local: &Connection,
    remote: &Connection,
    table: &SyncTable,
) -> Result<(), AppError> {
    let adopted = adopt_remote_columns(local, remote, table)?;
    let local_columns = adopted.columns;
    let remote_names = table_columns(remote, table.name)?;
    let remote_columns: Vec<String> = local_columns
        .iter()
        .filter(|name| {
            remote_names
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case(name))
        })
        .cloned()
        .collect();

    let local_rows = load_rows(
        local,
        table.name,
        &local_columns,
        table.fallback_time_column,
    )?;
    let remote_rows = load_rows(
        remote,
        table.name,
        &remote_columns,
        table.fallback_time_column,
    )?;
    let mut all_ids: Vec<&String> = local_rows.keys().collect();
    all_ids.extend(
        remote_rows
            .keys()
            .filter(|id| !local_rows.contains_key(*id)),
    );
    if all_ids.is_empty() {
        return Ok(());
    }

    let position =
        |columns: &[String], name: &str| columns.iter().position(|c| c.eq_ignore_ascii_case(name));
    let fill_from_remote: Vec<(usize, usize)> = adopted
        .newly_added
        .iter()
        .filter_map(|name| {
            Some((
                position(&local_columns, name)?,
                position(&remote_columns, name)?,
            ))
        })
        .collect();

    let mut local_stmt = local.prepare(&upsert_sql(table.name, &local_columns))?;
    let mut remote_stmt = local.prepare(&upsert_sql(table.name, &remote_columns))?;
    for id in all_ids {
        match choose_row(local_rows.get(id), remote_rows.get(id)) {
            Some(Side::Local(row)) => match remote_rows.get(id) {
                Some(remote_row) if !fill_from_remote.is_empty() => {
                    let mut values = row.values.clone();
                    for &(local_index, remote_index) in &fill_from_remote {
                        values[local_index] = remote_row.values[remote_index].clone();
                    }
                    local_stmt.execute(params_from_iter(values.iter()))?;
                }
                _ => {
                    local_stmt.execute(params_from_iter(row.values.iter()))?;
                }
            },
            Some(Side::Remote(row)) => {
                remote_stmt.execute(params_from_iter(row.values.iter()))?;
            }
            None => {}
        }
    }
    Ok(())
}

/// 插入或只更新给出的列：没有给出的列（远端没有的列）保留本机原值。
pub(super) fn upsert_sql(table: &str, columns: &[String]) -> String {
    let quoted: Vec<String> = columns.iter().map(|c| quote_ident(c)).collect();
    let updates: Vec<String> = quoted
        .iter()
        .zip(columns)
        .filter(|(_, name)| !name.eq_ignore_ascii_case("id"))
        .map(|(q, _)| format!("{0} = excluded.{0}", q))
        .collect();
    format!(
        "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT(id) DO UPDATE SET {}",
        quote_ident(table),
        quoted.join(", "),
        vec!["?"; columns.len()].join(", "),
        updates.join(", ")
    )
}

pub(super) enum Side<'a> {
    Local(&'a RowData),
    Remote(&'a RowData),
}

pub(super) fn choose_row<'a>(
    local: Option<&'a RowData>,
    remote: Option<&'a RowData>,
) -> Option<Side<'a>> {
    match (local, remote) {
        (Some(l), None) => Some(Side::Local(l)),
        (None, Some(r)) => Some(Side::Remote(r)),
        (Some(l), Some(r)) => match (l.compare_time, r.compare_time) {
            (Some(lc), Some(rc)) if rc > lc => Some(Side::Remote(r)),
            (None, Some(_)) => Some(Side::Remote(r)),
            _ => Some(Side::Local(l)),
        },
        (None, None) => None,
    }
}

#[derive(Clone)]
pub(super) struct RowData {
    values: Vec<Value>,
    compare_time: Option<NaiveDateTime>,
}

pub(super) fn load_rows(
    conn: &Connection,
    table: &str,
    columns: &[String],
    fallback_time_column: &str,
) -> Result<HashMap<String, RowData>, AppError> {
    let quoted: Vec<String> = columns.iter().map(|c| quote_ident(c)).collect();
    let sql = format!("SELECT {} FROM {}", quoted.join(", "), quote_ident(table));
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;

    let position = |name: &str| columns.iter().position(|c| c.eq_ignore_ascii_case(name));
    let id_index = position("id").unwrap_or(0);
    let updated_index = position("updated_at");
    let deleted_index = position("deleted_at");
    let fallback_index = position(fallback_time_column);

    let mut map = HashMap::new();
    while let Some(row) = rows.next()? {
        let mut values = Vec::with_capacity(columns.len());
        for i in 0..columns.len() {
            let value: Value = row.get(i)?;
            values.push(value);
        }
        let id_value = value_to_string(&values[id_index]);
        if let Some(id) = id_value {
            let updated = updated_index.and_then(|idx| value_to_string(&values[idx]));
            let deleted = deleted_index.and_then(|idx| value_to_string(&values[idx]));
            let fallback = fallback_index.and_then(|idx| value_to_string(&values[idx]));
            let normalized =
                normalize_compare_time(updated.clone(), deleted.clone(), fallback.clone());
            if let (Some(idx), Some(value)) = (updated_index, normalized.clone()) {
                values[idx] = Value::Text(value);
            }
            let compare = normalized.and_then(|value| parse_datetime_any(&value));
            map.insert(
                id,
                RowData {
                    values,
                    compare_time: compare,
                },
            );
        }
    }
    Ok(map)
}

pub(super) fn normalize_compare_time(
    updated: Option<String>,
    deleted: Option<String>,
    fallback: Option<String>,
) -> Option<String> {
    if let Some(u) = updated {
        if !u.is_empty() {
            return Some(u);
        }
    }
    if let Some(d) = deleted {
        if !d.is_empty() {
            return Some(d);
        }
    }
    fallback
}

pub(super) fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Text(text) => Some(text.clone()),
        Value::Integer(num) => Some(num.to_string()),
        Value::Real(num) => Some(num.to_string()),
        _ => None,
    }
}
