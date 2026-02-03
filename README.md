# Task Reminder (Tauri + Vue 3)

一个基于 Tauri + Vue 3 + TypeScript 的桌面任务提醒应用，包含主窗口与通知窗口。

## 环境准备
- 安装 Node.js 与 pnpm
- 安装 Rust 工具链
- 安装 Tauri 构建依赖（不同操作系统依赖略有差异）

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
若运行后界面仍是旧版本或未更新，可先执行：
```
pnpm build
```
再执行 `pnpm tauri dev`，确保 Tauri 不会回退到过期的 `dist/`。

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
1) 安装 WiX Toolset（Tauri v1 生成 MSI 需要 WiX 3.11）。
2) 配置 `src-tauri/tauri.conf.json`：
```
"bundle": {
  "active": true,
  "targets": ["msi"],
  "identifier": "ylfty.top"
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
