//! SQLite 数据库按行 LWW 合并能力（用于本地备份恢复、以及云端模式切回本地模式时的双向合并）。
//!
//! 旧版 WebDAV 文件同步已于 v3.0.0 正式移除，云端实时同步见 `crate::cloud`。

use std::collections::HashMap;

use chrono::NaiveDateTime;
use rusqlite::{params_from_iter, types::Value, Connection};

use crate::errors::AppError;
use crate::sync_schema::{
    adopt_remote_columns, ensure_sync_columns, quote_ident, table_columns, SyncTable, SYNC_TABLES,
};

mod merge;

pub(crate) use merge::merge_databases;

#[cfg(test)]
mod tests;
