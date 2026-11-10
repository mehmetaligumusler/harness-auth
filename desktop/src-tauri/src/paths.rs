//! Filesystem layout for ZCode Manager.
//!
//! Everything the manager owns lives under `%APPDATA%\zcode-manager`:
//!
//! ```text
//! %APPDATA%\zcode-manager\
//!   settings.json              app settings
//!   accounts.json              encrypted account vault
//!   proxy\config.yaml          proxy config (ZCODE_PROXY_CONFIG)
//!   proxy\store\credentials.json  proxy credential store (ZCODE_PROXY_STORE_DIR)
//!   proxy\errors.log           proxy error log (ZCODE_ERROR_LOG)
//!   proxy\stdout.log           proxy child stdout/stderr (diagnostics)
//!   proxy-test\<id>\           ephemeral dirs for connection tests
//!   launches\                  generated Claude Code launch scripts
//! ```

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Paths {
    pub data_dir: PathBuf,
}

impl Paths {
    pub fn new() -> Self {
        // Portable/test layout override wins over the per-user appdata dir.
        if let Ok(dir) = std::env::var("ZCODE_MANAGER_DATA_DIR") {
            if !dir.trim().is_empty() {
                return Self {
                    data_dir: PathBuf::from(dir.trim()),
                };
            }
        }
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                // Fallback for unusual environments: home directory.
                let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
                PathBuf::from(home).join(".zcode-manager")
            });
        Self {
            data_dir: base.join("zcode-manager"),
        }
    }

    pub fn settings_file(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }
    pub fn vault_file(&self) -> PathBuf {
        self.data_dir.join("accounts.json")
    }
    pub fn proxy_dir(&self) -> PathBuf {
        self.data_dir.join("proxy")
    }
    pub fn proxy_config(&self) -> PathBuf {
        self.proxy_dir().join("config.yaml")
    }
    pub fn proxy_store_dir(&self) -> PathBuf {
        self.proxy_dir().join("store")
    }
    pub fn proxy_errors_log(&self) -> PathBuf {
        self.proxy_dir().join("errors.log")
    }
    pub fn proxy_stdout_log(&self) -> PathBuf {
        self.proxy_dir().join("stdout.log")
    }
    pub fn launches_dir(&self) -> PathBuf {
        self.data_dir.join("launches")
    }

    pub fn ensure_all(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.data_dir.join("proxy").join("store"))?;
        std::fs::create_dir_all(self.launches_dir())?;
        Ok(())
    }
}

/// Locate the zcode-proxy sidecar binary.
///
/// Resolution order:
/// 1. `ZCODE_MANAGER_PROXY_EXE` env override (tests / portable layouts).
/// 2. `zcode-proxy.exe` next to the manager executable (bundled sidecar).
/// 3. Repo root checkout (dev builds).
pub fn resolve_proxy_exe() -> PathBuf {
    if let Ok(p) = std::env::var("ZCODE_MANAGER_PROXY_EXE") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return p;
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let sidecar = dir.join("zcode-proxy.exe");
            if sidecar.is_file() {
                return sidecar;
            }
        }
    }
    // Dev fallback: <repo>/desktop/src-tauri -> <repo>/zcode-proxy.exe
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf());
    if let Some(root) = repo_root {
        let candidate = root.join("zcode-proxy.exe");
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from("zcode-proxy.exe")
}
