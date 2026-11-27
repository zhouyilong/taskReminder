use std::sync::Arc;
use std::time::Duration;

use clap::{Parser, Subcommand};
use sqlx::mysql::MySqlPoolOptions;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use task_reminder_server::bootstrap::{ensure_initial_admin, run_migrations};
use task_reminder_server::cli::{run_admin_command, AdminCommand};
use task_reminder_server::config::ServerConfig;
use task_reminder_server::events::EventHub;
use task_reminder_server::routes::build_router;
use task_reminder_server::state::AppState;

#[derive(Parser, Debug)]
#[command(
    name = "task-reminder-server",
    version,
    about = "任务提醒 v3.0.0 云端服务"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// 启动 HTTP + SSE 云端服务（默认）
    Serve,
    /// 管理员应急运维命令
    Admin {
        #[command(subcommand)]
        action: AdminCommand,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,task_reminder_server=info")),
        )
        .with(fmt::layer())
        .init();

    let cli = Cli::parse();
    let config =
        ServerConfig::from_env().map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

    let pool = MySqlPoolOptions::new()
        .max_connections(16)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&config.database_url)
        .await?;

    run_migrations(&pool).await?;

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => {
            ensure_initial_admin(&pool, &config).await?;
            let addr = config.server_addr.clone();
            let state = AppState {
                pool,
                config: Arc::new(config),
                events: EventHub::new(),
            };
            let app = build_router(state);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            tracing::info!(addr = %addr, version = env!("CARGO_PKG_VERSION"), "TaskReminder 云端服务已启动");
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal())
                .await?;
        }
        Command::Admin { action } => {
            run_admin_command(&pool, action)
                .await
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        }
    }

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("收到停止信号，正在优雅关闭服务...");
}
