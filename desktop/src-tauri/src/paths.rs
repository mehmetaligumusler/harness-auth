//! Filesystem layout for HarnessAuth.
//!
//! Everything the app owns lives under `%APPDATA%\harness-auth`:
//!
//! ```text
//! %APPDATA%\harness-auth\
//!   settings.json                app settings
//!   accounts.json                encrypted ZCode account vault
//!   sessions.json                encrypted multi-harness session vault
//!   proxy\config.yaml            proxy config (ZCODE_PROXY_CONFIG)
//!   proxy\store\credentials.json proxy credential store (ZCODE_PROXY_STORE_DIR)
//!   proxy\errors.log             proxy error log (ZCODE_ERROR_LOG)
//!   proxy\stdout.log             proxy child stdout/stderr (diagnostics)
//!   launches\                    generated harness launch scripts
//!   tmp\                         ephemeral isolated login dirs
//! ```

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Paths {
    pub data_dir: PathBuf,
}

impl Paths {
    pub fn new() -> Self {
        // Portable/test layout override wins over the per-user appdata dir.
        if let Ok(dir) = std::env::var("HARNESS_AUTH_DATA_DIR") {
            if !dir.trim().is_empty() {
                return Self {
                    data_dir: PathBuf::from(dir.trim()),
                };
            }
        }
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
                PathBuf::from(home).join(".harness-auth")
            });
        Self {
            data_dir: base.join("harness-auth"),
        }
    }

    pub fn legacy_zcode_dir() -> Option<PathBuf> {
        let base = std::env::var("APPDATA")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("USERPROFILE").map(|h| PathBuf::from(h).join(".zcode-manager")))
            .ok()?;
        let legacy = base.join("zcode-manager");
        legacy.exists().then_some(legacy)
    }

    pub fn settings_file(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }
    pub fn unified_vault_file(&self) -> PathBuf {
        self.data_dir.join("vault.json")
    }
    pub fn vault_file(&self) -> PathBuf {
        self.data_dir.join("accounts.json")
    }
    pub fn sessions_file(&self) -> PathBuf {
        self.data_dir.join("sessions.json")
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
    pub fn tmp_dir(&self) -> PathBuf {
        self.data_dir.join("tmp")
    }
    pub fn runtime_dir(&self) -> PathBuf {
        self.data_dir.join("runtime")
    }
    pub fn proxy_exe(&self) -> PathBuf {
        crate::embedded_engine::resolve_or_extract(self)
            .unwrap_or_else(|_| self.runtime_dir().join("engine.exe"))
    }

    pub fn ensure_all(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.data_dir.join("proxy").join("store"))?;
        std::fs::create_dir_all(self.launches_dir())?;
        std::fs::create_dir_all(self.tmp_dir())?;
        std::fs::create_dir_all(self.runtime_dir())?;
        Ok(())
    }
}

/// Locate (or extract on demand) the embedded headless ZCode engine binary.
pub fn resolve_proxy_exe() -> PathBuf {
    Paths::new().proxy_exe()
}
