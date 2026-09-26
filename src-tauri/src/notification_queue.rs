use std::sync::{Arc, Mutex, MutexGuard};

use crate::models::NotificationPayload;

/// 待处理的提醒队列。
///
/// 同一时间可能有多条提醒到点（或启动时补发多条错过的提醒），
/// 弹窗按队列顺序逐条展示，处理完一条再显示下一条，避免后到的提醒覆盖先到的提醒。
#[derive(Clone, Default)]
pub struct NotificationQueue {
    items: Arc<Mutex<Vec<NotificationPayload>>>,
}

impl NotificationQueue {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Vec<NotificationPayload>> {
        self.items.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 入队。若同一提醒（`reminder_id`）已在队列中，用新的一条原位替换旧的一条，
    /// 避免高频循环提醒在无人处理时不断堆积。
    pub fn push(&self, payload: NotificationPayload) -> Vec<NotificationPayload> {
        let mut items = self.lock();
        if let Some(existing) = items
            .iter_mut()
            .find(|item| item.reminder_id == payload.reminder_id)
        {
            *existing = payload;
        } else {
            items.push(payload);
        }
        items.clone()
    }

    /// 移除指定提醒记录，返回剩余队列。
    pub fn remove_record(&self, record_id: &str) -> Vec<NotificationPayload> {
        let mut items = self.lock();
        items.retain(|item| item.record_id != record_id);
        items.clone()
    }

    /// 移除某个任务 / 循环提醒对应的全部条目，返回被移除的条目。
    pub fn remove_reminder(&self, reminder_id: &str) -> Vec<NotificationPayload> {
        let mut items = self.lock();
        let (removed, kept): (Vec<_>, Vec<_>) = items
            .drain(..)
            .partition(|item| item.reminder_id == reminder_id);
        *items = kept;
        removed
    }

    /// 清空队列，返回被清空的条目。
    pub fn drain(&self) -> Vec<NotificationPayload> {
        std::mem::take(&mut *self.lock())
    }

    pub fn snapshot(&self) -> Vec<NotificationPayload> {
        self.lock().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(record_id: &str, reminder_id: &str) -> NotificationPayload {
        NotificationPayload {
            record_id: record_id.to_string(),
            reminder_id: reminder_id.to_string(),
            reminder_type: "TASK".to_string(),
            description: format!("task {}", reminder_id),
            snooze_minutes: 5,
            scheduled_time: None,
        }
    }

    #[test]
    fn push_keeps_arrival_order() {
        let queue = NotificationQueue::new();
        queue.push(payload("r1", "a"));
        let items = queue.push(payload("r2", "b"));
        let ids: Vec<_> = items.iter().map(|item| item.record_id.as_str()).collect();
        assert_eq!(ids, vec!["r1", "r2"]);
    }

    #[test]
    fn push_replaces_same_reminder_in_place() {
        let queue = NotificationQueue::new();
        queue.push(payload("r1", "a"));
        queue.push(payload("r2", "b"));
        let items = queue.push(payload("r3", "a"));
        let ids: Vec<_> = items.iter().map(|item| item.record_id.as_str()).collect();
        assert_eq!(ids, vec!["r3", "r2"]);
    }

    #[test]
    fn remove_record_returns_remaining() {
        let queue = NotificationQueue::new();
        queue.push(payload("r1", "a"));
        queue.push(payload("r2", "b"));
        let remaining = queue.remove_record("r1");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].record_id, "r2");
        assert!(queue.remove_record("missing").len() == 1);
    }

    #[test]
    fn remove_reminder_and_drain() {
        let queue = NotificationQueue::new();
        queue.push(payload("r1", "a"));
        queue.push(payload("r2", "b"));
        let removed = queue.remove_reminder("b");
        assert_eq!(removed.len(), 1);
        assert_eq!(queue.snapshot().len(), 1);
        assert_eq!(queue.drain().len(), 1);
        assert!(queue.snapshot().is_empty());
    }
}
