# 仓库指南

## 项目结构与模块组织
- `src/` 存放 Vue 3 + TypeScript 前端。入口：`src/main.ts`、`src/App.vue`，以及通知窗口 `src/NotificationApp.vue`。
- `src/components/` 存放可复用 UI 组件。
- `src/styles.css` 为全局样式表；组件样式放在 `.vue` 文件内。
- `src-tauri/` 存放 Tauri 应用的 Rust 后端。
  - `src-tauri/src/` 为 Rust 应用代码。
  - `src-tauri/migrations/` 存放数据库迁移文件。
  - `src-tauri/icons/` 存放应用图标。
  - `src-tauri/tauri.conf.json` 定义窗口、打包与应用元数据。

## 构建、测试与开发命令
- `pnpm dev`：启动 Web UI 的 Vite 开发服务器。
- `pnpm build`：将前端打包到 `dist/`。
- `pnpm preview`：本地预览生产构建。
- `pnpm tauri dev`：以开发模式运行完整的 Tauri 桌面应用。
- `pnpm tauri build`：生成生产环境桌面应用包。
  - 若 `pnpm tauri dev` 仍显示旧界面，可先执行 `pnpm build` 再运行 `pnpm tauri dev`，避免回退到过期的 `dist/`。

## 编码风格与命名约定
- 缩进：`.vue`、`.ts`、`.css` 使用 2 个空格（保持现有格式）。
- Vue 组件文件名使用 `PascalCase`（如 `NotificationApp.vue`）。
- TypeScript 标识符：函数/变量用 `camelCase`，类型用 `PascalCase`。
- CSS 类名使用 `kebab-case`（如 `.titlebar-actions`）。
- Rust 代码使用标准 `snake_case` 的模块与函数命名；用 `cargo fmt`（默认 rustfmt）格式化。

## 测试指南
- `package.json` 尚未配置 JavaScript 测试框架。
- Rust 测试可在 `src-tauri/src/` 中添加 `#[cfg(test)]` 模块，并通过 `cargo test` 运行。
- 若引入 JS 测试框架，请将测试放在 `src/` 下（如 `*.spec.ts`），并在 `package.json` 中添加脚本。

## 提交与合并请求指南
- 此工作区未包含 Git 历史，无法检测现有提交规范。
- 提交信息使用简短祈使句，例如：`feat: add tray toggle`。
- PR 需包含：简要摘要、测试步骤，以及 UI 变更截图。

## 配置与资源
- 在 `src-tauri/tauri.conf.json` 中更新窗口行为、打包标识符与应用元数据。
- 保持静态 HTML 入口文件（`index.html`、`notification.html`）简洁，并与 Vue 入口保持同步。
