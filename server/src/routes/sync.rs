//! 增量同步（Pull / Push）、模式切换全量合并（Merge）与 SSE 实时事件推送。

use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::time::Duration;

use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures_core::Stream;
use serde::Deserialize;
use sqlx::{MySql, Row, Transaction};
use task_reminder_core::kinds::{ReminderAction, ReminderKind, RepeatMode, TaskStatus, TaskType};
use task_reminder_core::lww::is_incoming_newer;
use task_reminder_core::protocol::{
    SyncChangeSet, SyncEventPayload, SyncMergeRequest, SyncMergeResponse, SyncMergeSummary,
    SyncPullResponse, SyncPushRequest, SyncPushResponse, SyncRecurringTaskRow,
    SyncReminderRecordRow, SyncTaskRow,
};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::auth::AuthUser;
use crate::error::{ServerError, ServerResult};
use crate::state::AppState;

const DEFAULT_PULL_LIMIT: i64 = 500;
const MAX_PULL_LIMIT: i64 = 2000;

#[derive(Deserialize)]
pub struct PullQuery {
    #[serde(default)]
    pub since: i64,
    #[serde(default)]
    pub limit: Option<i64>,
}

fn task_from_row(row: &sqlx::mysql::MySqlRow) -> Result<(SyncTaskRow, i64), sqlx::Error> {
    let type_str: String = row.try_get("type")?;
    let status_str: String = row.try_get("status")?;
    let sticky_is_open: i8 = row.try_get("sticky_is_open")?;
    let urgent: i8 = row.try_get("urgent")?;
    let priority: i32 = row.try_get("priority")?;
    let server_seq: i64 = row.try_get("server_seq")?;
    Ok((
        SyncTaskRow {
            id: row.try_get("id")?,
            description: row.try_get("description")?,
            task_type: TaskType::parse(&type_str),
            status: TaskStatus::parse(&status_str),
            created_at: row.try_get("created_at")?,
            completed_at: row.try_get("completed_at")?,
            reminder_time: row.try_get("reminder_time")?,
            updated_at: row.try_get("updated_at")?,
            deleted_at: row.try_get("deleted_at")?,
            sticky_content: row.try_get("sticky_content")?,
            sticky_pos_x: row.try_get("sticky_pos_x")?,
            sticky_pos_y: row.try_get("sticky_pos_y")?,
            sticky_is_open: sticky_is_open != 0,
            sticky_width: row.try_get("sticky_width")?,
            sticky_height: row.try_get("sticky_height")?,
            tags: row.try_get("tags")?,
            priority: i64::from(priority),
            due_at: row.try_get("due_at")?,
            sticky_color: row.try_get("sticky_color")?,
            sort_order: row.try_get("sort_order")?,
            project: row.try_get("project")?,
            urgent: urgent != 0,
        },
        server_seq,
    ))
}

fn recurring_from_row(
    row: &sqlx::mysql::MySqlRow,
) -> Result<(SyncRecurringTaskRow, i64), sqlx::Error> {
    let type_str: String = row.try_get("type")?;
    let status_str: String = row.try_get("status")?;
    let repeat_mode_str: String = row.try_get("repeat_mode")?;
    let interval_minutes: i32 = row.try_get("interval_minutes")?;
    let is_paused: i8 = row.try_get("is_paused")?;
    let schedule_weekday: Option<i32> = row.try_get("schedule_weekday")?;
    let schedule_day: Option<i32> = row.try_get("schedule_day")?;
    let schedule_weekdays: Option<i32> = row.try_get("schedule_weekdays")?;
    let remaining_count: Option<i32> = row.try_get("remaining_count")?;
    let server_seq: i64 = row.try_get("server_seq")?;
    Ok((
        SyncRecurringTaskRow {
            id: row.try_get("id")?,
            description: row.try_get("description")?,
            task_type: TaskType::parse(&type_str),
            status: TaskStatus::parse(&status_str),
            created_at: row.try_get("created_at")?,
            completed_at: row.try_get("completed_at")?,
            interval_minutes: i64::from(interval_minutes),
            last_triggered: row.try_get("last_triggered")?,
            next_trigger: row.try_get("next_trigger")?,
            is_paused: is_paused != 0,
            start_time: row.try_get("start_time")?,
            end_time: row.try_get("end_time")?,
            updated_at: row.try_get("updated_at")?,
            deleted_at: row.try_get("deleted_at")?,
            repeat_mode: RepeatMode::parse(&repeat_mode_str),
            schedule_time: row.try_get("schedule_time")?,
            schedule_weekday: schedule_weekday.map(i64::from),
            schedule_day: schedule_day.map(i64::from),
            cron_expression: row.try_get("cron_expression")?,
            schedule_weekdays: schedule_weekdays.map(i64::from),
            tags: row.try_get("tags")?,
            ends_on: row.try_get("ends_on")?,
            remaining_count: remaining_count.map(i64::from),
            temp_trigger: row.try_get("temp_trigger")?,
            temp_replaces: row.try_get("temp_replaces")?,
        },
        server_seq,
    ))
}

fn record_from_row(
    row: &sqlx::mysql::MySqlRow,
) -> Result<(SyncReminderRecordRow, i64), sqlx::Error> {
    let type_str: String = row.try_get("type")?;
    let action_str: String = row.try_get("action")?;
    let server_seq: i64 = row.try_get("server_seq")?;
    Ok((
        SyncReminderRecordRow {
            id: row.try_get("id")?,
            reminder_id: row.try_get("reminder_id")?,
            description: row.try_get("description")?,
            reminder_type: ReminderKind::parse(&type_str),
            trigger_time: row.try_get("trigger_time")?,
            close_time: row.try_get("close_time")?,
            action: ReminderAction::parse(&action_str),
            updated_at: row.try_get("updated_at")?,
            deleted_at: row.try_get("deleted_at")?,
        },
        server_seq,
    ))
}

const SELECT_TASK_COLS: &str =
    "id, description, type, status, created_at, completed_at, reminder_time,
    updated_at, deleted_at, sticky_content, sticky_pos_x, sticky_pos_y, sticky_is_open,
    sticky_width, sticky_height, tags, priority, due_at, sticky_color, sort_order,
    project, urgent, server_seq";

const SELECT_RECURRING_COLS: &str = "id, description, type, status, created_at, completed_at,
    interval_minutes, last_triggered, next_trigger, is_paused, start_time, end_time,
    updated_at, deleted_at, repeat_mode, schedule_time, schedule_weekday, schedule_day,
    cron_expression, schedule_weekdays, tags, ends_on, remaining_count, temp_trigger,
    temp_replaces, server_seq";

const SELECT_RECORD_COLS: &str = "id, reminder_id, description, type, trigger_time, close_time,
    action, updated_at, deleted_at, server_seq";

async fn upsert_task(
    tx: &mut Transaction<'_, MySql>,
    user_id: &str,
    row: &SyncTaskRow,
    seq: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO tasks (
            user_id, id, description, type, status, created_at, completed_at, reminder_time,
            updated_at, deleted_at, sticky_content, sticky_pos_x, sticky_pos_y, sticky_is_open,
            sticky_width, sticky_height, tags, priority, due_at, sticky_color, sort_order,
            project, urgent, server_seq
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?
        ) ON DUPLICATE KEY UPDATE
            description = VALUES(description),
            type = VALUES(type),
            status = VALUES(status),
            created_at = VALUES(created_at),
            completed_at = VALUES(completed_at),
            reminder_time = VALUES(reminder_time),
            updated_at = VALUES(updated_at),
            deleted_at = VALUES(deleted_at),
            sticky_content = VALUES(sticky_content),
            sticky_pos_x = VALUES(sticky_pos_x),
            sticky_pos_y = VALUES(sticky_pos_y),
            sticky_is_open = VALUES(sticky_is_open),
            sticky_width = VALUES(sticky_width),
            sticky_height = VALUES(sticky_height),
            tags = VALUES(tags),
            priority = VALUES(priority),
            due_at = VALUES(due_at),
            sticky_color = VALUES(sticky_color),
            sort_order = VALUES(sort_order),
            project = VALUES(project),
            urgent = VALUES(urgent),
            server_seq = VALUES(server_seq)",
    )
    .bind(user_id)
    .bind(&row.id)
    .bind(&row.description)
    .bind(row.task_type.as_str())
    .bind(row.status.as_str())
    .bind(&row.created_at)
    .bind(&row.completed_at)
    .bind(&row.reminder_time)
    .bind(&row.updated_at)
    .bind(&row.deleted_at)
    .bind(&row.sticky_content)
    .bind(row.sticky_pos_x)
    .bind(row.sticky_pos_y)
    .bind(if row.sticky_is_open { 1i8 } else { 0i8 })
    .bind(row.sticky_width)
    .bind(row.sticky_height)
    .bind(&row.tags)
    .bind(row.priority as i32)
    .bind(&row.due_at)
    .bind(&row.sticky_color)
    .bind(row.sort_order)
    .bind(&row.project)
    .bind(if row.urgent { 1i8 } else { 0i8 })
    .bind(seq)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_recurring(
    tx: &mut Transaction<'_, MySql>,
    user_id: &str,
    row: &SyncRecurringTaskRow,
    seq: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO recurring_tasks (
            user_id, id, description, type, status, created_at, completed_at,
            interval_minutes, last_triggered, next_trigger, is_paused, start_time, end_time,
            updated_at, deleted_at, repeat_mode, schedule_time, schedule_weekday, schedule_day,
            cron_expression, schedule_weekdays, tags, ends_on, remaining_count, temp_trigger,
            temp_replaces, server_seq
        ) VALUES (
            ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?,
            ?, ?
        ) ON DUPLICATE KEY UPDATE
            description = VALUES(description),
            type = VALUES(type),
            status = VALUES(status),
            created_at = VALUES(created_at),
            completed_at = VALUES(completed_at),
            interval_minutes = VALUES(interval_minutes),
            last_triggered = VALUES(last_triggered),
            next_trigger = VALUES(next_trigger),
            is_paused = VALUES(is_paused),
            start_time = VALUES(start_time),
            end_time = VALUES(end_time),
            updated_at = VALUES(updated_at),
            deleted_at = VALUES(deleted_at),
            repeat_mode = VALUES(repeat_mode),
            schedule_time = VALUES(schedule_time),
            schedule_weekday = VALUES(schedule_weekday),
            schedule_day = VALUES(schedule_day),
            cron_expression = VALUES(cron_expression),
            schedule_weekdays = VALUES(schedule_weekdays),
            tags = VALUES(tags),
            ends_on = VALUES(ends_on),
            remaining_count = VALUES(remaining_count),
            temp_trigger = VALUES(temp_trigger),
            temp_replaces = VALUES(temp_replaces),
            server_seq = VALUES(server_seq)",
    )
    .bind(user_id)
    .bind(&row.id)
    .bind(&row.description)
    .bind(row.task_type.as_str())
    .bind(row.status.as_str())
    .bind(&row.created_at)
    .bind(&row.completed_at)
    .bind(row.interval_minutes as i32)
    .bind(&row.last_triggered)
    .bind(&row.next_trigger)
    .bind(if row.is_paused { 1i8 } else { 0i8 })
    .bind(&row.start_time)
    .bind(&row.end_time)
    .bind(&row.updated_at)
    .bind(&row.deleted_at)
    .bind(row.repeat_mode.as_str())
    .bind(&row.schedule_time)
    .bind(row.schedule_weekday.map(|v| v as i32))
    .bind(row.schedule_day.map(|v| v as i32))
    .bind(&row.cron_expression)
    .bind(row.schedule_weekdays.map(|v| v as i32))
    .bind(&row.tags)
    .bind(&row.ends_on)
    .bind(row.remaining_count.map(|v| v as i32))
    .bind(&row.temp_trigger)
    .bind(&row.temp_replaces)
    .bind(seq)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_record(
    tx: &mut Transaction<'_, MySql>,
    user_id: &str,
    row: &SyncReminderRecordRow,
    seq: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO reminder_records (
            user_id, id, reminder_id, description, type, trigger_time, close_time,
            action, updated_at, deleted_at, server_seq
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            reminder_id = VALUES(reminder_id),
            description = VALUES(description),
            type = VALUES(type),
            trigger_time = VALUES(trigger_time),
            close_time = VALUES(close_time),
            action = VALUES(action),
            updated_at = VALUES(updated_at),
            deleted_at = VALUES(deleted_at),
            server_seq = VALUES(server_seq)",
    )
    .bind(user_id)
    .bind(&row.id)
    .bind(&row.reminder_id)
    .bind(&row.description)
    .bind(row.reminder_type.as_str())
    .bind(&row.trigger_time)
    .bind(&row.close_time)
    .bind(row.action.as_str())
    .bind(&row.updated_at)
    .bind(&row.deleted_at)
    .bind(seq)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn pull(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(query): Query<PullQuery>,
) -> ServerResult<Json<SyncPullResponse>> {
    let since = query.since.max(0);
    let limit = query
        .limit
        .unwrap_or(DEFAULT_PULL_LIMIT)
        .clamp(1, MAX_PULL_LIMIT);
    let fetch_limit = limit + 1;

    let current_user_seq: i64 = sqlx::query("SELECT sync_seq FROM users WHERE id = ?")
        .bind(&auth.user_id)
        .fetch_one(&state.pool)
        .await?
        .try_get("sync_seq")?;

    let task_sql = format!(
        "SELECT {} FROM tasks WHERE user_id = ? AND server_seq > ? ORDER BY server_seq ASC LIMIT ?",
        SELECT_TASK_COLS
    );
    let recurring_sql = format!(
        "SELECT {} FROM recurring_tasks WHERE user_id = ? AND server_seq > ? ORDER BY server_seq ASC LIMIT ?",
        SELECT_RECURRING_COLS
    );
    let record_sql = format!(
        "SELECT {} FROM reminder_records WHERE user_id = ? AND server_seq > ? ORDER BY server_seq ASC LIMIT ?",
        SELECT_RECORD_COLS
    );

    let task_rows = sqlx::query(&task_sql)
        .bind(&auth.user_id)
        .bind(since)
        .bind(fetch_limit)
        .fetch_all(&state.pool)
        .await?;
    let recurring_rows = sqlx::query(&recurring_sql)
        .bind(&auth.user_id)
        .bind(since)
        .bind(fetch_limit)
        .fetch_all(&state.pool)
        .await?;
    let record_rows = sqlx::query(&record_sql)
        .bind(&auth.user_id)
        .bind(since)
        .bind(fetch_limit)
        .fetch_all(&state.pool)
        .await?;

    let mut tasks = Vec::with_capacity(task_rows.len());
    for r in &task_rows {
        tasks.push(task_from_row(r)?);
    }
    let mut recurring = Vec::with_capacity(recurring_rows.len());
    for r in &recurring_rows {
        recurring.push(recurring_from_row(r)?);
    }
    let mut records = Vec::with_capacity(record_rows.len());
    for r in &record_rows {
        records.push(record_from_row(r)?);
    }

    let limit_usize = limit as usize;
    let mut cutoffs = Vec::new();
    if tasks.len() > limit_usize {
        cutoffs.push(tasks[limit_usize - 1].1);
    }
    if recurring.len() > limit_usize {
        cutoffs.push(recurring[limit_usize - 1].1);
    }
    if records.len() > limit_usize {
        cutoffs.push(records[limit_usize - 1].1);
    }

    let (server_seq, has_more) = if let Some(cutoff) = cutoffs.into_iter().min() {
        tasks.retain(|(_, seq)| *seq <= cutoff);
        recurring.retain(|(_, seq)| *seq <= cutoff);
        records.retain(|(_, seq)| *seq <= cutoff);
        (cutoff, true)
    } else {
        let max_seen = tasks
            .iter()
            .map(|(_, s)| *s)
            .chain(recurring.iter().map(|(_, s)| *s))
            .chain(records.iter().map(|(_, s)| *s))
            .max()
            .unwrap_or(since);
        (max_seen.max(current_user_seq).max(since), false)
    };

    Ok(Json(SyncPullResponse {
        server_seq,
        has_more,
        changes: SyncChangeSet {
            tasks: tasks.into_iter().map(|(row, _)| row).collect(),
            recurring_tasks: recurring.into_iter().map(|(row, _)| row).collect(),
            reminder_records: records.into_iter().map(|(row, _)| row).collect(),
        },
    }))
}

pub async fn push(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<SyncPushRequest>,
) -> ServerResult<Json<SyncPushResponse>> {
    let mut tx = state.pool.begin().await?;
    let initial_seq: i64 = sqlx::query("SELECT sync_seq FROM users WHERE id = ? FOR UPDATE")
        .bind(&auth.user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ServerError::Unauthorized("账号不存在".to_string()))?
        .try_get("sync_seq")?;

    let mut seq = initial_seq;
    let mut accepted = 0usize;
    let mut rejected = 0usize;
    let mut server_wins = SyncChangeSet::default();

    let select_task_one = format!(
        "SELECT {} FROM tasks WHERE user_id = ? AND id = ?",
        SELECT_TASK_COLS
    );
    for incoming in &payload.changes.tasks {
        if incoming.id.trim().is_empty() {
            continue;
        }
        let existing = sqlx::query(&select_task_one)
            .bind(&auth.user_id)
            .bind(&incoming.id)
            .fetch_optional(&mut *tx)
            .await?
            .map(|r| task_from_row(&r))
            .transpose()?
            .map(|(r, _)| r);

        match existing {
            None => {
                seq += 1;
                upsert_task(&mut tx, &auth.user_id, incoming, seq).await?;
                accepted += 1;
            }
            Some(ref existing_row) => {
                if is_incoming_newer(existing_row.compare_time(), incoming.compare_time()) {
                    seq += 1;
                    upsert_task(&mut tx, &auth.user_id, incoming, seq).await?;
                    accepted += 1;
                } else if existing_row == incoming {
                    accepted += 1;
                } else {
                    rejected += 1;
                    server_wins.tasks.push(existing_row.clone());
                }
            }
        }
    }

    let select_rec_one = format!(
        "SELECT {} FROM recurring_tasks WHERE user_id = ? AND id = ?",
        SELECT_RECURRING_COLS
    );
    for incoming in &payload.changes.recurring_tasks {
        if incoming.id.trim().is_empty() {
            continue;
        }
        let existing = sqlx::query(&select_rec_one)
            .bind(&auth.user_id)
            .bind(&incoming.id)
            .fetch_optional(&mut *tx)
            .await?
            .map(|r| recurring_from_row(&r))
            .transpose()?
            .map(|(r, _)| r);

        match existing {
            None => {
                seq += 1;
                upsert_recurring(&mut tx, &auth.user_id, incoming, seq).await?;
                accepted += 1;
            }
            Some(ref existing_row) => {
                if is_incoming_newer(existing_row.compare_time(), incoming.compare_time()) {
                    seq += 1;
                    upsert_recurring(&mut tx, &auth.user_id, incoming, seq).await?;
                    accepted += 1;
                } else if existing_row == incoming {
                    accepted += 1;
                } else {
                    rejected += 1;
                    server_wins.recurring_tasks.push(existing_row.clone());
                }
            }
        }
    }

    let select_record_one = format!(
        "SELECT {} FROM reminder_records WHERE user_id = ? AND id = ?",
        SELECT_RECORD_COLS
    );
    for incoming in &payload.changes.reminder_records {
        if incoming.id.trim().is_empty() {
            continue;
        }
        let existing = sqlx::query(&select_record_one)
            .bind(&auth.user_id)
            .bind(&incoming.id)
            .fetch_optional(&mut *tx)
            .await?
            .map(|r| record_from_row(&r))
            .transpose()?
            .map(|(r, _)| r);

        match existing {
            None => {
                seq += 1;
                upsert_record(&mut tx, &auth.user_id, incoming, seq).await?;
                accepted += 1;
            }
            Some(ref existing_row) => {
                if is_incoming_newer(existing_row.compare_time(), incoming.compare_time()) {
                    seq += 1;
                    upsert_record(&mut tx, &auth.user_id, incoming, seq).await?;
                    accepted += 1;
                } else if existing_row == incoming {
                    accepted += 1;
                } else {
                    rejected += 1;
                    server_wins.reminder_records.push(existing_row.clone());
                }
            }
        }
    }

    if seq != initial_seq {
        sqlx::query("UPDATE users SET sync_seq = ? WHERE id = ?")
            .bind(seq)
            .bind(&auth.user_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    if seq != initial_seq {
        state.events.publish(
            &auth.user_id,
            SyncEventPayload {
                server_seq: seq,
                device_id: payload.device_id,
            },
        );
    }

    Ok(Json(SyncPushResponse {
        server_seq: seq,
        accepted,
        rejected,
        server_wins,
    }))
}

pub async fn merge(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<SyncMergeRequest>,
) -> ServerResult<Json<SyncMergeResponse>> {
    let mut tx = state.pool.begin().await?;
    let initial_seq: i64 = sqlx::query("SELECT sync_seq FROM users WHERE id = ? FOR UPDATE")
        .bind(&auth.user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ServerError::Unauthorized("账号不存在".to_string()))?
        .try_get("sync_seq")?;

    let mut seq = initial_seq;
    let mut summary = SyncMergeSummary::default();
    let mut remote_changes = SyncChangeSet::default();

    // 1. tasks
    let existing_task_rows = sqlx::query(&format!(
        "SELECT {} FROM tasks WHERE user_id = ?",
        SELECT_TASK_COLS
    ))
    .bind(&auth.user_id)
    .fetch_all(&mut *tx)
    .await?;
    let mut existing_tasks: HashMap<String, SyncTaskRow> = HashMap::new();
    for r in &existing_task_rows {
        let (item, _) = task_from_row(r)?;
        existing_tasks.insert(item.id.clone(), item);
    }
    let mut seen_tasks = HashSet::new();
    for incoming in &payload.changes.tasks {
        if incoming.id.trim().is_empty() || !seen_tasks.insert(incoming.id.clone()) {
            continue;
        }
        match existing_tasks.get(&incoming.id) {
            None => {
                seq += 1;
                upsert_task(&mut tx, &auth.user_id, incoming, seq).await?;
                summary.uploaded += 1;
            }
            Some(existing) => {
                if is_incoming_newer(existing.compare_time(), incoming.compare_time()) {
                    seq += 1;
                    upsert_task(&mut tx, &auth.user_id, incoming, seq).await?;
                    summary.updated_on_server += 1;
                } else {
                    remote_changes.tasks.push(existing.clone());
                    if existing != incoming {
                        summary.server_wins += 1;
                    }
                }
            }
        }
    }
    for (id, row) in existing_tasks {
        if !seen_tasks.contains(&id) {
            remote_changes.tasks.push(row);
            summary.server_only += 1;
        }
    }

    // 2. recurring_tasks
    let existing_rec_rows = sqlx::query(&format!(
        "SELECT {} FROM recurring_tasks WHERE user_id = ?",
        SELECT_RECURRING_COLS
    ))
    .bind(&auth.user_id)
    .fetch_all(&mut *tx)
    .await?;
    let mut existing_recs: HashMap<String, SyncRecurringTaskRow> = HashMap::new();
    for r in &existing_rec_rows {
        let (item, _) = recurring_from_row(r)?;
        existing_recs.insert(item.id.clone(), item);
    }
    let mut seen_recs = HashSet::new();
    for incoming in &payload.changes.recurring_tasks {
        if incoming.id.trim().is_empty() || !seen_recs.insert(incoming.id.clone()) {
            continue;
        }
        match existing_recs.get(&incoming.id) {
            None => {
                seq += 1;
                upsert_recurring(&mut tx, &auth.user_id, incoming, seq).await?;
                summary.uploaded += 1;
            }
            Some(existing) => {
                if is_incoming_newer(existing.compare_time(), incoming.compare_time()) {
                    seq += 1;
                    upsert_recurring(&mut tx, &auth.user_id, incoming, seq).await?;
                    summary.updated_on_server += 1;
                } else {
                    remote_changes.recurring_tasks.push(existing.clone());
                    if existing != incoming {
                        summary.server_wins += 1;
                    }
                }
            }
        }
    }
    for (id, row) in existing_recs {
        if !seen_recs.contains(&id) {
            remote_changes.recurring_tasks.push(row);
            summary.server_only += 1;
        }
    }

    // 3. reminder_records
    let existing_record_rows = sqlx::query(&format!(
        "SELECT {} FROM reminder_records WHERE user_id = ?",
        SELECT_RECORD_COLS
    ))
    .bind(&auth.user_id)
    .fetch_all(&mut *tx)
    .await?;
    let mut existing_records: HashMap<String, SyncReminderRecordRow> = HashMap::new();
    for r in &existing_record_rows {
        let (item, _) = record_from_row(r)?;
        existing_records.insert(item.id.clone(), item);
    }
    let mut seen_records = HashSet::new();
    for incoming in &payload.changes.reminder_records {
        if incoming.id.trim().is_empty() || !seen_records.insert(incoming.id.clone()) {
            continue;
        }
        match existing_records.get(&incoming.id) {
            None => {
                seq += 1;
                upsert_record(&mut tx, &auth.user_id, incoming, seq).await?;
                summary.uploaded += 1;
            }
            Some(existing) => {
                if is_incoming_newer(existing.compare_time(), incoming.compare_time()) {
                    seq += 1;
                    upsert_record(&mut tx, &auth.user_id, incoming, seq).await?;
                    summary.updated_on_server += 1;
                } else {
                    remote_changes.reminder_records.push(existing.clone());
                    if existing != incoming {
                        summary.server_wins += 1;
                    }
                }
            }
        }
    }
    for (id, row) in existing_records {
        if !seen_records.contains(&id) {
            remote_changes.reminder_records.push(row);
            summary.server_only += 1;
        }
    }

    if seq != initial_seq {
        sqlx::query("UPDATE users SET sync_seq = ? WHERE id = ?")
            .bind(seq)
            .bind(&auth.user_id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

    if seq != initial_seq {
        state.events.publish(
            &auth.user_id,
            SyncEventPayload {
                server_seq: seq,
                device_id: payload.device_id,
            },
        );
    }

    Ok(Json(SyncMergeResponse {
        server_seq: seq,
        summary,
        remote_changes,
    }))
}

pub async fn events(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ServerResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let current_seq: i64 = sqlx::query("SELECT sync_seq FROM users WHERE id = ?")
        .bind(&auth.user_id)
        .fetch_one(&state.pool)
        .await?
        .try_get("sync_seq")?;

    let rx = state.events.subscribe(&auth.user_id);
    let initial_payload = serde_json::to_string(&SyncEventPayload {
        server_seq: current_seq,
        device_id: String::new(),
    })
    .unwrap_or_else(|_| "{}".to_string());

    let initial = tokio_stream::once(Ok(Event::default()
        .event("connected")
        .data(initial_payload)));

    let updates = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(payload) => {
            let data = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
            Some(Ok(Event::default().event("sync").data(data)))
        }
        Err(_) => {
            // 消费端滞后丢消息时，发出 resync 事件触发客户端拉取最新增量。
            Some(Ok(Event::default().event("resync").data("{}")))
        }
    });

    Ok(Sse::new(initial.chain(updates)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(20))
            .text("ping"),
    ))
}
