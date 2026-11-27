# 多阶段构建：Rust 编译阶段 -> Debian Bookworm Slim 精简运行阶段（支持 docker exec 应急运维 CLI 与健康检查）
FROM rust:1-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml ./
COPY crates/core crates/core
COPY server server

RUN cargo build --release --bin task-reminder-server

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 -s /usr/sbin/nologin appuser

COPY --from=builder /workspace/target/release/task-reminder-server /usr/local/bin/task-reminder-server

USER appuser
EXPOSE 8080

ENV SERVER_ADDR=0.0.0.0:8080 \
    RUST_LOG=info,task_reminder_server=info

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/api/v1/health || exit 1

ENTRYPOINT ["/usr/local/bin/task-reminder-server"]
CMD ["serve"]
