# Task Reminder (Tauri + Vue 3)

一个基于 Tauri + Vue 3 + TypeScript 的桌面任务提醒应用，包含主窗口与通知窗口。

当前技术栈：**Tauri 2 + Vue 3 + TypeScript**。

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

## 分发打包后的文件
打包后在 `src-tauri/target/release/bundle/` 中按平台生成安装包或可执行文件，常见形式：
- Windows：`.msi` / `.exe`
- macOS：`.dmg` / `.app`
- Linux：`.AppImage` / `.deb` / `.rpm`

分发时可直接提供安装包给用户；若需要上架或更易于安装与更新，可结合签名与发布流程。版本号与应用元数据可在 `src-tauri/tauri.conf.json` 中调整。 

## 桌面便签故障排查与修复思路（经验记录）
以下内容用于处理“桌面便签打不开、点击无响应、层级异常、主窗口关闭异常”等问题。

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
- 前端桌面便签管理：`src/StickyNoteApp.vue`
- 前端主窗口开关逻辑：`src/App.vue`
- 后端窗口创建与层级：`src-tauri/src/main.rs`
- 便签数据读写：`src-tauri/src/db.rs`

### 回归检查清单
1. 点击“桌面便签”可稳定打开/关闭管理窗口。
2. 管理窗口中点击“便签列表项”可弹出便签项窗口。
3. 管理窗口中点击“新增便签”可新增并弹出便签项窗口。
4. 打开管理窗口后，主程序“关闭”按钮仍可正常点击。
5. 便签项窗口层级符合预期（贴桌面，非前置遮挡业务窗口）。
6. 冷启动后不会自动弹出桌面便签（保持手动打开）。
