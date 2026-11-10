//! Persistent application settings (`settings.json`).

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::paths::Paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: String,       // "dark" | "light"
    pub language: String,    // "tr" | "en"
    pub panel_port: u16,     // control panel port (persisted for re-attach)
    /// Control panel bearer token. Never serialized to the frontend; the
    /// backend always preserves its own value on save.
    #[serde(skip_serializing)]
    pub panel_token: String,
    pub keep_proxy_on_exit: bool, // proxy keeps running when the app closes
    pub proxy_autostart: bool,    // start proxy when the app launches
    pub autostart_windows: bool,  // run the app at Windows logon
    pub start_minimized: bool,    // start hidden to tray
    pub close_to_tray: bool,      // window close button hides to tray
    pub default_provider: String, // "zai" | "bigmodel" (for new accounts)
    pub default_plan: String,     // "coding-plan" | "start-plan"
    pub default_model: String,
    pub claude_default_folder: Option<String>,
    pub recent_folders: Vec<String>,
    pub recent_models: Vec<String>,
    pub log_level: String, // "info" | "debug"
    pub update_check: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            language: "tr".into(),
            panel_port: 8318,
            panel_token: crate::crypto::random_token(),
            keep_proxy_on_exit: true,
            proxy_autostart: false,
            autostart_windows: false,
            start_minimized: false,
            close_to_tray: true,
            default_provider: "zai".into(),
            default_plan: "coding-plan".into(),
            default_model: "glm-5.3".into(),
            claude_default_folder: None,
            recent_folders: Vec::new(),
            recent_models: Vec::new(),
            log_level: "info".into(),
            update_check: true,
        }
    }
}

impl Settings {
    pub fn load(paths: &Paths) -> Self {
        Self::load_from(&paths.settings_file())
    }

    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
                eprintln!("settings.json unreadable ({e}); using defaults");
                Settings::default()
            }),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, paths: &Paths) -> Result<(), String> {
        let path = paths.settings_file();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("mkdir failed: {e}"))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| format!("write failed: {e}"))
    }

    /// Sync the Windows per-user Run key with `autostart_windows`/`start_minimized`.
    pub fn sync_autostart(&self) -> Result<(), String> {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run = hkcu
            .open_subkey_with_flags(
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                winreg::enums::KEY_SET_VALUE | winreg::enums::KEY_READ,
            )
            .map_err(|e| format!("Run key açılamadı: {e}"))?;

        let existing: Result<String, _> = run.get_value("ZCodeManager");
        if !self.autostart_windows {
            if existing.is_ok() {
                run.delete_value("ZCodeManager")
                    .map_err(|e| format!("Run kaydı silinemedi: {e}"))?;
            }
            return Ok(());
        }

        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut cmd = format!("\"{}\"", exe.display());
        if self.start_minimized {
            cmd.push_str(" --hidden");
        }
        if existing.as_deref().ok() != Some(cmd.as_str()) {
            run.set_value("ZCodeManager", &cmd)
                .map_err(|e| format!("Run kaydı yazılamadı: {e}"))?;
        }
        Ok(())
    }
}
