import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const args = process.argv.slice(2);
const projectRoot = process.cwd();
const env = { ...process.env };
const isDevCommand = args[0] === "dev";

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
