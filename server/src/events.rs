//! 进程内 SSE 事件广播中心，按 `user_id` 严格隔离。

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use task_reminder_core::protocol::SyncEventPayload;
use tokio::sync::broadcast;

const CHANNEL_CAPACITY: usize = 64;

#[derive(Clone, Default)]
pub struct EventHub {
    channels: Arc<RwLock<HashMap<String, broadcast::Sender<SyncEventPayload>>>>,
}

impl EventHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self, user_id: &str) -> broadcast::Receiver<SyncEventPayload> {
        let mut map = self.channels.write().unwrap_or_else(|e| e.into_inner());
        let sender = map
            .entry(user_id.to_string())
            .or_insert_with(|| broadcast::channel(CHANNEL_CAPACITY).0);
        sender.subscribe()
    }

    pub fn publish(&self, user_id: &str, payload: SyncEventPayload) {
        let mut map = self.channels.write().unwrap_or_else(|e| e.into_inner());
        if let Some(sender) = map.get(user_id) {
            if sender.receiver_count() == 0 {
                map.remove(user_id);
            } else {
                let _ = sender.send(payload);
            }
        }
    }
}
