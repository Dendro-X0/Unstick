#!/usr/bin/env node
/**
 * Unstick dev runner — build (debug) then start service + UI.
 * Usage: pnpm install && pnpm dev
 */
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readFileSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const isWin = process.platform === "win32";
const exe = (name) =>
  path.join(root, "target", "debug", isWin ? `${name}.exe` : name);

const tmp = path.join(root, ".tmp");
mkdirSync(tmp, { recursive: true });
process.env.TMPDIR = tmp;
process.env.TEMP = tmp;
process.env.TMP = tmp;

/** @type {number[]} */
const pids = [];
let shuttingDown = false;

function run(cmd, args, useShell = false) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, {
      cwd: root,
      stdio: "inherit",
      shell: useShell,
      env: process.env,
      windowsHide: true,
    });
    child.on("error", reject);
    child.on("exit", (code, signal) => {
      if (signal) reject(new Error(`${cmd} killed by ${signal}`));
      else if (code !== 0) reject(new Error(`${cmd} exited ${code}`));
      else resolve();
    });
  });
}

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

/** Start a binary; on Windows use Start-Process to avoid EPERM on GUI exes. */
async function start(bin, label) {
  if (!existsSync(bin)) {
    throw new Error(`Missing binary: ${bin}`);
  }

  if (isWin) {
    const escaped = bin.replace(/'/g, "''");
    const wd = root.replace(/'/g, "''");
    const pidFile = path.join(tmp, `${label}.pid`);
    const pidEsc = pidFile.replace(/'/g, "''");
    // Service runs headless; UI needs a normal window.
    const style = label === "guardian-service" ? "Hidden" : "Normal";
    const ps = [
      `$p = Start-Process -FilePath '${escaped}' -WorkingDirectory '${wd}' -WindowStyle ${style} -PassThru`,
      `Set-Content -Path '${pidEsc}' -Value $p.Id -Encoding ascii`,
    ].join("; ");

    await new Promise((resolve, reject) => {
      const child = spawn(
        "powershell.exe",
        ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", ps],
        { cwd: root, stdio: "ignore", windowsHide: true },
      );
      child.on("error", reject);
      child.on("exit", (code) => {
        if (code === 0) resolve();
        else reject(new Error(`Start-Process ${label} exited ${code}`));
      });
    });

    const raw = readFileSync(pidFile, "utf8").trim();
    const pid = Number.parseInt(raw, 10);
    if (!Number.isFinite(pid)) {
      throw new Error(`Could not read pid for ${label} from ${pidFile}`);
    }
    pids.push(pid);
    console.log(`  ${label} pid=${pid}`);
    return;
  }

  const child = spawn(bin, [], {
    cwd: root,
    stdio: "ignore",
    detached: true,
    env: process.env,
  });
  child.unref();
  if (child.pid) pids.push(child.pid);
  console.log(`  ${label} pid=${child.pid}`);
}

function shutdown() {
  if (shuttingDown) return;
  shuttingDown = true;
  console.log("\n→ stopping Unstick…");
  for (const pid of pids) {
    try {
      if (isWin) {
        spawn("taskkill", ["/PID", String(pid), "/T", "/F"], {
          stdio: "ignore",
          windowsHide: true,
        });
      } else {
        process.kill(pid, "SIGTERM");
      }
    } catch {
      /* already gone */
    }
  }
}

async function main() {
  // Windows: prior UI/service keep .exe locked → cargo "Access is denied".
  if (isWin) {
    console.log("→ stopping any prior Unstick processes");
    await new Promise((resolve) => {
      const child = spawn(
        "taskkill",
        ["/F", "/IM", "guardian-ui.exe", "/IM", "guardian-service.exe", "/IM", "guardian-tray.exe"],
        { stdio: "ignore", windowsHide: true },
      );
      child.on("exit", () => resolve());
      child.on("error", () => resolve());
    });
    await sleep(400);
  }

  console.log("→ cargo build -p guardian-service -p guardian-ui");
  try {
    await run("cargo", ["build", "-p", "guardian-service", "-p", "guardian-ui"]);
  } catch (err) {
    if (err && err.code === "ENOENT") {
      await run("cargo", ["build", "-p", "guardian-service", "-p", "guardian-ui"], true);
    } else {
      throw err;
    }
  }

  const serviceBin = exe("guardian-service");
  const uiBin = exe("guardian-ui");
  if (!existsSync(serviceBin) || !existsSync(uiBin)) {
    console.error("Build finished but binaries missing:", serviceBin, uiBin);
    process.exit(1);
  }

  console.log("→ starting guardian-service");
  await start(serviceBin, "guardian-service");
  await sleep(800);
  console.log("→ starting guardian-ui");
  await start(uiBin, "guardian-ui");

  console.log("Unstick dev is running. Press Ctrl+C to stop service + UI.");

  const finish = () => {
    shutdown();
    process.exit(0);
  };
  process.once("SIGINT", finish);
  process.once("SIGTERM", finish);
  // Keep the event loop alive without an unsettled top-level await (Node 22).
  setInterval(() => {}, 60_000);
}

main().catch((err) => {
  console.error(err);
  shutdown();
  process.exit(1);
});
