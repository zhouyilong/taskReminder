import { spawn } from "node:child_process";
import { accessSync, constants, existsSync } from "node:fs";
import { resolve } from "node:path";

const args = process.argv.slice(2);
const projectRoot = process.cwd();
const env = { ...process.env };

if (process.platform === "linux") {
  const driRenderNode = "/dev/dri/renderD128";
  try {
    accessSync(driRenderNode, constants.R_OK | constants.W_OK);
  } catch {
    env.WEBKIT_DISABLE_DMABUF_RENDERER ??= "1";
    env.LIBGL_ALWAYS_SOFTWARE ??= "1";
    console.warn(
      "[tauri-runner] /dev/dri/renderD128 cannot be accessed; using software rendering. Add user to 'render' group and re-login to restore GPU rendering.",
    );
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
