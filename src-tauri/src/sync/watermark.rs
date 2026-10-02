//! 同步水位：识别“长期未同步期间已在其他设备删除并清理”的行，避免把它们重新上传（复活）。
//!
//! 每次成功上传后记下：上传前的时间（水位）、远端身份，以及上传的快照中每一行的 id。
//! 下一次合并时，若是**同一个远端**且距水位已超过墓碑保留期，本机有、远端没有、
//! id 在上次上传的集合中、且之后没有在本机修改过的行，就是被其他设备删除后墓碑已过期清理的行：
//! 在本机物理删除，不再上传。
//!
//! 为什么要求超过保留期：在保留期内被删除的行，远端一定还留着墓碑，按普通合并即可；
//! 保留期内远端却少了行，只可能是远端被重置或用旧备份覆盖，这时不应删除本机数据。

use std::collections::HashSet;

use rusqlite::OptionalExtension;

use super::*;

/// 快照中的行：（表名，id）。
pub(super) type RowIds = Vec<(&'static str, String)>;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Watermark {
    pub uploaded_at: NaiveDateTime,
    pub remote: String,
}

impl Watermark {
    /// 是否可以据此判断远端缺少的行已被删除：同一远端，且距上次上传已超过墓碑保留期。
    pub fn applies(&self, remote: &str, now: NaiveDateTime) -> bool {
        self.remote == remote
            && now - self.uploaded_at >= chrono::Duration::days(TOMBSTONE_RETENTION_DAYS_SYNC)
    }
}

/// 远端身份：规范化的 WebDAV 地址 + 远端目录。更换服务器或目录后水位失效。
pub(super) fn remote_identity(settings: &AppSettings) -> String {
    build_base_url(&settings.webdav_url, &settings.webdav_root_path).to_lowercase()
}

fn open(db_path: &std::path::Path) -> Result<Connection, AppError> {
    let conn = Connection::open(db_path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

pub(super) fn load(db_path: &std::path::Path) -> Result<Option<Watermark>, AppError> {
    let conn = open(db_path)?;
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT uploaded_at, remote FROM sync_watermark WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(row.and_then(|(uploaded_at, remote)| {
        Some(Watermark {
            uploaded_at: parse_datetime_any(&uploaded_at)?,
            remote,
        })
    }))
}

/// 上传的快照中每张同步表的行 id。
pub(super) fn snapshot_ids(conn: &Connection) -> Result<RowIds, AppError> {
    let mut ids = Vec::new();
    for table in SYNC_TABLES {
        let mut stmt = conn.prepare(&format!("SELECT id FROM {}", quote_ident(table.name)))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for id in rows {
            ids.push((table.name, id?));
        }
    }
    Ok(ids)
}

/// 成功上传后保存水位（整体替换）。
pub(super) fn save(
    db_path: &std::path::Path,
    remote: &str,
    uploaded_at: NaiveDateTime,
    ids: &[(&'static str, String)],
) -> Result<(), AppError> {
    let mut conn = open(db_path)?;
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM sync_watermark_rows", [])?;
    {
        let mut stmt =
            tx.prepare("INSERT OR IGNORE INTO sync_watermark_rows (table_name, id) VALUES (?, ?)")?;
        for (table, id) in ids {
            stmt.execute(rusqlite::params![table, id])?;
        }
    }
    tx.execute(
        "INSERT OR REPLACE INTO sync_watermark (id, uploaded_at, remote) VALUES (1, ?, ?)",
        rusqlite::params![time::format_datetime(&uploaded_at), remote],
    )?;
    tx.commit()?;
    Ok(())
}

/// 候选行：id、updated_at、deleted_at、回退比较时间。
type Candidate = (String, Option<String>, Option<String>, Option<String>);

/// 删除本机中“已在其他设备删除并清理”的行（判断规则见模块说明），返回删除的行数。
pub(super) fn drop_rows_deleted_elsewhere(
    local: &Connection,
    remote: &Connection,
    watermark: &Watermark,
) -> Result<usize, AppError> {
    let mut dropped = 0;
    for table in SYNC_TABLES {
        let remote_ids: HashSet<String> = {
            let mut stmt =
                remote.prepare(&format!("SELECT id FROM {}", quote_ident(table.name)))?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<_, _>>()?
        };
        let candidates: Vec<Candidate> = {
            let mut stmt = local.prepare(&format!(
                "SELECT t.id, t.updated_at, t.deleted_at, t.{fallback}
                   FROM {table} t
                   JOIN sync_watermark_rows w ON w.table_name = ?1 AND w.id = t.id",
                table = quote_ident(table.name),
                fallback = quote_ident(table.fallback_time_column),
            ))?;
            let rows = stmt.query_map([table.name], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })?;
            rows.collect::<Result<_, _>>()?
        };
        for (id, updated, deleted, fallback) in candidates {
            if remote_ids.contains(&id) {
                continue;
            }
            let changed = normalize_compare_time(updated, deleted, fallback)
                .and_then(|value| parse_datetime_any(&value));
            // 水位之后在本机修改过的行照常保留并上传；时间无法解析时也保留。
            if changed.is_some_and(|time| time <= watermark.uploaded_at) {
                dropped += local.execute(
                    &format!("DELETE FROM {} WHERE id = ?", quote_ident(table.name)),
                    [&id],
                )?;
            }
        }
    }
    Ok(dropped)
}
