//! 本地快照的导出与两个数据库的按行合并。

use super::*;

pub(super) fn temp_db_path(kind: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("taskreminder-{}-{}.db", kind, uuid::Uuid::new_v4()))
}

/// 导出本地数据库快照并清空其中的敏感设置（WebDAV 密码、同步密码）；远端只用到数据表。
pub(super) fn export_local_snapshot_bytes(db_path: &std::path::Path) -> Result<Vec<u8>, AppError> {
    let snapshot = temp_db_path("snapshot");
    let result = (|| {
        {
            let conn = Connection::open(db_path)?;
            let escaped = snapshot.to_string_lossy().replace('\'', "''");
            conn.execute_batch(&format!("VACUUM INTO '{}'", escaped))?;
        }
        {
            let conn = Connection::open(&snapshot)?;
            conn.execute(
                "UPDATE settings SET webdav_password = '', sync_passphrase = ''",
                [],
            )?;
        }
        Ok(std::fs::read(&snapshot)?)
    })();
    let _ = std::fs::remove_file(&snapshot);
    result
}

pub(super) fn merge_snapshot_bytes(
    local_path: &std::path::Path,
    data: &[u8],
) -> Result<(), AppError> {
    let remote = temp_db_path("remote");
    let result = std::fs::write(&remote, data)
        .map_err(AppError::from)
        .and_then(|_| merge_databases(local_path, &remote));
    let _ = std::fs::remove_file(&remote);
    result
}

pub(crate) fn merge_databases(
    local_path: &std::path::Path,
    remote_path: &std::path::Path,
) -> Result<(), AppError> {
    let mut local = Connection::open(local_path)?;
    let remote = Connection::open(remote_path)?;
    ensure_sync_columns(&local)?;
    ensure_sync_columns(&remote)?;

    let tx = local.transaction()?;
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
