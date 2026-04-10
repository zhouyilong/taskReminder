import { execFileSync, spawn } from "node:child_process";
import { accessSync, constants, existsSync } from "node:fs";
import { delimiter, resolve } from "node:path";

const args = process.argv.slice(2);
const projectRoot = process.cwd();
const env = { ...process.env };

function prependPath(entries) {
  const currentPath = env.PATH ?? env.Path ?? "";
  const mergedEntries = [...entries.filter(Boolean), currentPath].filter(Boolean);
  env.PATH = mergedEntries.join(delimiter);
}

function quoteForCmd(value) {
  return `"${String(value).replace(/"/g, '""')}"`;
}

function findVsDevCmd() {
  if (process.platform !== "win32") {
    return null;
  }

  const vswherePath = resolve(
    process.env["ProgramFiles(x86)"] ?? "C:/Program Files (x86)",
    "Microsoft Visual Studio/Installer/vswhere.exe",
  );

  if (!existsSync(vswherePath)) {
    return null;
  }

  try {
    const installationPath = execFileSync(
      vswherePath,
      [
        "-latest",
        "-products",
        "*",
        "-requires",
        "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
        "-property",
        "installationPath",
      ],
      {
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
      },
    ).trim();

    if (!installationPath) {
      return null;
    }

    const devCmdPath = resolve(installationPath, "Common7/Tools/VsDevCmd.bat");
    return existsSync(devCmdPath) ? devCmdPath : null;
  } catch {
    return null;
  }
}

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

if (process.platform === "win32") {
  const cargoBin = resolve(process.env.USERPROFILE ?? "", ".cargo/bin");
  if (existsSync(cargoBin)) {
    prependPath([cargoBin]);
  }
}

const localTauriBin =
  process.platform === "win32"
    ? resolve(projectRoot, "node_modules/.bin/tauri.cmd")
    : resolve(projectRoot, "node_modules/.bin/tauri");

const tauriCommand = existsSync(localTauriBin) ? localTauriBin : "tauri";
let command = tauriCommand;
let commandArgs = args;
let windowsVerbatimArguments = false;

if (process.platform === "win32") {
  const needsVsDevShell = !("VCINSTALLDIR" in process.env);
  const vsDevCmd = needsVsDevShell ? findVsDevCmd() : null;

  if (vsDevCmd) {
    const tauriInvocation = `call ${[quoteForCmd(tauriCommand), ...args.map(quoteForCmd)].join(" ")}`;
    const cmdScript = `call ${quoteForCmd(vsDevCmd)} -arch=amd64 && ${tauriInvocation}`;
    command = "cmd.exe";
    commandArgs = ["/d", "/c", cmdScript];
    windowsVerbatimArguments = true;
  } else {
    command = "cmd.exe";
    commandArgs = ["/d", "/c", `call ${[quoteForCmd(tauriCommand), ...args.map(quoteForCmd)].join(" ")}`];
    windowsVerbatimArguments = true;
  }
}

const child = spawn(command, commandArgs, {
  env,
  stdio: "inherit",
  windowsVerbatimArguments,
});

child.on("error", (error) => {
  console.error(`[tauri-runner] Failed to start Tauri CLI: ${error.message}`);
  process.exit(1);
});

child.on("exit", (code) => {
  process.exit(code ?? 1);
});
