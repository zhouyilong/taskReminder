# 仓库指南

## 项目结构与模块组织
- `src/` 存放 Vue 3 + TypeScript 前端。入口：`src/main.ts`、`src/App.vue`，以及通知窗口 `src/NotificationApp.vue`。
- `src/components/` 存放可复用 UI 组件。
- `src/styles.css` 为全局样式表；组件样式放在 `.vue` 文件内。
- `src/update.ts` 自动更新逻辑模块（检查更新、安装更新、偏好管理）。
- `src-tauri/` 存放 Tauri 应用的 Rust 后端。
  - `src-tauri/src/` 为 Rust 应用代码。
  - `src-tauri/migrations/` 存放数据库迁移文件。
  - `src-tauri/icons/` 存放应用图标。
  - `src-tauri/tauri.conf.json` 定义窗口、打包、更新器与应用元数据。
- `scripts/` 存放构建与发布脚本。
  - `scripts/build-updater.ps1` 签名构建 MSI + 生成更新清单。
  - `scripts/write-updater-manifest.mjs` 生成 `latest.json` 更新清单。
  - `scripts/tauri.mjs` Tauri CLI 包装器（处理 VS Dev Shell 环境）。

## 构建、测试与开发命令
- `pnpm dev`：启动 Web UI 的 Vite 开发服务器。
- `pnpm build`：将前端打包到 `dist/`。
- `pnpm preview`：本地预览生产构建。
- `pnpm tauri dev`：以开发模式运行完整的 Tauri 桌面应用。
- `pnpm tauri build`：生成生产环境桌面应用包。
  - 若 `pnpm tauri dev` 仍显示旧界面，可先执行 `pnpm build` 再运行 `pnpm tauri dev`，避免回退到过期的 `dist/`。
- `pnpm release:updater`：执行完整的更新器构建流程（签名 + MSI + latest.json）。

## 版本管理
- 版本号需在三处同步修改：`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`。
- 发布更新时，GitHub Release 需上传三个文件：`.msi`、`.msi.sig`、`latest.json`。

## 自动更新发布流程
1. 修改三处版本号
2. 构建：设置 `TAURI_SIGNING_PRIVATE_KEY` 和 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 环境变量后运行 `pnpm tauri build --bundles msi`
3. 生成清单：`node scripts/write-updater-manifest.mjs`
4. 发布：`gh release create v{version}` 上传 MSI + .sig + latest.json
5. 签名私钥位于 `~/.tauri/taskReminder-updater.key`

## 编码风格与命名约定
- 缩进：`.vue`、`.ts`、`.css` 使用 2 个空格（保持现有格式）。
- Vue 组件文件名使用 `PascalCase`（如 `NotificationApp.vue`）。
- TypeScript 标识符：函数/变量用 `camelCase`，类型用 `PascalCase`。
- CSS 类名使用 `kebab-case`（如 `.titlebar-actions`）。
- Rust 代码使用标准 `snake_case` 的模块与函数命名；用 `cargo fmt`（默认 rustfmt）格式化。

## 踩坑记录与注意事项

### Vue Proxy 与 Tauri 插件私有字段冲突
- **问题**：`@tauri-apps/plugin-updater` 的 `Update` 类使用了 JS 私有字段（`#field`），存入 Vue `ref()` 后被 Proxy 深度包装，调用 `downloadAndInstall()` 时报 `Cannot read private member from an object whose class did not declare it`。
- **解决**：使用 `shallowRef` 代替 `ref` 存储 Tauri 插件返回的类实例对象，避免 Vue 深度代理。
- **规则**：**凡是 Tauri 插件返回的类实例（如 `Update`、`Channel` 等），一律使用 `shallowRef` 而非 `ref` 存储。**

### 构建签名环境变量传递
- **问题**：PowerShell 中通过 `$env:TAURI_SIGNING_PRIVATE_KEY` 设置环境变量后，直接调用 `pnpm.cmd` 可能导致子进程环境变量丢失，构建卡住。
- **解决**：① 用 `.Trim()` 清除密钥尾部换行；② 设置 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""` 避免交互等待；③ 用 `cmd /c` 包装 `pnpm.cmd` 调用确保环境变量继承。
- **规则**：在 bash 中构建时，直接 `export` 两个环境变量后调用 `pnpm tauri build`，最可靠。

### onMounted 中异步操作的异常隔离
- **问题**：多个异步操作放在同一个 `try` 块中，前面的操作抛异常会导致后面的操作被跳过（如自动更新检查被数据初始化异常阻断）。
- **解决**：将相互独立的异步操作放在各自独立的 `try/catch` 块中。
- **规则**：`onMounted` 中多个独立的异步初始化操作应分别用 `try/catch` 包裹，互不影响。

## 测试指南
- `package.json` 尚未配置 JavaScript 测试框架。
- Rust 测试可在 `src-tauri/src/` 中添加 `#[cfg(test)]` 模块，并通过 `cargo test` 运行。
- 若引入 JS 测试框架，请将测试放在 `src/` 下（如 `*.spec.ts`），并在 `package.json` 中添加脚本。

## 提交与合并请求指南
- 提交信息使用简短祈使句，例如：`feat: 新增托盘开关`、`fix: 修复更新安装失败`。
- PR 需包含：简要摘要、测试步骤，以及 UI 变更截图。

## 配置与资源
- 在 `src-tauri/tauri.conf.json` 中更新窗口行为、打包标识符与应用元数据。
- 保持静态 HTML 入口文件（`index.html`、`notification.html`）简洁，并与 Vue 入口保持同步。
- 更新器配置在 `src-tauri/tauri.conf.json` 的 `plugins.updater` 节点，包含公钥和 GitHub Releases 端点。
