/**
 * Copies the compiled proxy binary into the Tauri sidecar slot.
 * Tauri expects the target-triple suffix and strips it at bundle time.
 */
import { copyFileSync, mkdirSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const desktopRoot = resolve(here, "..");
const repoRoot = resolve(desktopRoot, "..");

const source = join(repoRoot, "zcode-proxy.exe");
const target = join(desktopRoot, "src-tauri", "binaries", "zcode-proxy-x86_64-pc-windows-msvc.exe");

if (!existsSync(source)) {
  console.error(`Proxy binary not found at ${source}`);
  console.error("Build it first from the repo root: bun run build");
  process.exit(1);
}

mkdirSync(dirname(target), { recursive: true });
copyFileSync(source, target);
console.log(`Sidecar ready: ${target}`);
