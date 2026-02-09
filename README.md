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

3. 安装 Rust 工具链到项目本地（避免 home 不可写或污染系统环境）：
```
export RUSTUP_HOME="$PWD/.dev/rustup"
export CARGO_HOME="$PWD/.dev/cargo"
export PATH="$CARGO_HOME/bin:$PATH"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
rustup default stable
```
说明：本项目已在 `package.json` 的 `tauri` 脚本中自动注入上述变量（仅 Linux/macOS 生效）；Windows 不会注入这组变量，会使用系统默认的 Rust 环境。
如果你之前用 sudo 安装过 rustup，建议执行：
```
sudo chown -R $USER:$USER .dev
```

4. 如果遇到 `ENOSPC`（inotify 监听数不足），可提高系统上限：
```
sudo sysctl -w fs.inotify.max_user_watches=524288
```

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
