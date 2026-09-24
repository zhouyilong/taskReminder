# Task Reminder (Tauri + Vue 3)

## Recent Fix Notes
### v1.5.6 UI refresh
- Titlebar: app logo, standalone version label, and a colored sync-status pill (success / error / syncing) with a hover tooltip showing sync time and last error.
- Sidebar: soft highlight + left indicator for the active tab, count badges for pending tasks and running recurring reminders; becomes a horizontal tab bar below 980px width.
- Tasks: title + Markdown description merged into one composer card (press Enter to add; the description area expands on focus), leaving more room for the list.
- Tables: tighter rows, colored reminder-time chips (upcoming / overdue), status and type pills, and empty states for every list.
- Global styling is driven by CSS design tokens in `src/styles.css` (dark in `:root`, light in `.light-theme`); see “UI 样式约定” in `AGENTS.md`.
- Sticky-note titles now use the system sans-serif stack instead of an unbundled web font that fell back to a serif face on Windows.

### Sticky notes failed to save / stuck loading on Linux
- Symptom: on Linux, sticky-note windows stayed on “载入便签...”, or opened but could not save.
- Root cause: WebKitGTK often stops repainting transparent windows after the first frame; and in dev mode the sticky-note window used an External `localhost` URL, which WebKitGTK treats as a remote origin and denies IPC `invoke`.
- Fix: disable transparency for sticky-note windows on Linux only and inject the note data via an initialization script; load sticky-note windows with `WebviewUrl::App("sticky-note-item.html")` on every platform (Tauri rewrites it to `build.devUrl` in dev), guarded by a unit test in `main.rs`.
- Rule: every Tauri window should use `WebviewUrl::App(...)`; do not special-case dev mode with External URLs.

### Sticky notes not following main window scale/theme
- Symptom: after changing the main window zoom or theme, already-open sticky note windows did not update.
- Root cause: cross-window UI sync relied on a single event path, which was not stable enough across different webview/runtime paths.
- Fix:
  - unified `uiScale`, `theme`, and `windowOpacity` into one UI state payload;
  - broadcast from the main window, keep a backend snapshot, and re-send when sticky note windows are shown;
  - add a final fallback that injects the current UI state directly into sticky-note webviews and applies it via a browser custom event.
- Rule: for cross-window UI sync, keep at least “runtime broadcast + new-window replay + final fallback” instead of relying on a single event path.

### Sticky note pinning was being reset
- Symptom: clicking the pin button appeared to do nothing, or pinning was lost after opening/reordering sticky notes.
- Root cause: the pin state lived only on the front end while the Rust window-layer logic kept reapplying sticky-note z-order, so show/reorder/restart paths could overwrite or forget the pinned state.
- Fix: make the Rust backend the source of truth, persist `sticky_is_pinned` in the database, and always re-apply the correct top-most/bottom-most layer from that stored state when a sticky note is pinned, shown, or reordered.
- Rule: pinning-related window behavior must be backend-owned and persisted; do not rely on a front-end-only `always_on_top(true)` call for durable sticky-note pin state.

一个基于 Tauri + Vue 3 + TypeScript 的桌面任务提醒应用，包含主窗口、提醒弹窗与桌面便签窗口。

当前技术栈：**Tauri 2 + Vue 3 + TypeScript**，当前版本 **v1.5.6**。

## 功能概览
- **待办事项**：输入标题后回车即可添加，支持 Markdown 所见即所得描述；可设置一次性提醒时间，列表中按“即将到期 / 已过期”着色显示。
- **已办事项**：按标题或描述搜索，勾选即可取消完成。
- **循环提醒**：支持区间间隔、每天、每周、每月固定时间与 Cron 表达式，可暂停/恢复。
- **提醒记录**：按日期与类型筛选，查看每次提醒的处理结果（已关闭 / 已推迟 / 已完成 / 待处理），支持批量删除。
- **提醒弹窗**：到点弹出，支持“知道了”和“稍后提醒”（推迟分钟数可在设置中调整），可选提示音。
- **桌面便签**：每张便签一个独立窗口，支持 Markdown 编辑、自动保存、锚定（置顶并锁定位置）、标记完成，以及在右下角设置提醒时间。
- **外观**：深色/浅色主题、界面缩放、整体透明度，提醒弹窗主题可单独设置。
- **云同步**：通过 WebDAV 在多台设备间同步数据。
- **自动更新**：启动时自动检查 GitHub Releases 上的新版本，可配置仅用于更新的代理。
- **系统集成**：托盘菜单、开机自启、单实例运行。

## 环境准备
- 安装 Node.js 与 pnpm
- 安装 Rust 工具链
- 安装 Tauri 构建依赖（不同操作系统依赖略有差异）

## Ubuntu 24.04 开发准备（推荐）
1. 安装系统依赖：
```
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev \
  libgtk-3-dev libsoup-3.0-dev libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

2. 安装 Node.js 与 pnpm（任选其一方式）：
```
corepack enable
```
或：
```
npm i -g pnpm
```

3. 安装 Rust 工具链（全局用户级，推荐）：
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup default stable
```
说明：项目默认使用系统环境中的 `rustup` / `cargo`

4. 如果遇到 `ENOSPC`（inotify 监听数不足），可提高系统上限：
```
sudo sysctl -w fs.inotify.max_user_watches=524288
```

5. 如果启动时出现 `libEGL warning: failed to open /dev/dri/renderD128: 权限不够`：
```
sudo usermod -aG render $USER
```
执行后重新登录系统（或重启）使组权限生效。  
项目的 `scripts/tauri.mjs` 已在检测到该权限问题时自动降级为软件渲染，用于避免开发期反复刷警告。

## 启动开发
1) 安装依赖：
```
pnpm install
```

2) 启动 Web 开发（仅前端）：
```
pnpm dev
```

3) 启动桌面应用开发：
```
pnpm tauri dev
```

### 开发模式与生产实例共存
应用通过 `cfg!(debug_assertions)` 自动区分开发/生产模式:
- **开发模式** (`pnpm tauri dev`): 编译为 debug 构建,使用独立数据目录 `data-dev`
- **生产模式** (`pnpm tauri build`): 编译为 release 构建,使用标准数据目录 `data`

两种模式拥有完全独立的:
- 数据库文件 (`taskreminder.db`)
- 锁文件 (`.taskreminder.lock`)
- 应用数据存储

开发实例会在以下位置显示 `[开发]` 标识:
- 窗口标题栏: "任务提醒应用 **[开发]**"
- 系统托盘菜单: "打开 **[开发]**"
- 任务栏/Alt+Tab 窗口标题

这样你可以同时运行开发实例和已安装的生产实例进行测试对比,互不干扰。

### Tauri 2 权限说明
- 项目使用 capability 文件：`src-tauri/capabilities/default.json`
- 已为主窗口和通知窗口配置基础窗口权限（包含标题栏拖拽）

## 构建与打包
### 前端构建（仅 Web 资源）
```
pnpm build
```
构建产物输出到 `dist/`。

### 桌面应用打包
```
pnpm tauri build
```
打包产物默认输出到 `src-tauri/target/release/bundle/`。

### 不同环境打包
Tauri 的打包通常需要在目标操作系统上执行：
- Windows：在 Windows 上运行 `pnpm tauri build`
- macOS：在 macOS 上运行 `pnpm tauri build`
- Linux：在 Linux 上运行 `pnpm tauri build`

如需跨平台分发，建议在对应操作系统或 CI 中分别打包。

### Windows 打包 MSI
1) 安装 WiX Toolset（生成 MSI 需要 WiX 工具链）。
2) 配置 `src-tauri/tauri.conf.json`：
```
"identifier": "ylfty.top",
"bundle": {
  "active": true,
  "targets": ["msi"]
}
```
3) 执行打包：
```
pnpm tauri build --bundles msi
```
产物在 `src-tauri/target/release/bundle/msi/`。

普通 `pnpm tauri build` 只生成安装包，不会生成 updater 所需的 `.sig` 和 `latest.json`，因此也不要求配置 `TAURI_SIGNING_PRIVATE_KEY`。

### 自动更新发布（GitHub Releases）
项目已接入 Tauri 官方 updater，Windows 客户端会从 GitHub Releases 检查新版本。

当前更新清单地址固定为：
```
https://github.com/zhouyilong/taskReminder/releases/latest/download/latest.json
```

其中 `latest.json` 会指向当前版本对应的 MSI 安装包，例如：
```
https://github.com/zhouyilong/taskReminder/releases/download/v1.5.6/TaskReminderApp_1.5.6_x64_zh-CN.msi
```

建议使用下面的命令生成自动更新产物：
```
pnpm release:updater
```

该命令会：
- 读取 `$env:USERPROFILE\.tauri\taskReminder-updater.key`
- 执行 `pnpm tauri build --bundles msi --config src-tauri/tauri.updater.conf.json`
- 生成 MSI 对应的签名文件 `.sig`
- 在 `src-tauri/target/release/bundle/msi/` 中生成 `latest.json`

每次发布新版本时，需要把以下 3 个文件上传到对应版本号的 GitHub Release：
- `TaskReminderApp_<version>_x64_zh-CN.msi`
- `TaskReminderApp_<version>_x64_zh-CN.msi.sig`
- `latest.json`

发布要求：
- GitHub Release 标签需使用 `v<version>` 格式，例如 `v1.5.6`
- `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json` 中的版本号应保持一致
- `latest.json` 必须上传到“最新版本”对应的 Release，客户端才会通过 `releases/latest/download/latest.json` 获取到更新

如需填写更新说明，可在执行命令时传入：
```
powershell -ExecutionPolicy Bypass -File scripts/build-updater.ps1 -ReleaseNotes "1. 新增自动更新`n2. 优化提醒体验"
```

## 分发打包后的文件
打包后在 `src-tauri/target/release/bundle/` 中按平台生成安装包或可执行文件，常见形式：
- Windows：`.msi` / `.exe`
- macOS：`.dmg` / `.app`
- Linux：`.AppImage` / `.deb` / `.rpm`

分发时可直接提供安装包给用户；若需要上架或更易于安装与更新，可结合签名与发布流程。版本号与应用元数据可在 `src-tauri/tauri.conf.json` 中调整。 

## 桌面便签故障排查与修复思路（经验记录）
以下内容用于处理“桌面便签打不开、点击无响应、层级异常、主窗口关闭异常”等问题。

> 说明：早期版本有一个“桌面便签管理窗口”（`StickyNoteApp.vue`），现已移除。当前每张便签都是独立窗口（`sticky-note-item.html` → `src/StickyNoteItemApp.vue`，窗口标签 `sticky-note-item-<编码后的 id>`），可从主窗口标题栏或托盘菜单“新建便签”创建。下文中关于管理窗口的内容仅作历史经验保留，排查思路对便签窗口同样适用。

### 典型症状
- 点击“桌面便签”按钮后无反应，或提示“打开桌面便签超时”。
- 打开了桌面便签管理窗口，但点击“便签列表项/新增便签”没有反应。
- 桌面便签窗口遮挡主界面右上角按钮，导致主程序“关闭”看起来失效。
- 便签项窗口实际已创建，但被层级压在底层，用户误判为“没有打开”。

### 根因定位方法
1. 先区分“命令没执行”还是“命令执行但窗口不可见”
- 前端按钮点击后，先看是否进入 `invoke`（可在前端打印日志）。
- 后端命令中分别记录：数据库写入、窗口创建、`show`、定位、事件发射。

2. 再区分“UI点击被吞”还是“后端卡住”
- 若按钮点击无报错、无后端日志，优先排查拖拽区域（`data-tauri-drag-region` / `-webkit-app-region`）。
- 若后端最终成功但前端超时报错，优先排查同步阻塞链路（窗口 API + 同步通知）。

3. 最后排查窗口层级冲突
- 管理窗口与便签项窗口必须使用不同层级策略。
- 管理窗口不应长期 `always_on_top(true)`，否则容易挡住主窗口操作区。

### 修复原则（本项目）
1. 让命令“快速返回”
- `open_sticky_note` / `create_sticky_note` 中涉及窗口显示的操作改为异步执行，避免前端等待超时。
- 同步通知（`notify_local_change`）不要阻塞关键交互命令返回。

2. 严格隔离拖拽区和交互区
- 仅窗口顶部标题区域允许拖拽。
- 列表、按钮、输入框等交互区强制 `no-drag`，避免点击事件被窗口拖动吞掉。

3. 统一层级策略
- 管理窗口：用于操作，不设为常驻顶层（`always_on_top(false)`），避免遮挡主界面。
- 便签项窗口：贴桌面场景下可使用底层策略（`always_on_bottom(true)`）。

4. 保证位置不影响主操作
- 管理窗口固定右上角时，Y 轴预留顶部安全边距，避免压住主窗口标题栏按钮。

### 关键代码位置（便于快速回查）
- 前端便签窗口：`src/StickyNoteItemApp.vue`（入口 `src/stickyNoteItem.ts`）
- 前端主窗口“新建便签”入口：`src/App.vue`
- 托盘菜单“新建便签”：`src-tauri/src/tray.rs`
- 后端窗口创建、URL 与层级（锚定/贴桌面）：`src-tauri/src/main.rs`
- 便签数据读写与锚定状态持久化：`src-tauri/src/db.rs`

### 回归检查清单
1. 主窗口标题栏点击“新建便签”可稳定新建并弹出便签窗口。
2. 便签窗口内点击“+”可新增便签，点击“✓”可标记完成并关闭。
3. 便签内容修改后自动保存，重启应用后内容与位置保持一致。
4. 锚定后便签置顶且位置锁定，取消锚定后恢复贴桌面、可拖动；重启后锚定状态保持。
5. 便签右下角设置提醒时间后，到点弹出提醒；清除后不再提醒。
6. 打开便签后，主程序“关闭”按钮仍可正常点击。
7. Linux 下便签能正常载入并保存（不会停在“载入便签...”）。
