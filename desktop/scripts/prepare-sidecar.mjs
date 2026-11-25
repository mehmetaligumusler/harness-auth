/**
 * Compiles the headless ZCode engine with Bun and packs it as gzip + sha256 into
 * `desktop/src-tauri/embedded/` so Rust embeds it inside the single HarnessAuth.exe.
 *
 * Pass `--if-missing` to skip compiling when `engine.exe.gz` & `engine.sha256`
 * already exist (used by `prepare:sidecar`).
 */
import { createHash } from "node:crypto";
import { execSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const here = dirname(fileURLToPath(import.meta.url));
const desktopRoot = resolve(here, "..");
const repoRoot = resolve(desktopRoot, "..");
const embeddedDir = join(desktopRoot, "src-tauri", "embedded");
const gzOut = join(embeddedDir, "engine.exe.gz");
const shaOut = join(embeddedDir, "engine.sha256");
const ifMissing = process.argv.includes("--if-missing");

mkdirSync(embeddedDir, { recursive: true });

if (ifMissing && existsSync(gzOut) && existsSync(shaOut)) {
  console.log(`Embedded engine already present at ${gzOut}`);
  process.exit(0);
}

const tmpDir = join(repoRoot, ".tmp-bun");
const tmpExe = join(tmpDir, "harness-zcode-engine.exe");
mkdirSync(tmpDir, { recursive: true });

const env = {
  ...process.env,
  TEMP: tmpDir,
  TMP: tmpDir,
  BUN_TMPDIR: tmpDir,
};

try {
  console.log("Building fork worker & compiling headless ZCode engine...");
  execSync("bun run scripts/build-fork-worker.ts", {
    cwd: repoRoot,
    env,
    stdio: "inherit",
  });
  execSync(
    `bun build --compile --define "require.resolve=undefined" src/index.ts --outfile "${tmpExe}"`,
    {
      cwd: repoRoot,
      env,
      stdio: "inherit",
    },
  );

  const raw = readFileSync(tmpExe);
  const sha = createHash("sha256").update(raw).digest("hex");
  const compressed = gzipSync(raw, { level: 6 });

  writeFileSync(gzOut, compressed);
  writeFileSync(shaOut, `${sha}\n`, "utf8");

  const rawMb = (raw.length / (1024 * 1024)).toFixed(1);
  const gzMb = (compressed.length / (1024 * 1024)).toFixed(1);
  console.log(`Embedded engine ready: ${rawMb} MB -> ${gzMb} MB gzip (sha256: ${sha.slice(0, 12)})`);
} finally {
  try {
    rmSync(tmpDir, { recursive: true, force: true });
  } catch {
    // ignore cleanup failure
  }
}
