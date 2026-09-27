# 仓库指南

## 项目结构与模块组织
- `src/` 存放 Vue 3 + TypeScript 前端，共三个窗口入口：
  - 主窗口：`index.html` → `src/main.ts` → `src/App.vue`（只负责外壳：标题栏、侧边栏、按 Tab 切换视图、挂载共享弹窗与全局事件监听）。
    - `src/views/`：每个 Tab 一个视图——`TodayView`（今天时间线）、`CalendarView`（日历月视图 / 周视图，可拖动待办改期到某天或某个时段）、`TasksView`（含搜索、标签/优先级筛选与排序）、`CompletedView`、`RecurringView`、`RecordsView`、`StatsView`（统计）、`TrashView`（回收站）。Tab 键定义在 `src/navigation.ts`，上次打开的 Tab 记在 `localStorage.activeTab`。
    - `src/composables/`：模块级单例的共享状态——`useAppData`（四个列表、`refreshAll`、`dataVersion`）、`useSettings`（设置草稿、同步状态、设置/云同步弹窗开关）、`useUpdater`、`useUiPrefs`（主题、缩放、透明度、侧边栏）、`useDialogs`（确认框、详情、编辑待办/循环提醒）、`useItemActions`（完成、删除、右键菜单等通用操作）、`useSmartAdd`（带自然语言识别的新建待办/循环提醒）、`useContextMenu`、`usePagination`、`useNow`。
  - 提醒弹窗：`notification.html` → `src/notification.ts` → `src/NotificationApp.vue`。
  - 桌面便签：`sticky-note-item.html` → `src/stickyNoteItem.ts` → `src/StickyNoteItemApp.vue`（每张便签一个独立窗口，窗口标签为 `sticky-note-item-<编码后的 id>`）。
  - 快速添加：`quick-add.html` → `src/quickAdd.ts` → `src/QuickAddApp.vue`（窗口标签 `quick-add`，由全局快捷键或托盘菜单打开）。
- `src/components/` 存放可复用 UI 组件：
  - `MarkdownNoteEditor.vue`：基于 Milkdown Crepe 的 Markdown 所见即所得编辑器，`variant` 支持 `card`（带边框）与 `ghost`（无边框，嵌入卡片或便签）。
  - `Modal.vue`：通用弹窗（带进出过渡动画）。
  - `AppTitlebar.vue` / `AppSidebar.vue`：主窗口标题栏与侧边栏。
  - `SettingsModal.vue` / `WebdavModal.vue`：应用设置与云同步设置。
  - `SharedDialogs.vue`：挂在 App.vue 的共享弹窗（`TaskEditModal`、`RecurringEditModal`、详情、确认框、右键菜单），由 `useDialogs` 驱动，任何视图都可打开。
  - `RecurringFields.vue`：循环提醒各模式的规则字段，新建表单与编辑弹窗共用。
  - `Pagination.vue`：表格分页条，配合 `usePagination`。
  - `WeekdayPicker.vue`：每周多天选择（位掩码 `v-model`，含工作日/周末/每天预设）。
  - `TaskBadges.vue`（优先级与标签徽标）、`TagInput.vue`（标签输入）、`PriorityPicker.vue`（优先级分段选择）、`SmartParseHint.vue`（自然语言识别结果提示）。
  - `DataModal.vue`：导入导出与本地备份（从设置打开）。
- `src/api.ts` 封装所有 Tauri `invoke` 命令；`src/types.ts` 为前后端共享的数据类型。
- `src/markdown.ts` Markdown 转纯文本/预览文本工具（列表描述、提醒记录去掉前导列表标记）。
- `src/format.ts` 主窗口共用的时间与文案格式化；`src/recurring.ts` 循环规则展示、表单草稿、校验与提交载荷。
- `src/timeline.ts`（“今天”时间线）、`src/stats.ts`（统计面板）、`src/calendar.ts`（日历网格与按天归类）、`src/tasks.ts`（标签、优先级、筛选排序）、`src/nlp.ts`（自然语言时间/标签/优先级/循环规则识别）为纯函数，测试在同名 `*.spec.ts`。
- `src/syncStatus.ts` 云同步状态码到文案与色调的映射（兼容旧版中文状态）；`src/quietHours.ts` 勿扰时段说明文案。
- `src/safeStorage.ts` 带异常保护的 `localStorage` 封装；`src/startupError.ts` 启动失败时渲染错误页。
- `src/styles.css` 为三个窗口共用的全局样式表（设计令牌、主窗口、提醒弹窗、便签）；组件私有样式放在 `.vue` 文件内。
- `src/update.ts` 自动更新逻辑模块（检查更新、安装更新、偏好管理）。
- `src/weekdays.ts` 每周位掩码工具（与后端一致：周一 = bit0 … 周日 = bit6）；`src/shortcut.ts` 全局快捷键录制与展示。
- `src-tauri/` 存放 Tauri 应用的 Rust 后端。
  - `src-tauri/src/` 为 Rust 应用代码：`main.rs`（插件、命令注册与启动流程）、`commands/`（前端 `invoke` 的命令，按业务分为 `tasks`、`recurring`、`notification`、`trash`、`settings`、`sticky`、`system`、`data`，公共的 `ApiResult` / `into_api` 在 `commands/mod.rs`）、`windows/`（窗口事件；`windows/sticky.rs` 为便签窗口的标签编码、创建显示、层级与 UI 状态注入）、`db.rs`（SQLite 读写）、`scheduler.rs`（提醒调度与弹窗）、`recurrence.rs`（循环规则计算）、`sync.rs`（WebDAV 同步）、`sync_crypto.rs`（同步端到端加密：Argon2id + AES-256-GCM）、`notification_queue.rs`（提醒弹窗队列）、`time.rs`（本地时间格式化与解析）、`holidays.rs`（中国法定节假日与调休）、`quick_add.rs`（快速添加窗口）、`shortcuts.rs`（全局快捷键统一注册：快速添加、显示/隐藏全部便签）、`quiet_hours.rs`（勿扰时段判断）、`backup.rs`（JSON/Markdown/ICS 导出、JSON 导入合并、每日本地备份）、`tray.rs`（托盘菜单与提示：下一条提醒、完成/推迟、显示/隐藏全部便签）、`autostart.rs`、`single_instance.rs`、`paths.rs`（数据目录）、`models.rs`、`state.rs`、`errors.rs`、`maintenance.rs`（定期清理与优化）。
  - `src-tauri/migrations/` 存放数据库迁移文件；新增迁移后需在 `db.rs` 的 `migration_scripts()` 中登记，并同步 `sync.rs` 的列清单与 `ensure_sync_columns`。
  - `src-tauri/data/holidays-cn.json` 为内置法定节假日数据（`off` 放假日、`work` 调休上班日，支持 `[开始, 结束]` 区间），每年国务院发布次年安排后追加，并补充 `holidays.rs` 中的测试。
  - `src-tauri/icons/` 存放应用图标；源文件在 `src-tauri/icons/source/`（`icon.svg` 为主图标，`icon-small.svg` 为 16–32px 简化版），修改后执行 `python3 src-tauri/icons/source/render.py` 重新生成 PNG 与 `icon.ico`（需 `pip install cairosvg pillow`）。
  - `src-tauri/capabilities/default.json` 定义各窗口的权限。
  - `src-tauri/tauri.conf.json` 定义窗口、打包、更新器与应用元数据；`src-tauri/tauri.updater.conf.json` 为签名构建时的覆盖配置。
- `docs/ROADMAP.md` 为功能扩展与重构路线图，完成条目后同步勾选并补充变更记录。
- `.github/workflows/ci.yml` 为 CI（Ubuntu + Windows：`pnpm build`、`pnpm test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`）。
- `scripts/` 存放构建与发布脚本。
  - `scripts/build-updater.ps1` 签名构建 MSI + 生成更新清单。
  - `scripts/write-updater-manifest.mjs` 生成 `latest.json` 更新清单。
  - `scripts/tauri.mjs` Tauri CLI 包装器（处理 VS Dev Shell 环境）。

## 构建、测试与开发命令
- `pnpm dev`：启动 Web UI 的 Vite 开发服务器。
- `pnpm build`：先执行 `vue-tsc --noEmit` 类型检查，再将前端打包到 `dist/`。
- `pnpm typecheck`：仅做类型检查。
- `pnpm test`：运行 Vitest 前端单元测试（`src/**/*.spec.ts`）。
- `pnpm preview`：本地预览生产构建。
- `pnpm tauri dev`：以开发模式运行完整的 Tauri 桌面应用。
- `pnpm tauri build`：生成生产环境桌面应用包，不生成 updater 签名产物。
  - 若 `pnpm tauri dev` 仍显示旧界面，可先执行 `pnpm build` 再运行 `pnpm tauri dev`，避免回退到过期的 `dist/`。
- `pnpm release:updater`：执行完整的更新器构建流程（签名 + MSI + latest.json）。

## 版本管理
- 版本号需在三处同步修改：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`。
- 发布更新时，GitHub Release 需上传三个文件：`.msi`、`.msi.sig`、`latest.json`。

## 自动更新发布流程
1. 修改三处版本号
2. 构建：推荐直接执行 `pnpm release:updater`；若手动构建，则需设置 `TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 后运行 `pnpm tauri build --bundles msi --config src-tauri/tauri.updater.conf.json`
3. 生成清单：`pnpm release:updater` 会自动生成；若手动构建，则执行 `node scripts/write-updater-manifest.mjs`
4. 发布：`gh release create v{version}` 上传 MSI + .sig + latest.json
5. 签名私钥位于 `~/.tauri/taskReminder-updater.key`

也可以交给 CI（`.github/workflows/release.yml`）：改好三处版本号、写好 `docs/release-notes/v{version}.md` 后推送 `v{version}` tag，工作流会检查版本号（`scripts/check-release-version.mjs`）、签名构建 MSI、生成 `latest.json` 并上传到**草稿** Release，检查无误后在 GitHub 上点“Publish release”。需要先在仓库 Secrets 中配置 `TAURI_SIGNING_PRIVATE_KEY`（私钥文件内容）与 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。

## 编码风格与命名约定
- 缩进：`.vue`、`.ts`、`.css` 使用 2 个空格（保持现有格式）。
- Vue 组件文件名使用 `PascalCase`（如 `NotificationApp.vue`）。
- TypeScript 标识符：函数/变量用 `camelCase`，类型用 `PascalCase`。
- CSS 类名使用 `kebab-case`（如 `.titlebar-actions`）。

## UI 样式约定（v1.5.6 起）
- **颜色一律使用 `src/styles.css` 中 `:root` 定义的设计令牌**（如 `--bg-surface`、`--text-muted`、`--primary`、`--primary-soft`、`--primary-text`、`--success-*`、`--warning-*`、`--danger-*`、`--violet-*`），不要在组件里写死颜色。
- `:root` 为深色主题；浅色主题在文件末尾的 `.light-theme { ... }` 中覆盖同名令牌。新增令牌时两处都要补齐。
- 文字层级：`--text-title`（标题/强调）> `--text-base`（正文）> `--text-muted`（次要）> `--text-faint`（占位、空值）。
- 字体统一使用 `var(--font-sans)`，不要引用未随应用打包的 Web 字体（如 Fraunces、Inter、DM Sans），否则 Windows 上会回退为衬线字体。
- 常用组件类：
  - 表格单元格：`.cell-title`（加粗标题）、`.cell-muted`（次要描述）、`.cell-time`（等宽数字时间）、`.cell-empty`（空值）。
  - 标签：`.chip`（类型/模式，`.is-task` / `.is-recurring`）、`.status-pill`（带圆点状态，`.is-running` / `.is-paused` / `.action-*`）、`.time-chip`（提醒时间，`.is-upcoming` / `.is-overdue`）。
  - 表格空状态：在 `tbody` 末尾加 `tr.table-empty-row > td[colspan] > .table-empty`（含 `.table-empty-title` 与 `.table-empty-hint`）。
  - 容器：`.table-card`（表格卡片）、`.composer-card`（新建任务卡片）、`.form-card`（表单卡片）、`.search-field`（带图标搜索框）。
  - 复选框已全局自定义样式；圆形“完成”勾选框使用 `.check-round`。
- `select` 已全局去掉原生外观并使用内联 SVG 箭头；浅色主题的箭头颜色在 `.light-theme .select` 中单独覆盖。
- 修改样式后需同时检查深色与浅色主题，以及侧边栏展开/收起、窗口宽度小于 980px（侧边栏变为横向标签栏）三种状态。
- 统计卡片：`.stat-row > .stat-tile`（`.stat-label`、`.stat-value`、`.stat-hint`）；卡片容器 `.stats-card`；分段选择 `.segmented > .segmented-item.active`。
- 统计图表颜色使用 `--chart-completed` / `--chart-dismissed` / `--chart-snoozed` / `--chart-pending`（深浅主题分别校验过色觉辨识度），图表必须带图例，并提供数据表视图。
- Rust 代码使用标准 `snake_case` 的模块与函数命名；用 `cargo fmt`（默认 rustfmt）格式化。

## 踩坑记录与注意事项

### Vue Proxy 与 Tauri 插件私有字段冲突
- **问题**：`@tauri-apps/plugin-updater` 的 `Update` 类使用了 JS 私有字段（`#field`），存入 Vue `ref()` 后被 Proxy 深度包装，调用 `downloadAndInstall()` 时报 `Cannot read private member from an object whose class did not declare it`。
- **解决**：使用 `shallowRef` 代替 `ref` 存储 Tauri 插件返回的类实例对象，避免 Vue 深度代理。
- **规则**：**凡是 Tauri 插件返回的类实例（如 `Update`、`Channel` 等），一律使用 `shallowRef` 而非 `ref` 存储。**

### github release / GitHub Release 更新发布与签名构建
- **关键词**：github release、GitHub Release、Tauri updater、MSI、latest.json、TAURI_SIGNING_PRIVATE_KEY。
- 发布更新时，先同步三处版本号：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`。
- 普通 `pnpm tauri build` 不会生成 updater 签名产物，因此不要求设置 `TAURI_SIGNING_PRIVATE_KEY`。
- PowerShell 推荐直接执行 `pnpm release:updater`。该脚本会读取 `~/.tauri/taskReminder-updater.key`，用 `.Trim()` 清除签名私钥尾部换行，设置 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""` 避免交互等待，并通过 `cmd /c "pnpm.cmd tauri build --bundles msi --config src-tauri/tauri.updater.conf.json"` 确保子进程继承签名环境变量。
- 构建产物位于 `src-tauri/target/release/bundle/msi/`，GitHub Release 必须上传三个文件：`TaskReminderApp_{version}_x64_zh-CN.msi`、`TaskReminderApp_{version}_x64_zh-CN.msi.sig`、`latest.json`。
- 创建 GitHub Release 时直接使用当前版本号执行：`gh release create v{version} src-tauri/target/release/bundle/msi/TaskReminderApp_{version}_x64_zh-CN.msi src-tauri/target/release/bundle/msi/TaskReminderApp_{version}_x64_zh-CN.msi.sig src-tauri/target/release/bundle/msi/latest.json --title "v{version}" --notes "{release notes}"`。
- 在 bash 中构建时，直接 `export TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 后调用 `pnpm tauri build --bundles msi --config src-tauri/tauri.updater.conf.json`，再运行 `node scripts/write-updater-manifest.mjs` 生成 `latest.json`。

### Linux 下便签窗口必须使用 App URL
- **问题**：开发模式下便签窗口曾使用 `WebviewUrl::External(localhost)`，Linux WebKitGTK 会将其视为远程源并拒绝 `invoke`，导致便签无法保存。
- **解决**：`sticky_note_item_url()` 在所有平台都返回 `WebviewUrl::App("sticky-note-item.html")`，`tauri dev` 会自动改写为 `build.devUrl`，与主窗口同源；`windows/sticky.rs` 中有对应单元测试守护。
- **规则**：新增的 Tauri 窗口一律使用 `WebviewUrl::App(...)`，不要为开发模式单独改用 External URL。

### Linux 下透明便签窗口不重绘
- **问题**：Linux WebKitGTK 的透明窗口在首帧后经常不重绘，便签一直停在“载入便签...”。
- **解决**：仅在 Linux 关闭便签窗口透明，并通过初始化脚本直接注入便签数据；Windows 仍保持透明浮动外观。Linux 下相关样式挂在 `html.sticky-note-mode.is-linux` 上。

### 提醒弹窗队列与巡检
- 提醒由后端 `NotificationQueue` 维护，弹窗通过 `get_notification_queue` 读取、监听 `notification-queue` 事件更新，始终展示队首；`ack_notification` / `snooze_notification` 返回剩余队列，队列为空时后端隐藏弹窗。
- **规则**：新增触发提醒的路径一律走 `ReminderScheduler::handle_task` / `handle_recurring`，它们持有 `fire_lock` 并按提醒记录判重，保证计时器、30 秒巡检（`fire_due`）重复命中时不会重复弹出。不要绕过它直接写提醒记录再弹窗。
- 一次性提醒只补发 7 天内错过的（`MISSED_REMINDER_LOOKBACK_DAYS`）。

### 删除与清理必须走墓碑
- **问题**：直接 `DELETE` 行后，远端库仍有该行，同步合并会把它重新插回本地（“复活”）。
- **规则**：业务删除与定期清理一律写 `deleted_at` + `updated_at`（墓碑）；只有 `purge_expired_tombstones` 按保留期（本地 7 天，开启同步 60 天）物理删除，同步在合并后、上传前也会调用它。
- 回收站（`list_trash`）只列出保留期内的墓碑。恢复即清除 `deleted_at`；恢复循环提醒时从当前时间重新计算下次触发。
- 回收站的“永久删除”（`purge_trash` → `expire_tombstones`）不能直接 `DELETE`：把 `deleted_at` 改为 `EXPIRED_TOMBSTONE_TIME` 并刷新 `updated_at`，让本地行在合并中胜出，再由清理物理删除（未开启同步时立即清理）。

### 循环模式的兼容性
- 每周多天存于 `schedule_weekdays` 位掩码，`schedule_weekday` 始终写入掩码中最早的一天，供旧版本读取；读取时掩码为空则回退到 `schedule_weekday`。
- 新增循环模式时，旧版本的 `normalize_repeat_mode` 会把未知模式回退为区间间隔并可能同步回来，需在发布说明中提示多设备同时升级。

### 标签与优先级
- 标签存于 `tasks.tags`（逗号分隔），读写统一经过 `models::normalize_tags`（去掉 `#`、逗号，不区分大小写去重，最多 10 个、每个 24 字）；前端 `src/tasks.ts` 的 `normalizeTags` 与之保持一致。
- `update_task` 的 `tags` / `priority` 省略时保留原值：稍后提醒、日历拖动改期等只改时间的调用不要传这两个字段。

### 导入导出与备份
- 文件对话框由后端 `tauri-plugin-dialog` 弹出（`export_data` / `import_data` 为 async 命令，阻塞对话框不能在主线程等待），前端不传文件路径；因此不需要为对话框添加前端权限。
- 导入、备份恢复都按“较新的 `updated_at` 胜出”合并（导入走 `DbManager::import_rows`，恢复复用 `sync::merge_databases`），之后调用 `schedule_existing` 并 `notify_local_change`。
- 备份文件名固定为 `taskreminder-YYYYMMDD-HHMMSS.db`，`backup::resolve_backup` 只接受这种名字，防止路径穿越。

### 同步状态码
- `settings.webdav_last_sync_status` 存状态码（`sync::SyncState`：`never` / `syncing` / `success` / `first_sync` / `lock_busy` / `failed`），`get_status` 与 `sync-status` 事件也返回状态码；文案只在前端 `src/syncStatus.ts` 映射。
- **规则**：不要再按中文文案判断同步状态；新增状态时同时更新 `SyncState::parse` 与 `syncStatus.ts`。读取时兼容 2.0.1 之前存的中文文案。

### Cron 表达式
- `recurrence::cron_schedule_expr` 把 5 段表达式转换为 `cron` crate 的 6 段格式：周字段按标准 Unix 含义（0/7 = 周日），数字周几会换成英文缩写，因为 `cron` crate 的数字周几是 1 = 周日。6、7 段原样透传。
- 库里保存用户输入的原始表达式，转换只在计算时进行。

### 密码存放（凭据库）
- WebDAV 密码与同步密码在 Windows 上存凭据管理器（`secrets.rs`，服务名 `TaskReminderApp`，开发构建为 `TaskReminderApp-dev`，条目名 `{设备 ID}:{字段}`），数据库中留空，`settings.secret_storage` 记为 `keyring`；其他平台或写入失败时存数据库（`db`）。
- 读写都经 `DbManager::load_settings` / `save_settings`，调用方拿到的 `AppSettings` 里总是明文密码。密码不变时 `save_settings` 不会写凭据库；凭据库读取失败时不会用空值覆盖。
- 测试中的 `DbManager::new` 不访问系统凭据库；需要时用 `DbManager::with_secret_store` 注入 `secrets::MemoryStore`。
- 新增敏感设置时同样经 `resolve_secret_storage` 处理，并在 `export_local_snapshot_bytes` 中清空。

### 同步端到端加密
- 远端文件：明文为 `taskreminder.db`；开启加密后为 `taskreminder.db.enc`，同时把 `taskreminder.db` 换成以 `TaskReminder-Encrypted-Placeholder` 开头的占位说明（旧版本把它当数据库合并会失败，从而不会上传明文）。
- `sync_with_remote` 的顺序不能随意调整：先判断远端是否加密（未开启加密却遇到加密文件、或解密失败都直接报错，**绝不上传**），合并后先传 `.enc` 再替换明文文件。
- 远端存储经 `RemoteStore` 接口访问，测试用内存实现（`sync.rs` 测试中的 `MemoryStore`）覆盖首次同步、明文转加密、密码错误等流程；测试使用 `sync_crypto::TEST_PARAMS` 的小 KDF 参数。
- 上传的快照由 `export_local_snapshot_bytes` 生成并清空 `webdav_password` 与 `sync_passphrase`；新增敏感设置时要一并清空。
- 同步密码存于本机 `settings.sync_passphrase`（settings 表不参与合并）；`save_settings` 广播给其他窗口的设置会清空两个密码。
- 开发构建对 `argon2`/`blake2` 单独开启优化（`Cargo.toml` 的 `profile.dev.package`），否则每次同步派生密钥要数秒。

### 全局快捷键
- 快捷键由 `shortcuts::apply_shortcuts` 统一注册（先 `unregister_all` 再逐个注册：快速添加、显示/隐藏全部便签），启动时与保存设置后调用；某个失败不影响其他，原因合并后通过 `apply_quick_add_shortcut` 命令返回给设置界面。失败（格式无效、被其他程序占用、Wayland 不支持等）不能阻止应用启动。
- 新增全局快捷键时加到 `apply_shortcuts` 中，不要在别处单独注册，否则会被 `unregister_all` 清掉。

### 勿扰时段、系统通知与托盘
- 提醒入队后一律经 `ReminderScheduler::present` 展示：勿扰时段内只更新已打开的弹窗、不主动弹出，并记下 `quiet_held`；巡检中的 `release_quiet_hold` 在勿扰结束后弹出积压的提醒。系统原生通知（`tauri-plugin-notification`）也在这里按设置发送，勿扰期间不发。
- 勿扰、系统通知、便签快捷键都是本机设置（`settings` 表，迁移 `V2.0.2`），不参与同步。
- 托盘通过 `listen_any("data-updated")` 与每 30 秒的定时器刷新，刷新在后台线程执行（`tray::request_refresh`），不要在持有锁时同步调用菜单 API。托盘中的“完成 / 推迟”复用 `commands::tasks::complete_task_by_id` / `reschedule_task_reminder`。
- “隐藏全部便签”只隐藏窗口，不写 `sticky_is_open`，避免产生同步改动。

### onMounted 中异步操作的异常隔离
- **问题**：多个异步操作放在同一个 `try` 块中，前面的操作抛异常会导致后面的操作被跳过（如自动更新检查被数据初始化异常阻断）。
- **解决**：将相互独立的异步操作放在各自独立的 `try/catch` 块中。
- **规则**：`onMounted` 中多个独立的异步初始化操作应分别用 `try/catch` 包裹，互不影响。

## 测试指南
- 前端使用 Vitest：测试与被测模块同目录，命名为 `*.spec.ts`，运行 `pnpm test`。前端改动至少执行 `pnpm build`（含 `vue-tsc` 类型检查）与 `pnpm test`。
- 视图里的计算逻辑（如时间线、统计）优先抽成 `src/` 下的纯函数再写测试，组件只做展示。
- Rust 测试位于 `src-tauri/src/` 各模块的 `#[cfg(test)]` 中（便签窗口标签/URL、提醒队列、墓碑清理、同步合并、时间解析、节假日、循环规则），通过 `cargo test` 运行；需要数据库的测试用临时目录创建 `DbManager`，会自动执行迁移。
- 提交前运行 `cargo fmt` 与 `cargo clippy --all-targets -- -D warnings`，CI 会执行 `cargo fmt --check` 并在 clippy 有告警时失败。只在 Windows 编译的代码（`#[cfg(target_os = "windows")]`）在 Linux 上检查不到，可用 `rustup target add x86_64-pc-windows-gnu`（需 `mingw-w64`）后执行 `cargo clippy --target x86_64-pc-windows-gnu --all-targets` 预检。
- 在 Linux 上构建会改写 `src-tauri/gen/schemas/`，这些生成文件的无关变动不要提交。
- 仅调整前端 UI 时，可用 `pnpm dev` 在浏览器中预览；浏览器中没有 Tauri 运行时，需要在页面加载前注入 `window.__TAURI_INTERNALS__`（模拟 `invoke`、`transformCallback`、`metadata.currentWindow`）并返回示例数据，否则列表为空。

## 提交与合并请求指南
- 提交信息使用简短祈使句，例如：`feat: 新增托盘开关`、`fix: 修复更新安装失败`。
- PR 需包含：简要摘要、测试步骤，以及 UI 变更截图。

## 配置与资源
- 在 `src-tauri/tauri.conf.json` 中更新窗口行为、打包标识符与应用元数据。
- 保持静态 HTML 入口文件（`index.html`、`notification.html`、`quick-add.html`、`sticky-note-item.html`）简洁，并与 Vue 入口保持同步；新增入口时同时更新 `vite.config.ts` 的多页面配置。
- 更新器配置在 `src-tauri/tauri.conf.json` 的 `plugins.updater` 节点，包含公钥和 GitHub Releases 端点。
