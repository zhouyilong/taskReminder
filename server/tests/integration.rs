//! 服务端 MySQL 集成测试（需要环境变量 `TEST_DATABASE_URL`，未设置时跳过）。
//! 覆盖：管理员初始化、登录/续期/登出、角色权限与立即吊销、跨账号同 UUID 隔离、
//! 并发 push 严格递增 sync_seq、LWW 冲突裁决、墓碑不复活、模式切换 merge。

use std::sync::Arc;

use axum::{
    body::Body,
    http::{header, Method, Request, StatusCode},
};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use sqlx::mysql::MySqlPoolOptions;
use task_reminder_core::kinds::{TaskStatus, TaskType, UserRole};
use task_reminder_core::protocol::{
    AdminCreateUserRequest, AdminResetPasswordRequest, AdminUpdateUserRequest, AdminUserSummary,
    AuthTokenResponse, ChangePasswordRequest, HealthResponse, LoginRequest, RefreshRequest,
    SyncChangeSet, SyncMergeRequest, SyncMergeResponse, SyncPullResponse, SyncPushRequest,
    SyncPushResponse, SyncTaskRow, UpdateProfileRequest, UserInfo,
};
use tower::ServiceExt;

use task_reminder_server::bootstrap::{ensure_initial_admin, run_migrations};
use task_reminder_server::config::ServerConfig;
use task_reminder_server::events::EventHub;
use task_reminder_server::routes::build_router;
use task_reminder_server::state::AppState;

async fn setup_test_app() -> Option<(axum::Router, sqlx::MySqlPool)> {
    let url = std::env::var("TEST_DATABASE_URL").ok()?;
    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .connect(&url)
        .await
        .expect("连接测试 MySQL 失败");
    run_migrations(&pool).await.expect("运行迁移失败");

    // 清理测试库数据
    sqlx::query("DELETE FROM users")
        .execute(&pool)
        .await
        .unwrap();

    let config = ServerConfig {
        server_addr: "127.0.0.1:0".to_string(),
        database_url: url,
        jwt_secret: "integration-test-jwt-secret-32bytes-minimum!".to_string(),
        init_admin_username: "admin".to_string(),
        init_admin_password: Some("AdminPass123!".to_string()),
        access_token_ttl_minutes: 30,
        refresh_token_ttl_days: 30,
    };
    ensure_initial_admin(&pool, &config)
        .await
        .expect("初始化管理员失败");

    let state = AppState {
        pool: pool.clone(),
        config: Arc::new(config),
        events: EventHub::new(),
    };
    Some((build_router(state), pool))
}

async fn send_json<B: Serialize>(
    app: &axum::Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<&B>,
) -> (StatusCode, Vec<u8>) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {}", t));
    }
    let req = if let Some(b) = body {
        builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_vec(b).unwrap()))
            .unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, bytes)
}

fn parse_json<T: DeserializeOwned>(bytes: &[u8]) -> T {
    serde_json::from_slice(bytes).unwrap_or_else(|e| {
        panic!(
            "解析 JSON 失败: {} | raw={}",
            e,
            String::from_utf8_lossy(bytes)
        )
    })
}

fn sample_task(id: &str, desc: &str, updated_at: &str, deleted_at: Option<&str>) -> SyncTaskRow {
    SyncTaskRow {
        id: id.to_string(),
        description: desc.to_string(),
        task_type: TaskType::OneTime,
        status: TaskStatus::Pending,
        created_at: "2026-11-27T08:00:00".to_string(),
        completed_at: None,
        reminder_time: Some("2026-11-27T18:00:00".to_string()),
        updated_at: Some(updated_at.to_string()),
        deleted_at: deleted_at.map(str::to_string),
        sticky_content: String::new(),
        sticky_pos_x: 48.0,
        sticky_pos_y: 76.0,
        sticky_is_open: false,
        sticky_width: 420.0,
        sticky_height: 300.0,
        tags: "工作".to_string(),
        priority: 2,
        due_at: None,
        sticky_color: String::new(),
        sort_order: None,
        project: "v3".to_string(),
        urgent: false,
    }
}

#[tokio::test]
async fn full_auth_admin_sync_and_isolation_flow() {
    let Some((app, _pool)) = setup_test_app().await else {
        eprintln!("跳过 MySQL 集成测试：未设置 TEST_DATABASE_URL");
        return;
    };

    // 1. 健康检查
    let (st, bytes) = send_json::<()>(&app, Method::GET, "/api/v1/health", None, None).await;
    assert_eq!(st, StatusCode::OK);
    let health: HealthResponse = parse_json(&bytes);
    assert_eq!(health.status, "ok");
    assert_eq!(health.schema_version, "3.0.0");

    // 2. 管理员登录 & Token 轮换
    let (st, bytes) = send_json(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(&LoginRequest {
            username: "admin".to_string(),
            password: "AdminPass123!".to_string(),
            device_id: "dev-admin-1".to_string(),
            device_name: "Admin Mac".to_string(),
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let admin_auth: AuthTokenResponse = parse_json(&bytes);
    assert_eq!(admin_auth.user.role, UserRole::Admin);

    let (st, bytes) = send_json(
        &app,
        Method::POST,
        "/api/v1/auth/refresh",
        None,
        Some(&RefreshRequest {
            refresh_token: admin_auth.refresh_token.clone(),
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let refreshed: AuthTokenResponse = parse_json(&bytes);
    assert_ne!(refreshed.refresh_token, admin_auth.refresh_token);
    let admin_token = refreshed.access_token;

    // 3. 管理员创建两个普通用户 alice 与 bob
    let (st, bytes) = send_json(
        &app,
        Method::POST,
        "/api/v1/admin/users",
        Some(&admin_token),
        Some(&AdminCreateUserRequest {
            username: "alice".to_string(),
            nickname: "爱丽丝".to_string(),
            role: UserRole::User,
            password: "AlicePass123!".to_string(),
            must_change_password: true,
        }),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let alice_summary: AdminUserSummary = parse_json(&bytes);
    assert!(alice_summary.must_change_password);

    let (st, bytes) = send_json(
        &app,
        Method::POST,
        "/api/v1/admin/users",
        Some(&admin_token),
        Some(&AdminCreateUserRequest {
            username: "bob".to_string(),
            nickname: "鲍勃".to_string(),
            role: UserRole::User,
            password: "BobPass123!".to_string(),
            must_change_password: false,
        }),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let bob_summary: AdminUserSummary = parse_json(&bytes);

    // 4. alice 登录，无法访问管理员接口（403），修改密码后清除 must_change_password
    let (st, bytes) = send_json(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(&LoginRequest {
            username: "alice".to_string(),
            password: "AlicePass123!".to_string(),
            device_id: "dev-alice-1".to_string(),
            device_name: "Alice PC".to_string(),
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let alice_auth: AuthTokenResponse = parse_json(&bytes);
    assert!(alice_auth.user.must_change_password);

    let (st, _) = send_json::<()>(
        &app,
        Method::GET,
        "/api/v1/admin/users",
        Some(&alice_auth.access_token),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    let (st, bytes) = send_json(
        &app,
        Method::PUT,
        "/api/v1/auth/password",
        Some(&alice_auth.access_token),
        Some(&ChangePasswordRequest {
            current_password: "AlicePass123!".to_string(),
            new_password: "AliceNewPass456!".to_string(),
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let updated_alice: UserInfo = parse_json(&bytes);
    assert!(!updated_alice.must_change_password);

    let (st, bytes) = send_json(
        &app,
        Method::PUT,
        "/api/v1/auth/profile",
        Some(&alice_auth.access_token),
        Some(&UpdateProfileRequest {
            nickname: "Alice 新昵称".to_string(),
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let profiled: UserInfo = parse_json(&bytes);
    assert_eq!(profiled.nickname, "Alice 新昵称");

    // 5. 跨账号同 UUID 隔离测试：alice 和 bob 同时写入相同 UUID `shared-uuid-1`
    let (st, bytes) = send_json(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(&LoginRequest {
            username: "bob".to_string(),
            password: "BobPass123!".to_string(),
            device_id: "dev-bob-1".to_string(),
            device_name: "Bob PC".to_string(),
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let bob_auth: AuthTokenResponse = parse_json(&bytes);

    let (st, bytes) = send_json(
        &app,
        Method::POST,
        "/api/v1/sync/push",
        Some(&alice_auth.access_token),
        Some(&SyncPushRequest {
            device_id: "dev-alice-1".to_string(),
            changes: SyncChangeSet {
                tasks: vec![sample_task(
                    "shared-uuid-1",
                    "Alice 的私密待办",
                    "2026-11-27T10:00:00",
                    None,
                )],
                ..Default::default()
            },
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let alice_push: SyncPushResponse = parse_json(&bytes);
    assert_eq!(alice_push.accepted, 1);
    assert_eq!(alice_push.server_seq, 1);

    let (st, bytes) = send_json(
        &app,
        Method::POST,
        "/api/v1/sync/push",
        Some(&bob_auth.access_token),
        Some(&SyncPushRequest {
            device_id: "dev-bob-1".to_string(),
            changes: SyncChangeSet {
                tasks: vec![sample_task(
                    "shared-uuid-1",
                    "Bob 的私密待办",
                    "2026-11-27T11:00:00",
                    None,
                )],
                ..Default::default()
            },
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let bob_push: SyncPushResponse = parse_json(&bytes);
    assert_eq!(bob_push.accepted, 1);

    // 拉取校验各自只看到自己的数据
    let (st, bytes) = send_json::<()>(
        &app,
        Method::GET,
        "/api/v1/sync/pull?since=0",
        Some(&alice_auth.access_token),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let alice_pull: SyncPullResponse = parse_json(&bytes);
    assert_eq!(alice_pull.changes.tasks.len(), 1);
    assert_eq!(alice_pull.changes.tasks[0].description, "Alice 的私密待办");

    let (st, bytes) = send_json::<()>(
        &app,
        Method::GET,
        "/api/v1/sync/pull?since=0",
        Some(&bob_auth.access_token),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let bob_pull: SyncPullResponse = parse_json(&bytes);
    assert_eq!(bob_pull.changes.tasks.len(), 1);
    assert_eq!(bob_pull.changes.tasks[0].description, "Bob 的私密待办");

    // 6. LWW 冲突裁决 & 墓碑防复活
    // 先推送一个较新的删除墓碑（12:00）
    let (st, bytes) = send_json(
        &app,
        Method::POST,
        "/api/v1/sync/push",
        Some(&alice_auth.access_token),
        Some(&SyncPushRequest {
            device_id: "dev-alice-1".to_string(),
            changes: SyncChangeSet {
                tasks: vec![sample_task(
                    "shared-uuid-1",
                    "Alice 的私密待办",
                    "2026-11-27T12:00:00",
                    Some("2026-11-27T12:00:00"),
                )],
                ..Default::default()
            },
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let del_push: SyncPushResponse = parse_json(&bytes);
    assert_eq!(del_push.accepted, 1);
    assert_eq!(del_push.server_seq, 2);

    // 再推送一个较旧的未删除版本（11:30）：必须被拒并返回服务端墓碑（绝不复活）
    let (st, bytes) = send_json(
        &app,
        Method::POST,
        "/api/v1/sync/push",
        Some(&alice_auth.access_token),
        Some(&SyncPushRequest {
            device_id: "dev-alice-2".to_string(),
            changes: SyncChangeSet {
                tasks: vec![sample_task(
                    "shared-uuid-1",
                    "旧版本试图复活",
                    "2026-11-27T11:30:00",
                    None,
                )],
                ..Default::default()
            },
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let stale_push: SyncPushResponse = parse_json(&bytes);
    assert_eq!(stale_push.rejected, 1);
    assert_eq!(stale_push.server_wins.tasks.len(), 1);
    assert!(stale_push.server_wins.tasks[0].deleted_at.is_some());

    // 7. 模式切换全量 Merge 测试
    let (st, bytes) = send_json(
        &app,
        Method::POST,
        "/api/v1/sync/merge",
        Some(&alice_auth.access_token),
        Some(&SyncMergeRequest {
            device_id: "dev-alice-2".to_string(),
            changes: SyncChangeSet {
                tasks: vec![sample_task(
                    "local-only-1",
                    "本地新增待办",
                    "2026-11-27T13:00:00",
                    None,
                )],
                ..Default::default()
            },
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let merge_resp: SyncMergeResponse = parse_json(&bytes);
    assert_eq!(merge_resp.summary.uploaded, 1);
    assert_eq!(merge_resp.summary.server_only, 1);
    assert_eq!(merge_resp.remote_changes.tasks.len(), 1);
    assert_eq!(merge_resp.remote_changes.tasks[0].id, "shared-uuid-1");

    // 8. 并发 push 保证 sync_seq 严格递增
    let mut handles = Vec::new();
    for i in 0..8 {
        let app_clone = app.clone();
        let token = bob_auth.access_token.clone();
        handles.push(tokio::spawn(async move {
            let (st, bytes) = send_json(
                &app_clone,
                Method::POST,
                "/api/v1/sync/push",
                Some(&token),
                Some(&SyncPushRequest {
                    device_id: format!("dev-bob-{}", i),
                    changes: SyncChangeSet {
                        tasks: vec![sample_task(
                            &format!("concurrent-{}", i),
                            &format!("并发待办 {}", i),
                            "2026-11-27T14:00:00",
                            None,
                        )],
                        ..Default::default()
                    },
                }),
            )
            .await;
            assert_eq!(st, StatusCode::OK);
            parse_json::<SyncPushResponse>(&bytes).server_seq
        }));
    }
    let mut seqs = Vec::new();
    for h in handles {
        seqs.push(h.await.unwrap());
    }
    seqs.sort();
    seqs.dedup();
    assert_eq!(seqs.len(), 8);
    assert_eq!(*seqs.last().unwrap(), 9);

    // 9. 管理员禁用 alice -> alice 已有 Access Token 立即被 401 拒绝
    let (st, _) = send_json(
        &app,
        Method::PATCH,
        &format!("/api/v1/admin/users/{}", alice_summary.id),
        Some(&admin_token),
        Some(&AdminUpdateUserRequest {
            enabled: Some(false),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    let (st, _) = send_json::<()>(
        &app,
        Method::GET,
        "/api/v1/auth/me",
        Some(&alice_auth.access_token),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // 10. 管理员重置 bob 密码 -> bob 旧会话立即失效
    let (st, _) = send_json(
        &app,
        Method::POST,
        &format!("/api/v1/admin/users/{}/reset-password", bob_summary.id),
        Some(&admin_token),
        Some(&AdminResetPasswordRequest {
            new_password: "BobResetPass999!".to_string(),
            must_change_password: true,
        }),
    )
    .await;
    assert_eq!(st, StatusCode::OK);

    let (st, _) = send_json::<()>(
        &app,
        Method::GET,
        "/api/v1/auth/me",
        Some(&bob_auth.access_token),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);

    // 11. 管理员不能删除自己，但可以删除 bob（级联清空数据）
    let (st, _) = send_json::<()>(
        &app,
        Method::DELETE,
        &format!("/api/v1/admin/users/{}", admin_auth.user.id),
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::BAD_REQUEST);

    let (st, _) = send_json::<()>(
        &app,
        Method::DELETE,
        &format!("/api/v1/admin/users/{}", bob_summary.id),
        Some(&admin_token),
        None,
    )
    .await;
    assert_eq!(st, StatusCode::NO_CONTENT);
}
