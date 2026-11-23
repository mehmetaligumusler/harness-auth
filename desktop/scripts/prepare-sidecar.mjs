/**
 * Verifies or builds the compiled headless ZCode sidecar binary in the Tauri sidecar slot.
 * Tauri expects the target-triple suffix and strips it at bundle time.
 */
import { existsSync, mkdirSync } from "node:fs";
import { execSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const desktopRoot = resolve(here, "..");
const repoRoot = resolve(desktopRoot, "..");
const target = join(
  desktopRoot,
  "src-tauri",
  "binaries",
  "harness-zcode-engine-x86_64-pc-windows-msvc.exe",
);

mkdirSync(dirname(target), { recursive: true });

if (!existsSync(target)) {
  console.log("Sidecar binary not found; building headless engine...");
  execSync("bun run build:sidecar", { cwd: repoRoot, stdio: "inherit" });
}

if (!existsSync(target)) {
  console.error(`Sidecar binary still missing at ${target}`);
  process.exit(1);
}

console.log(`Sidecar ready: ${target}`);
