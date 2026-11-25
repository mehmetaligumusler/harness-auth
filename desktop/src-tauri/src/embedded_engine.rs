//! Embedded headless ZCode engine extractor.
//!
//! At compile time the gzip-compressed Bun ZCode engine (`engine.exe.gz`) and its
//! SHA-256 digest (`engine.sha256`) are embedded directly inside `HarnessAuth.exe`
//! via `include_bytes!`, eliminating any external sidecar `.exe` next to the app.
//! On first invocation (or version upgrade) it is atomically decompressed into
//! `%APPDATA%\harness-auth\runtime\engine-<sha12>.exe`.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use flate2::read::GzDecoder;

use crate::paths::Paths;

const ENGINE_GZ: &[u8] = include_bytes!("../embedded/engine.exe.gz");
const ENGINE_SHA256: &str = include_str!("../embedded/engine.sha256");

static EXTRACT_LOCK: Mutex<()> = Mutex::new(());

pub fn short_hash() -> &'static str {
    let trimmed = ENGINE_SHA256.trim();
    &trimmed[..trimmed.len().min(12)]
}

/// Resolve the ZCode engine binary, extracting the embedded payload to
/// `paths.runtime_dir()` if missing.
pub fn resolve_or_extract(paths: &Paths) -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("HARNESS_AUTH_ENGINE_EXE") {
        let p = PathBuf::from(p.trim());
        if p.is_file() {
            return Ok(p);
        }
    }

    let hash = short_hash();
    if hash.is_empty() {
        return Err("Embedded ZCode engine hash is empty".into());
    }

    let runtime_dir = paths.runtime_dir();
    let exe_name = format!("engine-{hash}.exe");
    let target = runtime_dir.join(&exe_name);

    if is_valid_binary(&target) {
        return Ok(target);
    }

    let _guard = EXTRACT_LOCK.lock().unwrap();
    if is_valid_binary(&target) {
        return Ok(target);
    }

    fs::create_dir_all(&runtime_dir)
        .map_err(|e| format!("Failed to create runtime dir {}: {e}", runtime_dir.display()))?;

    let tmp = runtime_dir.join(format!("{exe_name}.tmp-{}", std::process::id()));
    {
        let mut decoder = GzDecoder::new(ENGINE_GZ);
        let mut out = File::create(&tmp)
            .map_err(|e| format!("Failed to create {}: {e}", tmp.display()))?;
        io::copy(&mut decoder, &mut out)
            .map_err(|e| format!("Failed to decompress embedded ZCode engine: {e}"))?;
        out.flush()
            .map_err(|e| format!("Failed to flush {}: {e}", tmp.display()))?;
    }

    // Best-effort cleanup of stale extracted engine binaries from older versions.
    if let Ok(entries) = fs::read_dir(&runtime_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let s = name.to_string_lossy();
            if s.starts_with("engine-") && s.ends_with(".exe") && s != exe_name {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    if let Err(e) = fs::rename(&tmp, &target) {
        if is_valid_binary(&target) {
            let _ = fs::remove_file(&tmp);
            return Ok(target);
        }
        let _ = fs::remove_file(&tmp);
        return Err(format!("Failed to finalize {}: {e}", target.display()));
    }

    Ok(target)
}

fn is_valid_binary(path: &Path) -> bool {
    path.metadata()
        .map(|m| m.is_file() && m.len() > 1_000_000)
        .unwrap_or(false)
}
