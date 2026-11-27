-- TaskReminder v3.0.0 MySQL 8.0 初始化表结构
-- 统一字符集：utf8mb4 / utf8mb4_0900_ai_ci
-- 业务时间列保持 VARCHAR(32) 原样存储 YYYY-MM-DDTHH:MM:SS（与客户端 SQLite 100% 一致）

CREATE TABLE IF NOT EXISTS users (
  id                   VARCHAR(36)  NOT NULL PRIMARY KEY,
  username             VARCHAR(64)  NOT NULL UNIQUE,
  nickname             VARCHAR(64)  NOT NULL DEFAULT '',
  password_hash        VARCHAR(255) NOT NULL,
  role                 VARCHAR(16)  NOT NULL DEFAULT 'user' COMMENT 'admin | user',
  status               TINYINT      NOT NULL DEFAULT 1 COMMENT '1=启用 0=禁用',
  must_change_password TINYINT(1)   NOT NULL DEFAULT 0,
  sync_seq             BIGINT       NOT NULL DEFAULT 0 COMMENT '账号内单调递增变更序列号',
  last_login_at        DATETIME(3)  NULL,
  created_at           DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  updated_at           DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3) ON UPDATE CURRENT_TIMESTAMP(3)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS user_sessions (
  id                 VARCHAR(36)  NOT NULL PRIMARY KEY,
  user_id            VARCHAR(36)  NOT NULL,
  device_id          VARCHAR(64)  NOT NULL,
  device_name        VARCHAR(128) NOT NULL DEFAULT '',
  refresh_token_hash CHAR(64)     NOT NULL UNIQUE,
  expires_at         DATETIME(3)  NOT NULL,
  revoked_at         DATETIME(3)  NULL,
  last_used_at       DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  created_at         DATETIME(3)  NOT NULL DEFAULT CURRENT_TIMESTAMP(3),
  INDEX idx_sessions_user (user_id),
  CONSTRAINT fk_sessions_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS tasks (
  user_id        VARCHAR(36)  NOT NULL,
  id             VARCHAR(36)  NOT NULL,
  description    TEXT         NOT NULL,
  type           VARCHAR(32)  NOT NULL,
  status         VARCHAR(32)  NOT NULL,
  created_at     VARCHAR(32)  NOT NULL,
  completed_at   VARCHAR(32)  NULL,
  reminder_time  VARCHAR(32)  NULL,
  updated_at     VARCHAR(32)  NULL,
  deleted_at     VARCHAR(32)  NULL,
  sticky_content MEDIUMTEXT   NOT NULL,
  sticky_pos_x   DOUBLE       NOT NULL DEFAULT 48,
  sticky_pos_y   DOUBLE       NOT NULL DEFAULT 76,
  sticky_is_open TINYINT(1)   NOT NULL DEFAULT 0,
  sticky_width   DOUBLE       NOT NULL DEFAULT 420,
  sticky_height  DOUBLE       NOT NULL DEFAULT 300,
  tags           VARCHAR(512) NOT NULL DEFAULT '',
  priority       INT          NOT NULL DEFAULT 0,
  due_at         VARCHAR(32)  NULL,
  sticky_color   VARCHAR(32)  NOT NULL DEFAULT '',
  sort_order     DOUBLE       NULL,
  project        VARCHAR(64)  NOT NULL DEFAULT '',
  urgent         TINYINT(1)   NOT NULL DEFAULT 0,
  server_seq     BIGINT       NOT NULL DEFAULT 0,
  PRIMARY KEY (user_id, id),
  INDEX idx_tasks_user_seq (user_id, server_seq),
  CONSTRAINT fk_tasks_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS recurring_tasks (
  user_id           VARCHAR(36)  NOT NULL,
  id                VARCHAR(36)  NOT NULL,
  description       TEXT         NOT NULL,
  type              VARCHAR(32)  NOT NULL,
  status            VARCHAR(32)  NOT NULL,
  created_at        VARCHAR(32)  NOT NULL,
  completed_at      VARCHAR(32)  NULL,
  interval_minutes  INT          NOT NULL,
  last_triggered    VARCHAR(32)  NULL,
  next_trigger      VARCHAR(32)  NOT NULL,
  is_paused         TINYINT(1)   NOT NULL DEFAULT 0,
  start_time        VARCHAR(32)  NULL,
  end_time          VARCHAR(32)  NULL,
  updated_at        VARCHAR(32)  NULL,
  deleted_at        VARCHAR(32)  NULL,
  repeat_mode       VARCHAR(32)  NOT NULL DEFAULT 'INTERVAL_RANGE',
  schedule_time     VARCHAR(16)  NULL,
  schedule_weekday  INT          NULL,
  schedule_day      INT          NULL,
  cron_expression   VARCHAR(128) NULL,
  schedule_weekdays INT          NULL,
  tags              VARCHAR(512) NOT NULL DEFAULT '',
  ends_on           VARCHAR(16)  NULL,
  remaining_count   INT          NULL,
  temp_trigger      VARCHAR(32)  NULL,
  temp_replaces     VARCHAR(32)  NULL,
  server_seq        BIGINT       NOT NULL DEFAULT 0,
  PRIMARY KEY (user_id, id),
  INDEX idx_recurring_user_seq (user_id, server_seq),
  CONSTRAINT fk_recurring_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;

CREATE TABLE IF NOT EXISTS reminder_records (
  user_id      VARCHAR(36) NOT NULL,
  id           VARCHAR(36) NOT NULL,
  reminder_id  VARCHAR(36) NOT NULL,
  description  TEXT        NOT NULL,
  type         VARCHAR(32) NOT NULL,
  trigger_time VARCHAR(32) NOT NULL,
  close_time   VARCHAR(32) NULL,
  action       VARCHAR(32) NOT NULL,
  updated_at   VARCHAR(32) NULL,
  deleted_at   VARCHAR(32) NULL,
  server_seq   BIGINT      NOT NULL DEFAULT 0,
  PRIMARY KEY (user_id, id),
  INDEX idx_records_user_seq (user_id, server_seq),
  CONSTRAINT fk_records_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_ai_ci;
