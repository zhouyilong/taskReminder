//! TaskReminder v3.0.0 云端单体服务库入口。

pub mod auth;
pub mod bootstrap;
pub mod cli;
pub mod config;
pub mod error;
pub mod events;
pub mod routes;
pub mod state;

pub use routes::build_router;
