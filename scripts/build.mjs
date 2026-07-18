#!/usr/bin/env node
/**
 * Release build for Unstick service + UI (+ tray).
 * Usage: pnpm build
 */
import { spawn } from "node:child_process";
import { mkdirSync } from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const tmp = path.join(root, ".tmp");
mkdirSync(tmp, { recursive: true });
process.env.TMPDIR = tmp;
process.env.TEMP = tmp;
process.env.TMP = tmp;

const args = [
  "build",
  "--release",
  "-p",
  "guardian-service",
  "-p",
  "guardian-ui",
  "-p",
  "guardian-tray",
];

function spawnCargo(useShell) {
  return spawn("cargo", args, {
    cwd: root,
    stdio: "inherit",
    shell: useShell,
    env: process.env,
    windowsHide: true,
  });
}

const child = spawnCargo(false);
child.on("error", (err) => {
  if (err.code === "ENOENT") {
    const fallback = spawnCargo(true);
    fallback.on("exit", (code) => process.exit(code ?? 1));
    return;
  }
  console.error(err);
  process.exit(1);
});
child.on("exit", (code) => process.exit(code ?? 1));
