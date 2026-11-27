pub mod admin;
pub mod auth;
pub mod sync;
pub mod system;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, patch, post, put},
    Router,
};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::state::AppState;

/// 单次同步/合并请求体最大 16 MB，满足大数据集全量合并需求。
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

pub fn build_router(state: AppState) -> Router {
    let api = Router::new()
        // 系统
        .route("/health", get(system::health))
        .route("/time", get(system::server_time))
        // 认证
        .route("/auth/login", post(auth::login))
        .route("/auth/refresh", post(auth::refresh))
        .route("/auth/logout", post(auth::logout))
        .route("/auth/me", get(auth::me))
        .route("/auth/password", put(auth::change_password))
        .route("/auth/profile", put(auth::update_profile))
        // 同步与 SSE
        .route("/sync/pull", get(sync::pull))
        .route("/sync/push", post(sync::push))
        .route("/sync/merge", post(sync::merge))
        .route("/sync/events", get(sync::events))
        // 管理员账号管理
        .route(
            "/admin/users",
            get(admin::list_users).post(admin::create_user),
        )
        .route(
            "/admin/users/{id}",
            patch(admin::update_user).delete(admin::delete_user),
        )
        .route(
            "/admin/users/{id}/reset-password",
            post(admin::reset_password),
        );

    Router::new()
        .nest("/api/v1", api)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(CompressionLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
