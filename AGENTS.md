# 仓库指南

## 项目结构与模块组织
- `src/` 存放 Vue 3 + TypeScript 前端，共三个窗口入口：
  - 主窗口：`index.html` → `src/main.ts` → `src/App.vue`（待办、已办、循环提醒、提醒记录、设置、云同步）。
  - 提醒弹窗：`notification.html` → `src/notification.ts` → `src/NotificationApp.vue`。
  - 桌面便签：`sticky-note-item.html` → `src/stickyNoteItem.ts` → `src/StickyNoteItemApp.vue`（每张便签一个独立窗口，窗口标签为 `sticky-note-item-<编码后的 id>`）。
- `src/components/` 存放可复用 UI 组件：
  - `MarkdownNoteEditor.vue`：基于 Milkdown Crepe 的 Markdown 所见即所得编辑器，`variant` 支持 `card`（带边框）与 `ghost`（无边框，嵌入卡片或便签）。
  - `Modal.vue`：通用弹窗（带进出过渡动画）。
- `src/api.ts` 封装所有 Tauri `invoke` 命令；`src/types.ts` 为前后端共享的数据类型。
- `src/markdown.ts` Markdown 转纯文本/预览文本工具（列表描述、提醒记录去掉前导列表标记）。
- `src/safeStorage.ts` 带异常保护的 `localStorage` 封装；`src/startupError.ts` 启动失败时渲染错误页。
- `src/styles.css` 为三个窗口共用的全局样式表（设计令牌、主窗口、提醒弹窗、便签）；组件私有样式放在 `.vue` 文件内。
- `src/update.ts` 自动更新逻辑模块（检查更新、安装更新、偏好管理）。
- `src-tauri/` 存放 Tauri 应用的 Rust 后端。
  - `src-tauri/src/` 为 Rust 应用代码：`main.rs`（命令注册、窗口管理、便签窗口）、`db.rs`（SQLite 读写）、`scheduler.rs`（提醒调度与弹窗）、`recurrence.rs`（循环规则计算）、`sync.rs`（WebDAV 同步）、`tray.rs`（托盘菜单）、`autostart.rs`、`single_instance.rs`、`paths.rs`（数据目录）、`models.rs`、`state.rs`、`errors.rs`、`maintenance.rs`。
  - `src-tauri/migrations/` 存放数据库迁移文件。
  - `src-tauri/icons/` 存放应用图标。
  - `src-tauri/capabilities/default.json` 定义各窗口的权限。
  - `src-tauri/tauri.conf.json` 定义窗口、打包、更新器与应用元数据；`src-tauri/tauri.updater.conf.json` 为签名构建时的覆盖配置。
- `scripts/` 存放构建与发布脚本。
  - `scripts/build-updater.ps1` 签名构建 MSI + 生成更新清单。
  - `scripts/write-updater-manifest.mjs` 生成 `latest.json` 更新清单。
  - `scripts/tauri.mjs` Tauri CLI 包装器（处理 VS Dev Shell 环境）。

## 构建、测试与开发命令
- `pnpm dev`：启动 Web UI 的 Vite 开发服务器。
- `pnpm build`：将前端打包到 `dist/`。
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
- **解决**：`sticky_note_item_url()` 在所有平台都返回 `WebviewUrl::App("sticky-note-item.html")`，`tauri dev` 会自动改写为 `build.devUrl`，与主窗口同源；`main.rs` 中有对应单元测试守护。
- **规则**：新增的 Tauri 窗口一律使用 `WebviewUrl::App(...)`，不要为开发模式单独改用 External URL。

### Linux 下透明便签窗口不重绘
- **问题**：Linux WebKitGTK 的透明窗口在首帧后经常不重绘，便签一直停在“载入便签...”。
- **解决**：仅在 Linux 关闭便签窗口透明，并通过初始化脚本直接注入便签数据；Windows 仍保持透明浮动外观。Linux 下相关样式挂在 `html.sticky-note-mode.is-linux` 上。

### onMounted 中异步操作的异常隔离
- **问题**：多个异步操作放在同一个 `try` 块中，前面的操作抛异常会导致后面的操作被跳过（如自动更新检查被数据初始化异常阻断）。
- **解决**：将相互独立的异步操作放在各自独立的 `try/catch` 块中。
- **规则**：`onMounted` 中多个独立的异步初始化操作应分别用 `try/catch` 包裹，互不影响。

## 测试指南
- `package.json` 尚未配置 JavaScript 测试框架；前端改动至少执行 `pnpm build` 确认可编译。
- Rust 测试位于 `src-tauri/src/` 各模块的 `#[cfg(test)]` 中（如 `main.rs` 的便签窗口标签/URL 测试），通过 `cargo test` 运行。
- 仅调整前端 UI 时，可用 `pnpm dev` 在浏览器中预览；浏览器中没有 Tauri 运行时，需要在页面加载前注入 `window.__TAURI_INTERNALS__`（模拟 `invoke`、`transformCallback`、`metadata.currentWindow`）并返回示例数据，否则列表为空。
- 若引入 JS 测试框架，请将测试放在 `src/` 下（如 `*.spec.ts`），并在 `package.json` 中添加脚本。

## 提交与合并请求指南
- 提交信息使用简短祈使句，例如：`feat: 新增托盘开关`、`fix: 修复更新安装失败`。
- PR 需包含：简要摘要、测试步骤，以及 UI 变更截图。

## 配置与资源
- 在 `src-tauri/tauri.conf.json` 中更新窗口行为、打包标识符与应用元数据。
- 保持静态 HTML 入口文件（`index.html`、`notification.html`、`sticky-note-item.html`）简洁，并与 Vue 入口保持同步；新增入口时同时更新 `vite.config.ts` 的多页面配置。
- 更新器配置在 `src-tauri/tauri.conf.json` 的 `plugins.updater` 节点，包含公钥和 GitHub Releases 端点。
