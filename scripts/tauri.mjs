import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { delimiter, resolve } from "node:path";

const args = process.argv.slice(2);
const projectRoot = process.cwd();
const env = { ...process.env };
const isDevCommand = args[0] === "dev";

if (process.platform !== "win32") {
  const rustupHome = resolve(projectRoot, ".dev/rustup");
  const cargoHome = resolve(projectRoot, ".dev/cargo");
  const cargoBin = resolve(cargoHome, "bin");

  if (existsSync(rustupHome) && existsSync(cargoHome)) {
    env.RUSTUP_HOME = rustupHome;
    env.CARGO_HOME = cargoHome;
    env.PATH = [cargoBin, env.PATH].filter(Boolean).join(delimiter);
  }
}

if (process.platform === "linux" && isDevCommand) {
  // WSL/remote X server 环境常见 DRI3 与 /dev/dri 权限问题，默认回退软件渲染。
  if (!env.WEBKIT_DISABLE_DMABUF_RENDERER) {
    env.WEBKIT_DISABLE_DMABUF_RENDERER = "1";
  }
  if (!env.LIBGL_DRI3_DISABLE) {
    env.LIBGL_DRI3_DISABLE = "1";
  }
  if (!env.LIBGL_ALWAYS_SOFTWARE) {
    env.LIBGL_ALWAYS_SOFTWARE = "1";
  }
}

const localTauriBin =
  process.platform === "win32"
    ? resolve(projectRoot, "node_modules/.bin/tauri.cmd")
    : resolve(projectRoot, "node_modules/.bin/tauri");

const tauriCommand = existsSync(localTauriBin) ? localTauriBin : "tauri";
const child = spawn(tauriCommand, args, {
  env,
  stdio: "inherit",
  shell: process.platform === "win32",
});

child.on("error", (error) => {
  console.error(`[tauri-runner] Failed to start Tauri CLI: ${error.message}`);
  process.exit(1);
});

child.on("exit", (code) => {
  process.exit(code ?? 1);
});
