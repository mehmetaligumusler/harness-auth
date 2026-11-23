//! Claude Code (`claude`) multi-method & multi-session management.
//!
//! Manages:
//! - `%USERPROFILE%\.claude\settings.json` (`env` block for ZCode OAuth / Z API / clean OAuth)
//! - `%USERPROFILE%\.claude\.credentials.json` (Claude OAuth tokens)
//! - `%USERPROFILE%\.claude.json` (`oauthAccount` metadata)

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::claude;
use crate::harness::{
    atomic_write, detect_cli, launch_in_terminal, mask_secret, read_json_object, user_home,
    CliInfo, HarnessOAuthDone, HarnessSessionRecord, SessionVault,
};
use crate::state::AppState;

const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

pub const MANAGED_CLAUDE_ENV: &[&str] = &[
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "ANTHROPIC_SMALL_FAST_MODEL",
    "CLAUDE_CODE_SUBAGENT_MODEL",
];

#[derive(Debug, Clone)]
pub struct ClaudePaths {
    pub settings_json: PathBuf,
    pub credentials_json: PathBuf,
    pub claude_json: PathBuf,
}

impl ClaudePaths {
    pub fn live() -> Result<Self, String> {
        let home = user_home()?;
        Ok(Self::under(&home))
    }

    pub fn under(home: &Path) -> Self {
        let claude_dir = home.join(".claude");
        Self {
            settings_json: claude_dir.join("settings.json"),
            credentials_json: claude_dir.join(".credentials.json"),
            claude_json: home.join(".claude.json"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeLiveState {
    pub cli: CliInfo,
    /// "claude_oauth" | "zcode_oauth" | "z_api" | "custom_api" | "disabled"
    pub detected_mode: String,
    pub has_credentials_file: bool,
    pub live_account_hint: Option<String>,
    pub live_expires_at: Option<i64>,
    pub live_base_url: Option<String>,
    pub live_model: Option<String>,
}

pub fn detect_claude_cli() -> CliInfo {
    let extra = user_home()
        .ok()
        .map(|h| {
            vec![
                h.join(r"AppData\Roaming\npm\claude.cmd"),
                h.join(r".local\bin\claude.exe"),
            ]
        })
        .unwrap_or_default();
    detect_cli("claude", &extra)
}

pub fn inspect_live() -> ClaudeLiveState {
    let cli = detect_claude_cli();
    let Ok(paths) = ClaudePaths::live() else {
        return ClaudeLiveState {
            cli,
            detected_mode: "disabled".into(),
            ..Default::default()
        };
    };
    let mut state = inspect_live_at(&paths);
    state.cli = cli;
    state
}

pub fn inspect_live_at(paths: &ClaudePaths) -> ClaudeLiveState {
    let settings = read_json_object(&paths.settings_json);
    let env = settings
        .get("env")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let base_url = env
        .get("ANTHROPIC_BASE_URL")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let auth_token = env
        .get("ANTHROPIC_AUTH_TOKEN")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let api_key = env
        .get("ANTHROPIC_API_KEY")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let model = env
        .get("ANTHROPIC_MODEL")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let creds = read_json_object(&paths.credentials_json);
    let oauth_obj = creds.get("claudeAiOauth").and_then(|v| v.as_object());
    let has_credentials_file = oauth_obj
        .and_then(|o| o.get("accessToken").and_then(|v| v.as_str()))
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    let live_expires_at = oauth_obj
        .and_then(|o| o.get("expiresAt"))
        .and_then(|v| v.as_i64());

    let claude_json = read_json_object(&paths.claude_json);
    let email = claude_json
        .get("oauthAccount")
        .and_then(|v| v.as_object())
        .and_then(|o| o.get("emailAddress").and_then(|v| v.as_str()))
        .map(|s| s.to_string());

    let live_account_hint = email
        .or_else(|| {
            oauth_obj
                .and_then(|o| o.get("accessToken").and_then(|v| v.as_str()))
                .map(mask_secret)
        })
        .or_else(|| auth_token.as_deref().map(mask_secret))
        .or_else(|| api_key.as_deref().map(mask_secret));

    let detected_mode = if let Some(url) = &base_url {
        let lower = url.to_ascii_lowercase();
        if lower.starts_with("http://127.0.0.1:") || lower.starts_with("http://localhost:") {
            "zcode_oauth"
        } else if lower.contains("api.z.ai") || lower.contains("open.bigmodel.cn") {
            "z_api"
        } else {
            "custom_api"
        }
    } else if api_key.is_some() {
        "custom_api"
    } else if has_credentials_file {
        "claude_oauth"
    } else {
        "disabled"
    };

    ClaudeLiveState {
        cli: CliInfo::default(),
        detected_mode: detected_mode.into(),
        has_credentials_file,
        live_account_hint,
        live_expires_at,
        live_base_url: base_url,
        live_model: model,
    }
}

pub fn capture_current_oauth_at(
    paths: &ClaudePaths,
    vault_key: &[u8; 32],
    name: Option<&str>,
) -> Result<HarnessSessionRecord, String> {
    let raw_creds = std::fs::read_to_string(&paths.credentials_json).map_err(|_| {
        "Aktif Claude OAuth oturumu bulunamadı (~/.claude/.credentials.json yok)".to_string()
    })?;
    let creds_val: Value =
        serde_json::from_str(&raw_creds).map_err(|e| format!("Geçersiz .credentials.json: {e}"))?;
    let oauth_obj = creds_val
        .get("claudeAiOauth")
        .and_then(|v| v.as_object())
        .ok_or_else(|| "Kimlik dosyasında claudeAiOauth alanı bulunamadı".to_string())?;
    let access_token = oauth_obj
        .get("accessToken")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Claude OAuth accessToken boş".to_string())?;

    let expires_at = oauth_obj.get("expiresAt").and_then(|v| v.as_i64());
    let claude_json = read_json_object(&paths.claude_json);
    let oauth_account = claude_json.get("oauthAccount").cloned();
    let email = oauth_account
        .as_ref()
        .and_then(|v| v.get("emailAddress"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let account_hint = email.clone().or_else(|| Some(mask_secret(access_token)));
    let display_name = name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| match &email {
            Some(e) => format!("Claude ({e})"),
            None => "Claude OAuth".to_string(),
        });

    let payload = json!({
        "credentials": creds_val,
        "oauthAccount": oauth_account,
    });
    let meta = json!({
        "email": email,
        "scopes": oauth_obj.get("scopes").cloned().unwrap_or(Value::Null),
    });

    SessionVault::build_record(
        "claude",
        "claude_oauth",
        &display_name,
        Some("anthropic".into()),
        account_hint,
        expires_at,
        meta,
        &payload,
        vault_key,
    )
}

fn patch_settings_env(
    settings_path: &Path,
    mutate: impl FnOnce(&mut Map<String, Value>),
) -> Result<(), String> {
    let mut root = read_json_object(settings_path);
    let mut env = root
        .get("env")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    mutate(&mut env);

    if env.is_empty() {
        root.remove("env");
    } else {
        root.insert("env".into(), Value::Object(env));
    }

    let formatted =
        serde_json::to_string_pretty(&Value::Object(root)).map_err(|e| e.to_string())?;
    atomic_write(settings_path, &formatted)
}

pub fn apply_claude_oauth_at(paths: &ClaudePaths, payload: &Value) -> Result<(), String> {
    let creds = payload
        .get("credentials")
        .filter(|v| v.is_object())
        .ok_or_else(|| "Oturum verisinde credentials nesnesi eksik".to_string())?;

    let creds_text = serde_json::to_string_pretty(creds).map_err(|e| e.to_string())?;
    atomic_write(&paths.credentials_json, &creds_text)?;

    if let Some(oauth_account) = payload.get("oauthAccount").filter(|v| !v.is_null()) {
        let mut root = read_json_object(&paths.claude_json);
        root.insert("oauthAccount".into(), oauth_account.clone());
        let text =
            serde_json::to_string_pretty(&Value::Object(root)).map_err(|e| e.to_string())?;
        atomic_write(&paths.claude_json, &text)?;
    }

    // Clear proxy/API env overrides while preserving user's own env keys.
    patch_settings_env(&paths.settings_json, |env| {
        for key in MANAGED_CLAUDE_ENV {
            env.remove(*key);
        }
    })
}

pub fn apply_env_bridge_at(
    paths: &ClaudePaths,
    base_url: &str,
    auth_token: &str,
    model: Option<&str>,
) -> Result<(), String> {
    if let Some(m) = model.filter(|m| !m.is_empty()) {
        if !claude::valid_model_name(m) {
            return Err(format!("Geçersiz model adı: {m}"));
        }
    }

    patch_settings_env(&paths.settings_json, |env| {
        for key in MANAGED_CLAUDE_ENV {
            env.remove(*key);
        }
        env.insert("ANTHROPIC_BASE_URL".into(), json!(base_url));
        env.insert("ANTHROPIC_AUTH_TOKEN".into(), json!(auth_token));
        if let Some(m) = model.filter(|m| !m.is_empty()) {
            for var in [
                "ANTHROPIC_MODEL",
                "ANTHROPIC_DEFAULT_OPUS_MODEL",
                "ANTHROPIC_DEFAULT_SONNET_MODEL",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL",
                "ANTHROPIC_SMALL_FAST_MODEL",
                "CLAUDE_CODE_SUBAGENT_MODEL",
            ] {
                env.insert(var.into(), json!(m));
            }
        }
    })
}

pub fn resolve_z_api_base(provider: &str, custom_base_url: Option<&str>) -> Result<String, String> {
    if let Some(custom) = custom_base_url.map(str::trim).filter(|s| !s.is_empty()) {
        if !custom.starts_with("https://") && !custom.starts_with("http://") {
            return Err("Özel API adresi http:// veya https:// ile başlamalı".into());
        }
        return Ok(custom.trim_end_matches('/').to_string());
    }
    match provider {
        "zai" => Ok("https://api.z.ai/api/anthropic".into()),
        "bigmodel" => Ok("https://open.bigmodel.cn/api/anthropic".into()),
        other => Err(format!("Desteklenmeyen Z API sağlayıcısı: {other}")),
    }
}

/// Disable active Claude Code method. If `clear_oauth_credentials` is true and
/// a live `.credentials.json` is not yet backed up in `vault`, creates a backup
/// record first and returns it so no session is ever lost.
pub fn disable_at(
    paths: &ClaudePaths,
    clear_oauth_credentials: bool,
    vault: &SessionVault,
    vault_key: &[u8; 32],
) -> Result<Option<HarnessSessionRecord>, String> {
    let mut backup: Option<HarnessSessionRecord> = None;

    if clear_oauth_credentials && paths.credentials_json.is_file() {
        if let Ok(rec) = capture_current_oauth_at(paths, vault_key, Some("Claude OAuth (Yedek)")) {
            if let Ok(payload) = rec.decrypt_payload(vault_key) {
                if !vault.has_matching_payload("claude", vault_key, &payload) {
                    backup = Some(rec);
                }
            }
        }
    }

    patch_settings_env(&paths.settings_json, |env| {
        for key in MANAGED_CLAUDE_ENV {
            env.remove(*key);
        }
    })?;

    if clear_oauth_credentials {
        if paths.credentials_json.is_file() {
            let _ = std::fs::remove_file(&paths.credentials_json);
        }
        if paths.claude_json.is_file() {
            let mut root = read_json_object(&paths.claude_json);
            if root.remove("oauthAccount").is_some() {
                let text = serde_json::to_string_pretty(&Value::Object(root))
                    .map_err(|e| e.to_string())?;
                atomic_write(&paths.claude_json, &text)?;
            }
        }
    }

    Ok(backup)
}

pub fn start_oauth_login(app: &AppHandle, name_hint: Option<String>) -> Result<(), String> {
    let cli = detect_claude_cli();
    if !cli.found {
        return Err("Claude Code CLI (`claude`) sistemde bulunamadı".into());
    }

    let (launches_dir, temp_dir) = {
        let state = app.state::<AppState>();
        let mut flow = state.harness_oauth.lock().unwrap();
        if flow.running {
            return Err("Zaten devam eden bir oturum ekleme akışı var".into());
        }
        let temp_dir = state
            .paths
            .tmp_dir()
            .join(format!("claude-login-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Geçici oturum klasörü oluşturulamadı: {e}"))?;
        flow.running = true;
        flow.harness = "claude".into();
        flow.label = name_hint.clone().unwrap_or_default();
        flow.started_at = Some(chrono::Utc::now().timestamp_millis());
        flow.temp_dir = Some(temp_dir.clone());
        (state.paths.launches_dir(), temp_dir)
    };

    let home = user_home().unwrap_or_else(|_| launches_dir.clone());
    let temp_str = temp_dir.display().to_string();
    let pid = launch_in_terminal(
        &launches_dir,
        "Claude Code OAuth Giriş",
        &home,
        &[("CLAUDE_CONFIG_DIR", &temp_str)],
        "claude auth login",
        true,
    )?;

    {
        let state = app.state::<AppState>();
        state.harness_oauth.lock().unwrap().child_pid = Some(pid);
    }

    let app = app.clone();
    let name_hint = name_hint.unwrap_or_default();
    std::thread::spawn(move || poll_claude_login(app, temp_dir, name_hint));
    Ok(())
}

fn poll_claude_login(app: AppHandle, temp_dir: PathBuf, name_hint: String) {
    let isolated_paths = ClaudePaths {
        settings_json: temp_dir.join("settings.json"),
        credentials_json: temp_dir.join(".credentials.json"),
        claude_json: temp_dir.join(".claude.json"),
    };

    let deadline = Instant::now() + LOGIN_TIMEOUT;
    let mut captured: Result<Option<String>, String> = Err("Zaman aşımı (5 dk)".into());

    loop {
        let still_running = {
            let state = app.state::<AppState>();
            let running = state.harness_oauth.lock().unwrap().running;
            running
        };
        if !still_running {
            captured = Err("Giriş iptal edildi".into());
            break;
        }
        if Instant::now() > deadline {
            break;
        }
        if isolated_paths.credentials_json.is_file() {
            let state = app.state::<AppState>();
            let vault_key = state.vault_key();
            if let Ok(rec) = capture_current_oauth_at(
                &isolated_paths,
                &vault_key,
                (!name_hint.trim().is_empty()).then_some(name_hint.as_str()),
            ) {
                let id = rec.id.clone();
                let mut sessions = state.sessions.lock().unwrap();
                sessions.sessions.push(rec);
                if let Err(e) = sessions.save(&state.paths.sessions_file()) {
                    captured = Err(e);
                } else {
                    captured = Ok(Some(id));
                }
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    let _ = std::fs::remove_dir_all(&temp_dir);
    {
        let state = app.state::<AppState>();
        let mut flow = state.harness_oauth.lock().unwrap();
        flow.running = false;
        flow.child_pid = None;
        flow.temp_dir = None;
    }

    let done = match captured {
        Ok(session_id) => HarnessOAuthDone {
            ok: true,
            harness: "claude".into(),
            session_id,
            error: None,
        },
        Err(e) => HarnessOAuthDone {
            ok: false,
            harness: "claude".into(),
            session_id: None,
            error: Some(e),
        },
    };
    let _ = app.emit("harness-oauth://done", &done);
}

pub fn launch_claude_folder(launches_dir: &Path, folder: &Path) -> Result<(), String> {
    launch_in_terminal(launches_dir, "Claude Code", folder, &[], "claude", true).map(|_| ())
}

fn validate_claude_provider(provider_id: &str) -> Result<String, String> {
    let pid = provider_id.trim();
    if pid.is_empty()
        || !pid
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err("Geçersiz sağlayıcı kimliği (yalnızca harf, rakam, - ve _)".into());
    }
    Ok(pid.to_string())
}

fn clean_optional_base_url(base_url: Option<&str>) -> Result<Option<String>, String> {
    let clean = base_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_end_matches('/').to_string());
    if let Some(url) = &clean {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("Base URL http:// veya https:// ile başlamalı".into());
        }
    }
    Ok(clean)
}

fn validate_api_model_name(model: &str) -> Result<(), String> {
    let valid = !model.is_empty()
        && model.len() <= 128
        && model.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '[' | ']' | '/' | ':' | '@')
        });
    if valid {
        Ok(())
    } else {
        Err(format!("Geçersiz model adı: {model}"))
    }
}

pub fn create_api_session(
    vault_key: &[u8; 32],
    name: &str,
    provider_id: &str,
    api_key: &str,
    base_url: Option<&str>,
    model: Option<&str>,
) -> Result<HarnessSessionRecord, String> {
    let pid = validate_claude_provider(provider_id)?;
    let key = api_key.trim();
    if key.len() < 4 {
        return Err("API Key çok kısa".into());
    }
    let clean_base = clean_optional_base_url(base_url)?;
    let clean_model = model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);
    if let Some(m) = &clean_model {
        validate_api_model_name(m)?;
    }

    let display_name = if name.trim().is_empty() {
        format!("Claude API — {pid}")
    } else {
        name.trim().to_string()
    };

    let payload = json!({
        "providerId": pid,
        "apiKey": key,
        "baseUrl": clean_base,
        "model": clean_model,
    });
    let meta = json!({
        "providerId": pid,
        "baseUrl": clean_base,
        "model": clean_model,
    });

    SessionVault::build_record(
        "claude",
        "claude_api",
        &display_name,
        Some(pid),
        Some(mask_secret(key)),
        None,
        meta,
        &payload,
        vault_key,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn update_api_session(
    rec: &mut HarnessSessionRecord,
    vault_key: &[u8; 32],
    name: Option<&str>,
    provider_id: Option<&str>,
    api_key: Option<&str>,
    base_url: Option<&str>,
    model: Option<&str>,
) -> Result<(), String> {
    if let Some(n) = name.map(str::trim).filter(|s| !s.is_empty()) {
        rec.name = n.to_string();
    }
    if rec.kind != "claude_api" {
        return Ok(());
    }
    let mut payload = rec.decrypt_payload(vault_key)?;
    let cur_pid = payload
        .get("providerId")
        .and_then(|v| v.as_str())
        .unwrap_or("anthropic")
        .to_string();
    let pid = match provider_id {
        Some(p) => validate_claude_provider(p)?,
        None => cur_pid,
    };
    let key = match api_key.map(str::trim).filter(|s| !s.is_empty()) {
        Some(k) => {
            if k.len() < 4 {
                return Err("API Key çok kısa".into());
            }
            k.to_string()
        }
        None => payload
            .get("apiKey")
            .and_then(|v| v.as_str())
            .ok_or("Mevcut API Key bulunamadı")?
            .to_string(),
    };
    let clean_base = match base_url {
        Some(raw) => clean_optional_base_url(Some(raw))?,
        None => payload
            .get("baseUrl")
            .and_then(|v| v.as_str())
            .map(String::from),
    };
    let clean_model = match model {
        Some(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                validate_api_model_name(trimmed)?;
                Some(trimmed.to_string())
            }
        }
        None => payload
            .get("model")
            .and_then(|v| v.as_str())
            .map(String::from),
    };

    payload = json!({
        "providerId": pid,
        "apiKey": key,
        "baseUrl": clean_base,
        "model": clean_model,
    });
    let sealed = crate::crypto::seal(vault_key, &serde_json::to_string(&payload).map_err(|e| e.to_string())?)?;
    rec.encrypted = sealed;
    rec.provider_hint = Some(pid.clone());
    rec.account_hint = Some(mask_secret(&key));
    rec.meta = json!({
        "providerId": pid,
        "baseUrl": clean_base,
        "model": clean_model,
    });
    Ok(())
}

pub fn apply_claude_api_at(
    paths: &ClaudePaths,
    payload: &Value,
    model_override: Option<&str>,
) -> Result<(), String> {
    let api_key = payload
        .get("apiKey")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or("Oturum verisinde apiKey eksik")?;
    let base_url = payload
        .get("baseUrl")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let model = model_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            payload
                .get("model")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
        });

    if let Some(m) = model {
        validate_api_model_name(m)?;
    }

    patch_settings_env(&paths.settings_json, |env| {
        for key in MANAGED_CLAUDE_ENV {
            env.remove(*key);
        }
        if let Some(url) = base_url {
            env.insert("ANTHROPIC_BASE_URL".into(), json!(url));
            env.insert("ANTHROPIC_AUTH_TOKEN".into(), json!(api_key));
        } else {
            env.insert("ANTHROPIC_API_KEY".into(), json!(api_key));
        }
        if let Some(m) = model {
            for var in [
                "ANTHROPIC_MODEL",
                "ANTHROPIC_DEFAULT_OPUS_MODEL",
                "ANTHROPIC_DEFAULT_SONNET_MODEL",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL",
                "ANTHROPIC_SMALL_FAST_MODEL",
                "CLAUDE_CODE_SUBAGENT_MODEL",
            ] {
                env.insert(var.into(), json!(m));
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "ha-claude-test-{tag}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn key() -> [u8; 32] {
        crate::crypto::derive_vault_key("test-master-secret")
    }

    #[test]
    fn settings_preserve_user_hooks_and_custom_env() {
        let home = temp_home("preserve");
        let paths = ClaudePaths::under(&home);
        std::fs::create_dir_all(paths.settings_json.parent().unwrap()).unwrap();
        std::fs::write(
            &paths.settings_json,
            r#"{
              "permissions": { "allow": ["Bash(git:*)"] },
              "env": { "CUSTOM_USER_VAR": "keep-me", "ANTHROPIC_BASE_URL": "old" }
            }"#,
        )
        .unwrap();

        apply_env_bridge_at(&paths, "https://api.z.ai/api/anthropic", "zk-12345678", Some("glm-5.3"))
            .unwrap();
        let after = read_json_object(&paths.settings_json);
        assert_eq!(after["permissions"]["allow"][0], "Bash(git:*)");
        assert_eq!(after["env"]["CUSTOM_USER_VAR"], "keep-me");
        assert_eq!(after["env"]["ANTHROPIC_BASE_URL"], "https://api.z.ai/api/anthropic");
        assert_eq!(after["env"]["ANTHROPIC_AUTH_TOKEN"], "zk-12345678");
        assert_eq!(after["env"]["ANTHROPIC_MODEL"], "glm-5.3");

        let live = inspect_live_at(&paths);
        assert_eq!(live.detected_mode, "z_api");

        // Disable removes only managed keys and leaves CUSTOM_USER_VAR + permissions intact.
        let vault = SessionVault::default();
        let backup = disable_at(&paths, false, &vault, &key()).unwrap();
        assert!(backup.is_none());
        let cleaned = read_json_object(&paths.settings_json);
        assert_eq!(cleaned["permissions"]["allow"][0], "Bash(git:*)");
        assert_eq!(cleaned["env"]["CUSTOM_USER_VAR"], "keep-me");
        assert!(cleaned["env"].get("ANTHROPIC_BASE_URL").is_none());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn capture_apply_and_auto_backup_claude_oauth() {
        let home = temp_home("oauth");
        let paths = ClaudePaths::under(&home);
        std::fs::create_dir_all(paths.credentials_json.parent().unwrap()).unwrap();
        std::fs::write(
            &paths.credentials_json,
            r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-secret","refreshToken":"rt-1","expiresAt":1800000000000}}"#,
        )
        .unwrap();
        std::fs::write(
            &paths.claude_json,
            r#"{"theme":"dark","oauthAccount":{"emailAddress":"dev@example.com"}}"#,
        )
        .unwrap();

        let rec = capture_current_oauth_at(&paths, &key(), None).unwrap();
        assert_eq!(rec.name, "Claude (dev@example.com)");
        assert_eq!(rec.account_hint.as_deref(), Some("dev@example.com"));

        // Switching to ZCode OAuth proxy preserves credentials unless explicitly cleared,
        // and sets detected_mode = zcode_oauth.
        apply_env_bridge_at(&paths, "http://127.0.0.1:8317", "proxy-key", Some("glm-5.3")).unwrap();
        assert_eq!(inspect_live_at(&paths).detected_mode, "zcode_oauth");

        // Applying Claude OAuth clears bridge env and restores credentials.
        let payload = rec.decrypt_payload(&key()).unwrap();
        apply_claude_oauth_at(&paths, &payload).unwrap();
        assert_eq!(inspect_live_at(&paths).detected_mode, "claude_oauth");
        let cj = read_json_object(&paths.claude_json);
        assert_eq!(cj["theme"], "dark");
        assert_eq!(cj["oauthAccount"]["emailAddress"], "dev@example.com");

        // Disable with clear_oauth_credentials=true creates an auto-backup if vault lacks it.
        let empty_vault = SessionVault::default();
        let backup = disable_at(&paths, true, &empty_vault, &key()).unwrap();
        assert!(backup.is_some());
        assert!(!paths.credentials_json.exists());
        assert!(read_json_object(&paths.claude_json).get("oauthAccount").is_none());
        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn claude_api_official_and_custom_provider_roundtrip() {
        let home = temp_home("api-session");
        let paths = ClaudePaths::under(&home);

        // 1) Official Anthropic API (no baseUrl -> uses ANTHROPIC_API_KEY)
        let mut rec = create_api_session(
            &key(),
            "Anthropic Direct",
            "anthropic",
            "sk-ant-api03-11112222",
            None,
            Some("claude-sonnet-4-5"),
        )
        .unwrap();
        assert_eq!(rec.kind, "claude_api");
        let p1 = rec.decrypt_payload(&key()).unwrap();
        apply_claude_api_at(&paths, &p1, None).unwrap();
        let env1 = read_json_object(&paths.settings_json)["env"].clone();
        assert_eq!(env1["ANTHROPIC_API_KEY"], "sk-ant-api03-11112222");
        assert!(env1.get("ANTHROPIC_BASE_URL").is_none());
        assert_eq!(inspect_live_at(&paths).detected_mode, "custom_api");

        // 2) Update to OpenRouter with preserved key -> switches to BASE_URL + AUTH_TOKEN
        update_api_session(
            &mut rec,
            &key(),
            Some("OpenRouter Pro"),
            Some("openrouter"),
            None,
            Some("https://openrouter.ai/api"),
            Some("anthropic/claude-sonnet-4"),
        )
        .unwrap();
        assert_eq!(rec.name, "OpenRouter Pro");
        let p2 = rec.decrypt_payload(&key()).unwrap();
        apply_claude_api_at(&paths, &p2, None).unwrap();
        let env2 = read_json_object(&paths.settings_json)["env"].clone();
        assert!(env2.get("ANTHROPIC_API_KEY").is_none());
        assert_eq!(env2["ANTHROPIC_BASE_URL"], "https://openrouter.ai/api");
        assert_eq!(env2["ANTHROPIC_AUTH_TOKEN"], "sk-ant-api03-11112222");
        assert_eq!(env2["ANTHROPIC_MODEL"], "anthropic/claude-sonnet-4");

        let _ = std::fs::remove_dir_all(&home);
    }
}
