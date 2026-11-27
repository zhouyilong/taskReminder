//! 服务端共享应用状态。

use std::sync::Arc;

use sqlx::MySqlPool;

use crate::config::ServerConfig;
use crate::events::EventHub;

#[derive(Clone)]
pub struct AppState {
    pub pool: MySqlPool,
    pub config: Arc<ServerConfig>,
    pub events: EventHub,
}
