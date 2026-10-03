//! 数据库迁移：按版本执行 `migrations/` 下的脚本。

use super::*;

impl DbManager {
    pub(super) fn ensure_version_table(&self, conn: &Connection) -> Result<(), AppError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                version TEXT NOT NULL,
                applied_at TEXT NOT NULL,
                description TEXT
            );",
        )?;

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))
            .unwrap_or(0);

        if count == 0 {
            conn.execute(
                "INSERT INTO schema_version (id, version, applied_at, description) VALUES (1, '0.0.0', ?, 'Initial')",
                [now_string()],
            )?;
        }
        Ok(())
    }

    pub(super) fn apply_migrations(&self, conn: &mut Connection) -> Result<(), AppError> {
        let current = self.get_current_version(conn)?;
        let scripts = migration_scripts();

        for script in scripts {
            if compare_version(&script.version, &current) == std::cmp::Ordering::Greater {
                let tx = conn.transaction()?;
                execute_sql_script(&tx, script.sql)?;
                tx.execute(
                    "UPDATE schema_version SET version = ?, applied_at = ?, description = ? WHERE id = 1",
                    params![script.version, now_string(), script.description],
                )?;
                tx.commit()?;
            }
        }
        Ok(())
    }

    pub(super) fn get_current_version(&self, conn: &Connection) -> Result<String, AppError> {
        let version: Option<String> = conn
            .query_row(
                "SELECT version FROM schema_version WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(version.unwrap_or_else(|| "0.0.0".to_string()))
    }
}

pub(super) struct MigrationScript {
    version: String,
    description: String,
    sql: &'static str,
}

pub(super) fn migration_scripts() -> Vec<MigrationScript> {
    vec![
        MigrationScript {
            version: "1.1.0".to_string(),
            description: "initial schema".to_string(),
            sql: include_str!("../../migrations/V1.1.0__initial_schema.sql"),
        },
        MigrationScript {
            version: "1.2.0".to_string(),
            description: "add webdav settings".to_string(),
            sql: include_str!("../../migrations/V1.2.0__add_webdav_settings.sql"),
        },
        MigrationScript {
            version: "1.3.0".to_string(),
            description: "add sync metadata".to_string(),
            sql: include_str!("../../migrations/V1.3.0__add_sync_metadata.sql"),
        },
        MigrationScript {
            version: "1.4.0".to_string(),
            description: "add notification theme".to_string(),
            sql: include_str!("../../migrations/V1.4.0__add_notification_theme.sql"),
        },
        MigrationScript {
            version: "1.4.1".to_string(),
            description: "add recurring modes".to_string(),
            sql: include_str!("../../migrations/V1.4.1__add_recurring_modes.sql"),
        },
        MigrationScript {
            version: "1.4.2".to_string(),
            description: "add sticky note settings".to_string(),
            sql: include_str!("../../migrations/V1.4.2__add_sticky_note_settings.sql"),
        },
        MigrationScript {
            version: "1.4.3".to_string(),
            description: "add sticky notes table".to_string(),
            sql: include_str!("../../migrations/V1.4.3__add_sticky_notes_table.sql"),
        },
        MigrationScript {
            version: "1.4.4".to_string(),
            description: "add sticky note title and type".to_string(),
            sql: include_str!("../../migrations/V1.4.4__add_sticky_note_meta.sql"),
        },
        MigrationScript {
            version: "1.4.5".to_string(),
            description: "merge sticky notes into tasks".to_string(),
            sql: include_str!("../../migrations/V1.4.5__merge_sticky_notes_into_tasks.sql"),
        },
        MigrationScript {
            version: "1.4.6".to_string(),
            description: "add sticky note opacity".to_string(),
            sql: include_str!("../../migrations/V1.4.6__add_sticky_note_opacity.sql"),
        },
        MigrationScript {
            version: "1.4.7".to_string(),
            description: "add sticky note item size".to_string(),
            sql: include_str!("../../migrations/V1.4.7__add_sticky_note_item_size.sql"),
        },
        MigrationScript {
            version: "1.4.8".to_string(),
            description: "add window opacity".to_string(),
            sql: include_str!("../../migrations/V1.4.8__add_window_opacity.sql"),
        },
        MigrationScript {
            version: "1.4.9".to_string(),
            description: "add sticky note pin state".to_string(),
            sql: include_str!("../../migrations/V1.4.9__add_sticky_note_pin_state.sql"),
        },
        MigrationScript {
            version: "1.6.0".to_string(),
            description: "add weekday mask and quick add shortcut".to_string(),
            sql: include_str!("../../migrations/V1.6.0__add_weekday_mask_and_quick_add.sql"),
        },
        MigrationScript {
            version: "2.0.0".to_string(),
            description: "add task tags and priority".to_string(),
            sql: include_str!("../../migrations/V2.0.0__add_task_tags_and_priority.sql"),
        },
        MigrationScript {
            version: "2.0.1".to_string(),
            description: "add sync encryption".to_string(),
            sql: include_str!("../../migrations/V2.0.1__add_sync_encryption.sql"),
        },
        MigrationScript {
            version: "2.0.2".to_string(),
            description: "add quiet hours, native notification and sticky toggle shortcut"
                .to_string(),
            sql: include_str!(
                "../../migrations/V2.0.2__add_quiet_hours_notification_and_sticky_shortcut.sql"
            ),
        },
        MigrationScript {
            version: "2.0.3".to_string(),
            description: "add secret storage".to_string(),
            sql: include_str!("../../migrations/V2.0.3__add_secret_storage.sql"),
        },
        MigrationScript {
            version: "2.0.4".to_string(),
            description: "add sticky snap".to_string(),
            sql: include_str!("../../migrations/V2.0.4__add_sticky_snap.sql"),
        },
        MigrationScript {
            version: "2.0.5".to_string(),
            description: "add holiday auto update".to_string(),
            sql: include_str!("../../migrations/V2.0.5__add_holiday_auto_update.sql"),
        },
        MigrationScript {
            version: "2.0.6".to_string(),
            description: "add sync watermark".to_string(),
            sql: include_str!("../../migrations/V2.0.6__add_sync_watermark.sql"),
        },
        MigrationScript {
            version: "2.1.0".to_string(),
            description: "add due time, sticky color, sort order and recurring tags".to_string(),
            sql: include_str!("../../migrations/V2.1.0__add_due_color_order_recurring_tags.sql"),
        },
        MigrationScript {
            version: "2.1.1".to_string(),
            description: "add completed retention".to_string(),
            sql: include_str!("../../migrations/V2.1.1__add_completed_retention.sql"),
        },
    ]
}

pub(super) fn compare_version(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |v: &str| -> Vec<i32> {
        v.split('.')
            .map(|part| part.parse::<i32>().unwrap_or(0))
            .collect()
    };
    let av = parse(a);
    let bv = parse(b);
    for i in 0..3 {
        let left = *av.get(i).unwrap_or(&0);
        let right = *bv.get(i).unwrap_or(&0);
        if left != right {
            return left.cmp(&right);
        }
    }
    std::cmp::Ordering::Equal
}

/// 测试用：建出某个版本发布时的库（只执行版本号不超过 `version` 的迁移，并记下库结构版本），
/// 之后用 `DbManager::new` 打开即可模拟从该版本升级。
#[cfg(test)]
pub(crate) fn create_schema_up_to(conn: &Connection, version: &str) -> Result<(), AppError> {
    for script in migration_scripts() {
        if compare_version(&script.version, version) != std::cmp::Ordering::Greater {
            execute_sql_script(conn, script.sql)?;
        }
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            version TEXT NOT NULL,
            applied_at TEXT NOT NULL,
            description TEXT
        );",
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO schema_version (id, version, applied_at, description)
         VALUES (1, ?, '2026-09-01T00:00:00', 'test fixture')",
        [version],
    )?;
    Ok(())
}

pub(super) fn execute_sql_script(conn: &Connection, sql: &str) -> Result<(), AppError> {
    let mut cleaned = String::new();
    for line in sql.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }
        cleaned.push_str(line);
        cleaned.push('\n');
    }

    for statement in cleaned.split(';') {
        let trimmed = statement.trim();
        if trimmed.is_empty() || adds_existing_column(conn, trimmed)? {
            continue;
        }
        conn.execute_batch(trimmed)?;
    }
    Ok(())
}

/// `ALTER TABLE t ADD [COLUMN] c ...` 且该列已存在时返回 `true`：同步会把更新版本新增的同步列
/// 提前补到本机（`sync_schema::adopt_remote_columns`），升级到那个版本时对应迁移要跳过补列，
/// 而不是报“duplicate column name”。
pub(super) fn adds_existing_column(conn: &Connection, statement: &str) -> Result<bool, AppError> {
    let tokens: Vec<&str> = statement.split_whitespace().take(6).collect();
    let keyword =
        |i: usize, word: &str| tokens.get(i).is_some_and(|t| t.eq_ignore_ascii_case(word));
    if !(keyword(0, "ALTER") && keyword(1, "TABLE") && keyword(3, "ADD")) {
        return Ok(false);
    }
    let column_index = if keyword(4, "COLUMN") { 5 } else { 4 };
    let (Some(table), Some(column)) = (tokens.get(2), tokens.get(column_index)) else {
        return Ok(false);
    };
    let unquote = |name: &str| {
        name.trim_matches(|c| c == '"' || c == '`' || c == '[' || c == ']')
            .to_string()
    };
    let existing = crate::sync_schema::table_columns(conn, &unquote(table))?;
    let column = unquote(column);
    Ok(existing
        .iter()
        .any(|c| c.name.eq_ignore_ascii_case(&column)))
}
