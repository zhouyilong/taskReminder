//! 两个 SQLite 数据库的按行 LWW 合并（含墓碑一致与未知列保留）。

use super::*;

pub(crate) fn merge_databases(
    local_path: &std::path::Path,
    remote_path: &std::path::Path,
) -> Result<(), AppError> {
    let mut local = Connection::open(local_path)?;
    let remote = Connection::open(remote_path)?;
    ensure_sync_columns(&local)?;
    // 源库不补列：合并只读它实际拥有的列，旧版本没有的列保留目标库的值。

    let tx = local.transaction()?;
    for table in SYNC_TABLES {
        merge_table(&tx, &remote, table)?;
    }
    tx.commit()?;
    Ok(())
}

/// 按行合并一张表，较新的一方胜出（见 `choose_row`）。
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

/// 插入或只更新给出的列：没有给出的列（源库没有的列）保留目标库原值。
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
            let compare = task_reminder_core::lww::effective_compare_time(
                updated.as_deref(),
                deleted.as_deref(),
                fallback.as_deref(),
            );
            if let (Some(idx), Some(dt)) = (updated_index, compare) {
                values[idx] = Value::Text(crate::time::format_datetime(&dt));
            }
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

pub(super) fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Text(text) => Some(text.clone()),
        Value::Integer(num) => Some(num.to_string()),
        Value::Real(num) => Some(num.to_string()),
        _ => None,
    }
}
