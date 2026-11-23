//! Multi-harness session vault and shared CLI/filesystem helpers.
//!
//! Manages encrypted sessions for:
//! - Claude Code (`claude`)
//! - OpenCode (`opencode`)
//! - Antigravity CLI (`agy`)

pub mod antigravity;
pub mod claude_harness;
pub mod opencode;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::crypto;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CliInfo {
    pub found: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessSessionRecord {
    pub id: String,
    /// "claude" | "opencode" | "antigravity"
    pub harness: String,
    /// "claude_oauth" | "opencode_snapshot" | "opencode_api" | "opencode_zcode_proxy" | "antigravity_oauth"
    pub kind: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub provider_hint: Option<String>,
    #[serde(default)]
    pub account_hint: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub meta: Value,
    pub encrypted: String,
    pub created_at: i64,
    #[serde(default)]
    pub last_used_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessSessionSummary {
    pub id: String,
    pub harness: String,
    pub kind: String,
    pub name: String,
    pub enabled: bool,
    pub provider_hint: Option<String>,
    pub account_hint: Option<String>,
    pub expires_at: Option<i64>,
    pub meta: Value,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub active: bool,
}

impl HarnessSessionRecord {
    pub fn summary(&self, active: bool) -> HarnessSessionSummary {
        HarnessSessionSummary {
            id: self.id.clone(),
            harness: self.harness.clone(),
            kind: self.kind.clone(),
            name: self.name.clone(),
            enabled: self.enabled,
            provider_hint: self.provider_hint.clone(),
            account_hint: self.account_hint.clone(),
            expires_at: self.expires_at,
            meta: self.meta.clone(),
            created_at: self.created_at,
            last_used_at: self.last_used_at,
            active,
        }
    }

    pub fn decrypt_payload(&self, vault_key: &[u8; 32]) -> Result<Value, String> {
        let json = crypto::open(vault_key, &self.encrypted)?;
        serde_json::from_str(&json).map_err(|e| format!("oturum verisi çözülemedi: {e}"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SessionVault {
    pub version: u32,
    pub sessions: Vec<HarnessSessionRecord>,
}

impl SessionVault {
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => SessionVault::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        atomic_write(path, &json)
    }

    pub fn build_record(
        harness: &str,
        kind: &str,
        name: &str,
        provider_hint: Option<String>,
        account_hint: Option<String>,
        expires_at: Option<i64>,
        meta: Value,
        payload: &Value,
        vault_key: &[u8; 32],
    ) -> Result<HarnessSessionRecord, String> {
        let raw = serde_json::to_string(payload).map_err(|e| e.to_string())?;
        let encrypted = crypto::seal(vault_key, &raw)?;
        Ok(HarnessSessionRecord {
            id: uuid::Uuid::new_v4().to_string(),
            harness: harness.to_string(),
            kind: kind.to_string(),
            name: name.to_string(),
            enabled: true,
            provider_hint,
            account_hint,
            expires_at,
            meta,
            encrypted,
            created_at: chrono::Utc::now().timestamp_millis(),
            last_used_at: None,
        })
    }

    pub fn summaries(
        &self,
        claude_active_id: Option<&str>,
        opencode_active_id: Option<&str>,
        antigravity_active_id: Option<&str>,
    ) -> Vec<HarnessSessionSummary> {
        self.sessions
            .iter()
            .map(|s| {
                let active = match s.harness.as_str() {
                    "claude" => claude_active_id == Some(s.id.as_str()),
                    "opencode" => opencode_active_id == Some(s.id.as_str()),
                    "antigravity" => antigravity_active_id == Some(s.id.as_str()),
                    _ => false,
                };
                s.summary(active && s.enabled)
            })
            .collect()
    }

    /// Check if any existing session in this harness decrypts to an identical payload.
    pub fn has_matching_payload(&self, harness: &str, vault_key: &[u8; 32], payload: &Value) -> bool {
        self.sessions
            .iter()
            .filter(|s| s.harness == harness)
            .any(|s| s.decrypt_payload(vault_key).ok().as_ref() == Some(payload))
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HarnessOAuthState {
    pub running: bool,
    pub harness: String,
    pub label: String,
    pub started_at: Option<i64>,
    #[serde(skip)]
    pub child_pid: Option<u32>,
    #[serde(skip)]
    pub temp_dir: Option<PathBuf>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessOAuthDone {
    pub ok: bool,
    pub harness: String,
    pub session_id: Option<String>,
    pub error: Option<String>,
}

pub fn user_home() -> Result<PathBuf, String> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .map_err(|_| "Kullanıcı ev dizini (USERPROFILE/HOME) bulunamadı".to_string())
}

pub fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("Klasör oluşturulamadı: {e}"))?;
    }
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let tmp = path.with_file_name(format!(".{file_name}.tmp-harnessauth"));
    std::fs::write(&tmp, content).map_err(|e| format!("Dosya yazılamadı: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Dosya taşınamadı: {e}")
    })
}

pub fn mask_secret(val: &str) -> String {
    let t = val.trim();
    if t.len() <= 8 {
        return "•".repeat(t.len().max(4));
    }
    format!("{}…{}", &t[..6], &t[t.len() - 4..])
}

fn hide(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

fn output_hidden(cmd: &mut Command) -> Option<std::process::Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    hide(cmd);
    cmd.output().ok()
}

pub fn detect_cli(binary_name: &str, extra_candidates: &[PathBuf]) -> CliInfo {
    let mut where_cmd = Command::new("where.exe");
    where_cmd.arg(binary_name);
    let mut path = output_hidden(&mut where_cmd).and_then(|out| {
        if out.status.success() {
            String::from_utf8(out.stdout)
                .ok()?
                .lines()
                .next()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
        } else {
            None
        }
    });

    if path.is_none() {
        path = extra_candidates
            .iter()
            .find(|p| p.is_file())
            .map(|p| p.display().to_string());
    }

    let version = if path.is_some() {
        let mut cmd = Command::new("cmd.exe");
        cmd.args(["/d", "/c", binary_name, "--version"]);
        output_hidden(&mut cmd).and_then(|out| {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                (!text.is_empty()).then_some(text)
            } else {
                None
            }
        })
    } else {
        None
    };

    CliInfo {
        found: path.is_some() || version.is_some(),
        version,
        path,
    }
}

/// Launch a CLI command inside a visible terminal window (Windows Terminal first,
/// falling back to classic `cmd.exe`), optionally injecting isolated env vars.
pub fn launch_in_terminal(
    launches_dir: &Path,
    title: &str,
    folder: &Path,
    env_pairs: &[(&str, &str)],
    command_line: &str,
    pause_on_error: bool,
) -> Result<u32, String> {
    if !folder.is_dir() {
        return Err(format!("Klasör bulunamadı: {}", folder.display()));
    }
    std::fs::create_dir_all(launches_dir).map_err(|e| format!("Klasör oluşturulamadı: {e}"))?;
    let mut lines: Vec<String> = vec!["@echo off".into(), format!("title HarnessAuth ^| {title}")];
    for (k, v) in env_pairs {
        lines.push(format!("set \"{k}={v}\""));
    }
    lines.push(format!("cd /d \"{}\"", folder.display()));
    lines.push(command_line.to_string());
    if pause_on_error {
        lines.push("if errorlevel 1 pause".into());
    }
    let script = launches_dir.join(format!(
        "launch-{}-{}.cmd",
        chrono::Utc::now().timestamp_millis(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&script, lines.join("\r\n"))
        .map_err(|e| format!("Başlatma betiği yazılamadı: {e}"))?;

    let mut wt = Command::new("wt.exe");
    wt.args(["new-tab", "--title", title])
        .arg("-d")
        .arg(folder)
        .args(["cmd", "/c"])
        .arg(&script)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide(&mut wt);
    if let Ok(child) = wt.spawn() {
        return Ok(child.id());
    }

    let mut cmd = Command::new("cmd.exe");
    cmd.args(["/d", "/c", "start", title, "cmd", "/c"])
        .arg(&script)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide(&mut cmd);
    let child = cmd
        .spawn()
        .map_err(|e| format!("{title} penceresi açılamadı: {e}"))?;
    Ok(child.id())
}

pub fn read_json_object(path: &Path) -> serde_json::Map<String, Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}
