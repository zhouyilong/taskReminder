//! 标签与项目的批量管理：重命名、合并与移除（v2.2.3）。
//!
//! 标签存在 `tasks.tags` 与 `recurring_tasks.tags`，项目存在 `tasks.project`，没有单独的表；
//! 这里逐行改写未删除的条目（含已完成的待办），只更新实际变化的行并写 `updated_at`，
//! 开启同步时作为普通修改传到其他设备。名称比较不区分大小写，与前端筛选一致。

use super::*;

/// 存放标签的同步表。
const TAG_TABLES: [&str; 2] = ["tasks", "recurring_tasks"];

fn single_tag(value: &str) -> Option<String> {
    crate::models::normalize_tags([value]).into_iter().next()
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl DbManager {
    /// 把标签 `old` 改名为 `new`；`new` 与条目上已有的标签相同即为合并（去重）。返回改动的条数。
    pub fn rename_tag(&self, old: &str, new: &str) -> Result<usize, AppError> {
        let new = single_tag(new).ok_or_else(|| AppError::Invalid("新标签名不能为空".into()))?;
        self.rewrite_tag(old, Some(&new))
    }

    /// 从所有条目上去掉标签 `name`。返回改动的条数。
    pub fn remove_tag(&self, name: &str) -> Result<usize, AppError> {
        self.rewrite_tag(name, None)
    }

    fn rewrite_tag(&self, old: &str, replacement: Option<&str>) -> Result<usize, AppError> {
        let old = single_tag(old).ok_or_else(|| AppError::Invalid("标签名不能为空".into()))?;
        let mut conn = self.get_conn()?;
        let tx = conn.transaction()?;
        let now = now_string();
        let mut changed = 0;
        for table in TAG_TABLES {
            let rows: Vec<(String, Option<String>)> = {
                let mut stmt = tx.prepare(&format!(
                    "SELECT id, tags FROM {table} WHERE deleted_at IS NULL AND tags IS NOT NULL AND tags != ''"
                ))?;
                let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
                rows.collect::<Result<_, _>>()?
            };
            for (id, raw) in rows {
                let before = tags_from_db(raw.clone());
                if !before.iter().any(|tag| same_name(tag, &old)) {
                    continue;
                }
                let after = crate::models::normalize_tags(before.iter().filter_map(|tag| {
                    if same_name(tag, &old) {
                        replacement
                    } else {
                        Some(tag.as_str())
                    }
                }));
                let value = tags_to_db(&after);
                if raw.as_deref() == Some(value.as_str()) {
                    continue;
                }
                changed += tx.execute(
                    &format!("UPDATE {table} SET tags = ?1, updated_at = ?2 WHERE id = ?3"),
                    params![value, now, id],
                )?;
            }
        }
        tx.commit()?;
        Ok(changed)
    }

    /// 把项目 `old` 中的待办移到项目 `new`（空字符串为移出到未分组）；
    /// `new` 已存在即为合并。返回改动的条数。
    pub fn rename_project(&self, old: &str, new: &str) -> Result<usize, AppError> {
        let old = normalize_project(old);
        if old.is_empty() {
            return Err(AppError::Invalid("项目名不能为空".into()));
        }
        let new = normalize_project(new);
        let mut conn = self.get_conn()?;
        let tx = conn.transaction()?;
        let now = now_string();
        let ids: Vec<(String, String)> = {
            let mut stmt = tx.prepare(
                "SELECT id, project FROM tasks WHERE deleted_at IS NULL AND project IS NOT NULL AND project != ''",
            )?;
            let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        let mut changed = 0;
        for (id, project) in ids {
            if !same_name(&project, &old) || project == new {
                continue;
            }
            changed += tx.execute(
                "UPDATE tasks SET project = ?1, updated_at = ?2 WHERE id = ?3",
                params![new, now, id],
            )?;
        }
        tx.commit()?;
        Ok(changed)
    }
}
