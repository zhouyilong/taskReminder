//! 参与同步的表与列：同步合并、旧库补列与“迁移与同步列一致”测试共用这一份定义。
//!
//! 新增同步列时：写迁移 → 在这里登记（`added`，定义与迁移一致）→ `cargo test` 会检查两者一致。
//! 新增的同步列必须可空或带 `DEFAULT`，否则旧版本合并时无法补上该列；同步表里也不要再加
//! 只在本机使用的列（放到 `settings` 或单独的表），否则旧版本会把它当作不认识的列同步出去。

use rusqlite::Connection;

use crate::errors::AppError;

#[derive(Debug, Clone, Copy)]
pub(crate) struct SyncColumn {
    pub name: &'static str,
    /// 列定义，与迁移中的写法一致（类型、`NOT NULL`、`DEFAULT`）。
    pub definition: &'static str,
    /// `true` 表示后来经 `ALTER TABLE ADD COLUMN` 加入：合并较旧的库前会先补上。
    pub added: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SyncTable {
    pub name: &'static str,
    /// 行没有 `updated_at` / `deleted_at` 时用来比较新旧的列。
    pub fallback_time_column: &'static str,
    pub columns: &'static [SyncColumn],
    /// 同表中只在本机使用、不参与同步的列。
    pub local_only: &'static [&'static str],
}

const fn initial(name: &'static str, definition: &'static str) -> SyncColumn {
    SyncColumn {
        name,
        definition,
        added: false,
    }
}

const fn added(name: &'static str, definition: &'static str) -> SyncColumn {
    SyncColumn {
        name,
        definition,
        added: true,
    }
}

pub(crate) const SYNC_TABLES: &[SyncTable] = &[
    SyncTable {
        name: "tasks",
        fallback_time_column: "created_at",
        columns: &[
            initial("id", "TEXT PRIMARY KEY"),
            initial("description", "TEXT NOT NULL"),
            initial("type", "TEXT NOT NULL"),
            initial("status", "TEXT NOT NULL"),
            initial("created_at", "TEXT NOT NULL"),
            initial("completed_at", "TEXT"),
            initial("reminder_time", "TEXT"),
            added("updated_at", "TEXT"),
            added("deleted_at", "TEXT"),
            added("sticky_content", "TEXT NOT NULL DEFAULT ''"),
            added("sticky_pos_x", "REAL NOT NULL DEFAULT 48"),
            added("sticky_pos_y", "REAL NOT NULL DEFAULT 76"),
            added("sticky_is_open", "INTEGER NOT NULL DEFAULT 0"),
            added("sticky_width", "REAL NOT NULL DEFAULT 284"),
            added("sticky_height", "REAL NOT NULL DEFAULT 280"),
            added("tags", "TEXT NOT NULL DEFAULT ''"),
            added("priority", "INTEGER NOT NULL DEFAULT 0"),
            added("due_at", "TEXT"),
            added("sticky_color", "TEXT NOT NULL DEFAULT ''"),
            added("sort_order", "REAL"),
        ],
        local_only: &["sticky_is_pinned"],
    },
    SyncTable {
        name: "recurring_tasks",
        fallback_time_column: "created_at",
        columns: &[
            initial("id", "TEXT PRIMARY KEY"),
            initial("description", "TEXT NOT NULL"),
            initial("type", "TEXT NOT NULL"),
            initial("status", "TEXT NOT NULL"),
            initial("created_at", "TEXT NOT NULL"),
            initial("completed_at", "TEXT"),
            initial("interval_minutes", "INTEGER NOT NULL"),
            initial("last_triggered", "TEXT"),
            initial("next_trigger", "TEXT NOT NULL"),
            initial("is_paused", "INTEGER NOT NULL DEFAULT 0"),
            initial("start_time", "TEXT"),
            initial("end_time", "TEXT"),
            added("updated_at", "TEXT"),
            added("deleted_at", "TEXT"),
            added("repeat_mode", "TEXT NOT NULL DEFAULT 'INTERVAL_RANGE'"),
            added("schedule_time", "TEXT"),
            added("schedule_weekday", "INTEGER"),
            added("schedule_day", "INTEGER"),
            added("cron_expression", "TEXT"),
            added("schedule_weekdays", "INTEGER"),
            added("tags", "TEXT NOT NULL DEFAULT ''"),
        ],
        local_only: &[],
    },
    SyncTable {
        name: "reminder_records",
        fallback_time_column: "trigger_time",
        columns: &[
            initial("id", "TEXT PRIMARY KEY"),
            initial("reminder_id", "TEXT NOT NULL"),
            initial("description", "TEXT NOT NULL"),
            initial("type", "TEXT NOT NULL"),
            initial("trigger_time", "TEXT NOT NULL"),
            initial("close_time", "TEXT"),
            initial("action", "TEXT NOT NULL"),
            added("updated_at", "TEXT"),
            added("deleted_at", "TEXT"),
        ],
        local_only: &[],
    },
];

impl SyncTable {
    fn is_known(&self, column: &str) -> bool {
        self.columns
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(column))
    }

    fn is_local_only(&self, column: &str) -> bool {
        self.local_only
            .iter()
            .any(|c| c.eq_ignore_ascii_case(column))
    }
}

/// `PRAGMA table_info` 中的一列。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ColumnInfo {
    pub name: String,
    pub decl_type: String,
    pub not_null: bool,
    pub default: Option<String>,
}

pub(crate) fn table_columns(conn: &Connection, table: &str) -> Result<Vec<ColumnInfo>, AppError> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", quote_ident(table)))?;
    let rows = stmt.query_map([], |row| {
        Ok(ColumnInfo {
            name: row.get("name")?,
            decl_type: row.get::<_, Option<String>>("type")?.unwrap_or_default(),
            not_null: row.get::<_, i64>("notnull")? != 0,
            default: row.get("dflt_value")?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn has_column(columns: &[ColumnInfo], name: &str) -> bool {
    columns.iter().any(|c| c.name.eq_ignore_ascii_case(name))
}

pub(crate) fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// 给较旧的库补上后来加入的同步列（合并前对本机库与远端库都执行）。
pub(crate) fn ensure_sync_columns(conn: &Connection) -> Result<(), AppError> {
    for table in SYNC_TABLES {
        let existing = table_columns(conn, table.name)?;
        for column in table.columns.iter().filter(|c| c.added) {
            if !has_column(&existing, column.name) {
                conn.execute(
                    &format!(
                        "ALTER TABLE {} ADD COLUMN {} {}",
                        table.name, column.name, column.definition
                    ),
                    [],
                )?;
            }
        }
    }
    Ok(())
}

/// 确定一张表参与合并的列，并把远端有、本机没有的**不认识的列**（更新版本新增的同步列）补到本机，
/// 让它们的值随合并保存下来、再随上传带回，而不是被丢掉。
///
/// 返回参与合并的列（本机表中除本机专用列以外的所有列，含之前补上的不认识的列）与本次新补上的列。
/// 无法安全补上的列（`NOT NULL` 却没有默认值、名称或定义不是简单写法）跳过，只记日志，不影响同步。
pub(crate) fn adopt_remote_columns(
    local: &Connection,
    remote: &Connection,
    table: &SyncTable,
) -> Result<AdoptedColumns, AppError> {
    let mut local_columns = table_columns(local, table.name)?;
    let mut newly_added = Vec::new();
    for column in table_columns(remote, table.name)? {
        if table.is_known(&column.name)
            || table.is_local_only(&column.name)
            || has_column(&local_columns, &column.name)
        {
            continue;
        }
        match add_column_sql(table.name, &column) {
            Some(sql) => match local.execute(&sql, []) {
                Ok(_) => {
                    newly_added.push(column.name.clone());
                    local_columns.push(column);
                }
                Err(err) => eprintln!(
                    "[sync] 无法保留远端新增的列 {}.{}: {}",
                    table.name, column.name, err
                ),
            },
            None => eprintln!(
                "[sync] 跳过远端新增的列 {}.{}：定义无法安全补上",
                table.name, column.name
            ),
        }
    }
    Ok(AdoptedColumns {
        columns: local_columns
            .into_iter()
            .map(|c| c.name)
            .filter(|name| !table.is_local_only(name))
            .collect(),
        newly_added,
    })
}

pub(crate) struct AdoptedColumns {
    /// 参与合并的列。
    pub columns: Vec<String>,
    /// 本次合并才补到本机的列：本机的行还没有这些列的值。
    pub newly_added: Vec<String>,
}

/// 按远端表的定义生成补列语句。列名、类型与默认值来自远端文件，只接受简单写法，避免拼进任意 SQL。
fn add_column_sql(table: &str, column: &ColumnInfo) -> Option<String> {
    if !is_simple_ident(&column.name) || !is_simple_type(&column.decl_type) {
        return None;
    }
    let default = match column.default.as_deref() {
        Some(value) if is_literal(value) => Some(value),
        Some(_) => return None,
        None => None,
    };
    if column.not_null && default.is_none() {
        return None;
    }
    let mut sql = format!(
        "ALTER TABLE {} ADD COLUMN {}",
        quote_ident(table),
        quote_ident(&column.name)
    );
    if !column.decl_type.is_empty() {
        sql.push(' ');
        sql.push_str(&column.decl_type);
    }
    if column.not_null {
        sql.push_str(" NOT NULL");
    }
    if let Some(value) = default {
        sql.push_str(" DEFAULT ");
        sql.push_str(value);
    }
    Some(sql)
}

fn is_simple_ident(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit())
}

/// 类型名：如 `TEXT`、`INTEGER`、`VARCHAR(24)`、`DOUBLE PRECISION`。
fn is_simple_type(decl: &str) -> bool {
    decl.len() <= 64
        && decl
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '_' | '(' | ')' | ','))
}

/// 默认值只接受字面量：`NULL`、数字、不含换行的单引号字符串。
fn is_literal(value: &str) -> bool {
    let value = value.trim();
    if value.eq_ignore_ascii_case("NULL") {
        return true;
    }
    if value.parse::<f64>().is_ok() && !value.contains(|c: char| c.is_ascii_alphabetic()) {
        return true;
    }
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        let inner = &value[1..value.len() - 1];
        return !inner.contains(['\n', '\r', '\0']) && !inner.replace("''", "").contains('\'');
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DbManager;

    fn migrated_db() -> (DbManager, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("taskreminder-schema-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        (DbManager::new(dir.join("test.db")).unwrap(), dir)
    }

    /// 解析列定义中的类型、`NOT NULL` 与 `DEFAULT`，与 `PRAGMA table_info` 对照。
    fn expected(definition: &str) -> (String, bool, Option<String>) {
        let upper = definition.to_ascii_uppercase();
        let decl_type = definition.split_whitespace().next().unwrap().to_string();
        let default = upper
            .find("DEFAULT ")
            .map(|i| definition[i + "DEFAULT ".len()..].trim().to_string());
        (decl_type, upper.contains("NOT NULL"), default)
    }

    #[test]
    fn migrations_create_every_sync_column_as_declared() {
        let (db, dir) = migrated_db();
        let conn = Connection::open(db.db_path()).unwrap();
        for table in SYNC_TABLES {
            let actual = table_columns(&conn, table.name).unwrap();
            for column in table.columns {
                let info = actual
                    .iter()
                    .find(|c| c.name == column.name)
                    .unwrap_or_else(|| panic!("迁移后缺少同步列 {}.{}", table.name, column.name));
                let (decl_type, not_null, default) = expected(column.definition);
                assert_eq!(info.decl_type, decl_type, "{}.{}", table.name, column.name);
                assert_eq!(info.not_null, not_null, "{}.{}", table.name, column.name);
                assert_eq!(info.default, default, "{}.{}", table.name, column.name);
            }
            // 同步表中的每一列都必须登记：要么参与同步，要么明确是本机专用。
            for info in &actual {
                assert!(
                    table.is_known(&info.name) || table.is_local_only(&info.name),
                    "{}.{} 没有登记在 SYNC_TABLES 中",
                    table.name,
                    info.name
                );
            }
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn added_columns_are_addable_to_old_databases() {
        for table in SYNC_TABLES {
            for column in table.columns.iter().filter(|c| c.added) {
                let (_, not_null, default) = expected(column.definition);
                assert!(
                    !not_null || default.is_some(),
                    "{}.{} 是 NOT NULL 却没有默认值，旧库无法补列",
                    table.name,
                    column.name
                );
            }
        }
    }

    #[test]
    fn ensure_sync_columns_upgrades_initial_schema() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/V1.1.0__initial_schema.sql"))
            .unwrap();
        ensure_sync_columns(&conn).unwrap();
        ensure_sync_columns(&conn).unwrap();
        for table in SYNC_TABLES {
            let actual = table_columns(&conn, table.name).unwrap();
            for column in table.columns {
                assert!(has_column(&actual, column.name), "{}", column.name);
            }
        }
    }

    fn column(name: &str, decl: &str, not_null: bool, default: Option<&str>) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            decl_type: decl.to_string(),
            not_null,
            default: default.map(str::to_string),
        }
    }

    #[test]
    fn add_column_sql_accepts_only_simple_definitions() {
        assert_eq!(
            add_column_sql("tasks", &column("due_at", "TEXT", false, None)).as_deref(),
            Some("ALTER TABLE \"tasks\" ADD COLUMN \"due_at\" TEXT")
        );
        assert_eq!(
            add_column_sql("tasks", &column("color", "TEXT", true, Some("'it''s'"))).as_deref(),
            Some("ALTER TABLE \"tasks\" ADD COLUMN \"color\" TEXT NOT NULL DEFAULT 'it''s'")
        );
        assert!(add_column_sql("tasks", &column("lead", "INTEGER", true, Some("-10"))).is_some());
        // NOT NULL 却没有默认值：旧行无法取值。
        assert!(add_column_sql("tasks", &column("x", "TEXT", true, None)).is_none());
        // 非字面量默认值、可疑的名称与类型。
        assert!(add_column_sql(
            "tasks",
            &column("x", "TEXT", false, Some("CURRENT_TIMESTAMP"))
        )
        .is_none());
        assert!(add_column_sql("tasks", &column("x", "TEXT", false, Some("'a' || 'b'"))).is_none());
        assert!(add_column_sql("tasks", &column("x\"; DROP", "TEXT", false, None)).is_none());
        assert!(
            add_column_sql("tasks", &column("x", "TEXT; DROP TABLE tasks", false, None)).is_none()
        );
    }
}
