//! Antigravity CLI (`agy`) multi-account session management.
//!
//! Real Windows storage layout:
//! - `%USERPROFILE%\.gemini\oauth_creds.json` (OAuth JSON tokens)
//! - `%USERPROFILE%\.gemini\google_accounts.json` (`{ "active": "<email>", "old": [...] }`)
//! - Optional mirror: `%USERPROFILE%\.gemini\antigravity-cli\antigravity-oauth-token`
//! - Optional keyring: `service = "gemini"`, `account = "antigravity"`

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::harness::{
    atomic_write, detect_cli, launch_in_terminal, mask_secret, read_json_object, user_home,
    CliInfo, HarnessOAuthDone, HarnessSessionRecord, SessionVault,
};
use crate::state::AppState;
use crate::unified_vault::{UnifiedAccount, UnifiedVault};

const KEYRING_SERVICE: &str = "gemini";
const KEYRING_ACCOUNT: &str = "antigravity";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
pub struct AntigravityPaths {
    pub oauth_creds_json: PathBuf,
    pub google_accounts_json: PathBuf,
    pub token_mirror: PathBuf,
    pub settings_json: PathBuf,
}

impl AntigravityPaths {
    pub fn live() -> Result<Self, String> {
        let home = user_home()?;
        Ok(Self::under(&home))
    }

    pub fn under(home: &Path) -> Self {
        let gemini = home.join(".gemini");
        let cli_dir = gemini.join("antigravity-cli");
        Self {
            oauth_creds_json: gemini.join("oauth_creds.json"),
            google_accounts_json: gemini.join("google_accounts.json"),
            token_mirror: cli_dir.join("antigravity-oauth-token"),
            settings_json: cli_dir.join("settings.json"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityLiveState {
    pub cli: CliInfo,
    pub has_keyring_token: bool,
    pub has_file_token: bool,
    pub active_email: Option<String>,
    pub token_hint: Option<String>,
    pub expiry_date: Option<i64>,
    pub is_expired: bool,
    pub scope: Option<String>,
}

pub fn detect_agy_cli() -> CliInfo {
    let mut extra = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        extra.push(PathBuf::from(&local).join(r"agy\bin\agy.exe"));
        extra.push(PathBuf::from(&local).join(r"Programs\Antigravity\agy.exe"));
    }
    if let Ok(home) = user_home() {
        extra.push(home.join(r"AppData\Local\agy\bin\agy.exe"));
        extra.push(home.join(r"AppData\Roaming\npm\agy.cmd"));
        extra.push(home.join(r"AppData\Local\Programs\Antigravity\agy.exe"));
        extra.push(home.join(r".local\bin\agy.exe"));
    }
    detect_cli("agy", &extra)
}

fn read_keyring_token() -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).ok()?;
    entry.get_password().ok().filter(|s| !s.trim().is_empty())
}

fn write_keyring_token(raw: &str) {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT) {
        let _ = entry.set_password(raw);
    }
}

fn delete_keyring_token() {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT) {
        let _ = entry.delete_credential();
    }
}

pub fn parse_token_json(raw: &str) -> Result<Value, String> {
    let v: Value = serde_json::from_str(raw.trim())
        .map_err(|e| format!("Geçersiz Google OAuth token JSON: {e}"))?;
    let obj = v
        .as_object()
        .ok_or_else(|| "Token JSON bir nesne olmalı".to_string())?;
    let has_access = obj
        .get("access_token")
        .and_then(|x| x.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    let has_refresh = obj
        .get("refresh_token")
        .and_then(|x| x.as_str())
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    if !has_access && !has_refresh {
        return Err("Token JSON içinde access_token veya refresh_token eksik".into());
    }
    Ok(v)
}

pub fn inspect_token_value(val: &Value) -> (Option<String>, Option<i64>, bool, Option<String>) {
    let token_hint = val
        .get("refresh_token")
        .or_else(|| val.get("access_token"))
        .and_then(|v| v.as_str())
        .map(mask_secret);
    let expiry_date = val.get("expiry_date").and_then(|v| v.as_i64());
    let now_ms = chrono::Utc::now().timestamp_millis();
    let is_expired = expiry_date.map(|e| e < now_ms).unwrap_or(false);
    let scope = val.get("scope").and_then(|v| v.as_str()).map(String::from);
    (token_hint, expiry_date, is_expired, scope)
}

fn read_active_email_at(paths: &AntigravityPaths) -> Option<String> {
    read_json_object(&paths.google_accounts_json)
        .get("active")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn read_file_token_at(paths: &AntigravityPaths) -> Option<String> {
    std::fs::read_to_string(&paths.token_mirror)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::fs::read_to_string(&paths.oauth_creds_json)
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
}

pub fn inspect_live() -> AntigravityLiveState {
    let cli = detect_agy_cli();
    let kr_raw = read_keyring_token();
    let has_keyring_token = kr_raw.is_some();

    let paths = AntigravityPaths::live().ok();
    let file_raw = paths.as_ref().and_then(read_file_token_at);
    let active_email = paths.as_ref().and_then(read_active_email_at);
    let has_file_token = file_raw.is_some();

    let parsed = kr_raw
        .as_deref()
        .and_then(|s| parse_token_json(s).ok())
        .or_else(|| file_raw.as_deref().and_then(|s| parse_token_json(s).ok()));

    let (token_hint, expiry_date, is_expired, scope) = match &parsed {
        Some(v) => inspect_token_value(v),
        None => (None, None, false, None),
    };

    AntigravityLiveState {
        cli,
        has_keyring_token,
        has_file_token,
        active_email,
        token_hint,
        expiry_date,
        is_expired,
        scope,
    }
}

pub fn capture_from_raw(
    raw: &str,
    vault_key: &[u8; 32],
    name: Option<&str>,
    email: Option<&str>,
) -> Result<HarnessSessionRecord, String> {
    let mut parsed = parse_token_json(raw)?;
    let (token_hint, expiry_date, _is_expired, scope) = inspect_token_value(&parsed);

    let clean_email = email
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| {
            parsed
                .get("email")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
        });

    if let (Some(em), Some(obj)) = (&clean_email, parsed.as_object_mut()) {
        obj.insert("email".into(), json!(em));
    }

    let display_hint = clean_email.clone().or(token_hint);
    let display_name = name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| match &display_hint {
            Some(h) => format!("Antigravity ({h})"),
            None => "Antigravity OAuth".to_string(),
        });

    let meta = json!({
        "scope": scope,
        "email": clean_email,
        "tokenType": parsed.get("token_type").cloned().unwrap_or(json!("Bearer")),
    });

    SessionVault::build_record(
        "antigravity",
        "antigravity_oauth",
        &display_name,
        Some("google".into()),
        display_hint,
        expiry_date,
        meta,
        &parsed,
        vault_key,
    )
}

pub fn capture_current_at(
    paths: &AntigravityPaths,
    vault_key: &[u8; 32],
    name: Option<&str>,
    include_keyring: bool,
) -> Result<HarnessSessionRecord, String> {
    let kr = if include_keyring {
        read_keyring_token()
    } else {
        None
    };
    let raw = kr
        .or_else(|| read_file_token_at(paths))
        .ok_or_else(|| {
            "Aktif Antigravity CLI oturumu bulunamadı (`~/.gemini/oauth_creds.json` ve keyring boş)"
                .to_string()
        })?;
    let email = read_active_email_at(paths);
    capture_from_raw(&raw, vault_key, name, email.as_deref())
}

pub fn capture_current(
    vault_key: &[u8; 32],
    name: Option<&str>,
) -> Result<HarnessSessionRecord, String> {
    let paths = AntigravityPaths::live()?;
    capture_current_at(&paths, vault_key, name, true)
}

pub fn write_token_files_at(paths: &AntigravityPaths, token_val: &Value) -> Result<(), String> {
    // Store pure OAuth fields in `oauth_creds.json` + mirror, while updating `google_accounts.json` if email exists.
    let mut pure_creds = token_val.clone();
    let email = token_val
        .get("email")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);
    if let Some(obj) = pure_creds.as_object_mut() {
        obj.remove("email");
    }

    let pretty = serde_json::to_string_pretty(&pure_creds).map_err(|e| e.to_string())?;
    atomic_write(&paths.oauth_creds_json, &pretty)?;
    let compact = serde_json::to_string(&pure_creds).map_err(|e| e.to_string())?;
    atomic_write(&paths.token_mirror, &compact)?;

    if let Some(em) = email {
        let mut ga = read_json_object(&paths.google_accounts_json);
        let prev_active = ga
            .get("active")
            .and_then(|v| v.as_str())
            .map(String::from)
            .filter(|p| !p.is_empty() && p != &em);
        let mut old: Vec<String> = ga
            .get("old")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .filter(|s| s != &em)
                    .collect()
            })
            .unwrap_or_default();
        if let Some(prev) = prev_active {
            if !old.contains(&prev) {
                old.insert(0, prev);
            }
        }
        ga.insert("active".into(), json!(em));
        ga.insert("old".into(), json!(old));
        let ga_text = serde_json::to_string_pretty(&ga).map_err(|e| e.to_string())?;
        atomic_write(&paths.google_accounts_json, &ga_text)?;
    }
    Ok(())
}

pub fn clear_token_files_at(paths: &AntigravityPaths) {
    if paths.oauth_creds_json.is_file() {
        let _ = std::fs::remove_file(&paths.oauth_creds_json);
    }
    if paths.token_mirror.is_file() {
        let _ = std::fs::remove_file(&paths.token_mirror);
    }
    if paths.google_accounts_json.is_file() {
        let mut ga = read_json_object(&paths.google_accounts_json);
        ga.insert("active".into(), Value::Null);
        if let Ok(text) = serde_json::to_string_pretty(&ga) {
            let _ = atomic_write(&paths.google_accounts_json, &text);
        }
    }
}

pub fn unified_from_agy_session(rec: HarnessSessionRecord) -> UnifiedAccount {
    UnifiedAccount {
        id: rec.id,
        name: rec.name,
        kind: "antigravity_oauth".into(),
        provider: "google".into(),
        enabled: rec.enabled,
        key_hint: rec.account_hint.clone().unwrap_or_default(),
        user_hint: rec.account_hint,
        base_url: None,
        package: None,
        default_plan: None,
        default_model: None,
        models: Vec::new(),
        has_jwt: false,
        has_secret: true,
        expires_at: rec.expires_at,
        created_at: rec.created_at,
        last_used_at: rec.last_used_at,
        encrypted: rec.encrypted,
    }
}

pub fn capture_unified_current_at(
    paths: &AntigravityPaths,
    vault_key: &[u8; 32],
    name: Option<&str>,
    include_keyring: bool,
) -> Result<UnifiedAccount, String> {
    let rec = capture_current_at(paths, vault_key, name, include_keyring)?;
    Ok(unified_from_agy_session(rec))
}

pub fn capture_unified_current(
    vault_key: &[u8; 32],
    name: Option<&str>,
) -> Result<UnifiedAccount, String> {
    let rec = capture_current(vault_key, name)?;
    Ok(unified_from_agy_session(rec))
}

pub fn activate_unified_at(
    paths: &AntigravityPaths,
    record: &UnifiedAccount,
    vault_key: &[u8; 32],
    update_keyring: bool,
) -> Result<(), String> {
    let token_val = record.decrypt_payload(vault_key)?;
    write_token_files_at(paths, &token_val)?;
    if update_keyring {
        if let Ok(compact) = serde_json::to_string(&token_val) {
            write_keyring_token(&compact);
        }
    }
    Ok(())
}

pub fn activate_unified(record: &UnifiedAccount, vault_key: &[u8; 32]) -> Result<(), String> {
    let paths = AntigravityPaths::live()?;
    activate_unified_at(&paths, record, vault_key, true)
}

pub fn disable_unified_at(
    paths: &AntigravityPaths,
    vault: &UnifiedVault,
    vault_key: &[u8; 32],
    clear_keyring: bool,
) -> Result<Option<UnifiedAccount>, String> {
    let mut backup: Option<UnifiedAccount> = None;

    if let Ok(rec) =
        capture_unified_current_at(paths, vault_key, Some("Antigravity (Yedek)"), clear_keyring)
    {
        if let Ok(payload) = rec.decrypt_payload(vault_key) {
            let already_saved = vault
                .accounts
                .iter()
                .filter(|a| a.kind == "antigravity_oauth")
                .any(|a| {
                    a.decrypt_payload(vault_key)
                        .ok()
                        .map(|p| {
                            p.get("refresh_token") == payload.get("refresh_token")
                                || p == payload
                        })
                        .unwrap_or(false)
                });
            if !already_saved {
                backup = Some(rec);
            }
        }
    }

    if clear_keyring {
        delete_keyring_token();
    }
    clear_token_files_at(paths);
    Ok(backup)
}

pub fn disable_unified(
    vault: &UnifiedVault,
    vault_key: &[u8; 32],
) -> Result<Option<UnifiedAccount>, String> {
    let paths = AntigravityPaths::live()?;
    disable_unified_at(&paths, vault, vault_key, true)
}

pub fn start_oauth_login(app: &AppHandle, name_hint: Option<String>) -> Result<(), String> {
    let cli = detect_agy_cli();
    if !cli.found {
        return Err("Antigravity CLI (`agy`) sistemde bulunamadı".into());
    }

    let paths = AntigravityPaths::live()?;
    let prev_keyring = read_keyring_token();
    let prev_mirror = std::fs::read_to_string(&paths.token_mirror).ok();
    let prev_creds = std::fs::read_to_string(&paths.oauth_creds_json).ok();
    let prev_accounts = std::fs::read_to_string(&paths.google_accounts_json).ok();

    let launches_dir = {
        let state = app.state::<AppState>();
        let mut flow = state.harness_oauth.lock().unwrap();
        if flow.running {
            return Err("Zaten devam eden bir oturum ekleme akışı var".into());
        }
        flow.running = true;
        flow.harness = "antigravity".into();
        flow.label = name_hint.clone().unwrap_or_default();
        flow.started_at = Some(chrono::Utc::now().timestamp_millis());
        flow.temp_dir = None;
        state.paths.launches_dir()
    };

    delete_keyring_token();
    clear_token_files_at(&paths);

    let home = user_home().unwrap_or_else(|_| launches_dir.clone());
    let pid = match launch_in_terminal(&launches_dir, "Antigravity CLI Giriş", &home, &[], "agy", true) {
        Ok(pid) => pid,
        Err(e) => {
            restore_live_slot(
                &paths,
                prev_keyring.as_deref(),
                prev_mirror.as_deref(),
                prev_creds.as_deref(),
                prev_accounts.as_deref(),
            );
            let state = app.state::<AppState>();
            state.harness_oauth.lock().unwrap().running = false;
            return Err(e);
        }
    };

    {
        let state = app.state::<AppState>();
        state.harness_oauth.lock().unwrap().child_pid = Some(pid);
    }

    let app = app.clone();
    let name_hint = name_hint.unwrap_or_default();
    std::thread::spawn(move || {
        poll_agy_login(
            app,
            paths,
            prev_keyring,
            prev_mirror,
            prev_creds,
            prev_accounts,
            name_hint,
        )
    });
    Ok(())
}

fn restore_live_slot(
    paths: &AntigravityPaths,
    prev_kr: Option<&str>,
    prev_mirror: Option<&str>,
    prev_creds: Option<&str>,
    prev_accounts: Option<&str>,
) {
    if let Some(kr) = prev_kr {
        write_keyring_token(kr);
    } else {
        delete_keyring_token();
    }
    if let Some(f) = prev_mirror {
        let _ = atomic_write(&paths.token_mirror, f);
    }
    if let Some(c) = prev_creds {
        let _ = atomic_write(&paths.oauth_creds_json, c);
    }
    if let Some(a) = prev_accounts {
        let _ = atomic_write(&paths.google_accounts_json, a);
    }
}

fn poll_agy_login(
    app: AppHandle,
    paths: AntigravityPaths,
    prev_kr: Option<String>,
    prev_mirror: Option<String>,
    prev_creds: Option<String>,
    prev_accounts: Option<String>,
    name_hint: String,
) {
    let deadline = Instant::now() + LOGIN_TIMEOUT;
    let mut captured: Result<Option<String>, String> = Err("Zaman aşımı (5 dk)".into());

    loop {
        let still_running = {
            let state = app.state::<AppState>();
            let running = state.harness_oauth.lock().unwrap().running;
            running
        };
        if !still_running {
            restore_live_slot(
                &paths,
                prev_kr.as_deref(),
                prev_mirror.as_deref(),
                prev_creds.as_deref(),
                prev_accounts.as_deref(),
            );
            captured = Err("Giriş iptal edildi".into());
            break;
        }
        if Instant::now() > deadline {
            restore_live_slot(
                &paths,
                prev_kr.as_deref(),
                prev_mirror.as_deref(),
                prev_creds.as_deref(),
                prev_accounts.as_deref(),
            );
            break;
        }

        let candidate = read_keyring_token().or_else(|| read_file_token_at(&paths));
        if let Some(raw) = candidate {
            let email = read_active_email_at(&paths);
            let state = app.state::<AppState>();
            let vault_key = state.vault_key();
            if let Ok(rec) = capture_from_raw(
                &raw,
                &vault_key,
                (!name_hint.trim().is_empty()).then_some(name_hint.as_str()),
                email.as_deref(),
            ) {
                let id = rec.id.clone();
                let unified = unified_from_agy_session(rec);
                let mut vault = state.vault.lock().unwrap();
                vault.accounts.push(unified);
                if let Err(e) = vault.save(&state.paths.unified_vault_file()) {
                    captured = Err(e);
                } else {
                    let mut s = state.settings.lock().unwrap();
                    s.antigravity_active_session_id = Some(id.clone());
                    let _ = s.save(&state.paths);
                    captured = Ok(Some(id));
                }
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    {
        let state = app.state::<AppState>();
        let mut flow = state.harness_oauth.lock().unwrap();
        flow.running = false;
        flow.child_pid = None;
    }

    let done = match captured {
        Ok(session_id) => HarnessOAuthDone {
            ok: true,
            harness: "antigravity".into(),
            session_id,
            error: None,
        },
        Err(e) => HarnessOAuthDone {
            ok: false,
            harness: "antigravity".into(),
            session_id: None,
            error: Some(e),
        },
    };
    let _ = app.emit("harness-oauth://done", &done);
}

pub fn launch_agy_folder(launches_dir: &Path, folder: &Path) -> Result<(), String> {
    launch_in_terminal(launches_dir, "Antigravity CLI", folder, &[], "agy", true).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "ha-agy-test-{tag}-{}-{}",
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
    fn oauth_creds_and_google_accounts_roundtrip() {
        let home = temp_home("gemini-creds");
        let paths = AntigravityPaths::under(&home);
        std::fs::create_dir_all(paths.oauth_creds_json.parent().unwrap()).unwrap();
        std::fs::write(
            &paths.oauth_creds_json,
            r#"{
              "access_token": "ya29.a0AfB_example_token",
              "refresh_token": "1//04_example_refresh",
              "token_type": "Bearer",
              "expiry_date": 1999999999000,
              "scope": "https://www.googleapis.com/auth/cloud-platform"
            }"#,
        )
        .unwrap();
        std::fs::write(
            &paths.google_accounts_json,
            r#"{"active": "user@example.com", "old": []}"#,
        )
        .unwrap();

        let captured = capture_unified_current_at(&paths, &key(), None, false).unwrap();
        assert_eq!(captured.name, "Antigravity (user@example.com)");
        assert_eq!(captured.user_hint.as_deref(), Some("user@example.com"));

        let mut vault = UnifiedVault::default();
        vault.accounts.push(captured.clone());

        let backup = disable_unified_at(&paths, &vault, &key(), false).unwrap();
        assert!(backup.is_none());
        assert!(!paths.oauth_creds_json.exists());

        activate_unified_at(&paths, &captured, &key(), false).unwrap();
        assert!(paths.oauth_creds_json.is_file());
        assert_eq!(read_active_email_at(&paths).as_deref(), Some("user@example.com"));

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn reject_invalid_token_json() {
        assert!(parse_token_json(r#"{"token_type":"Bearer"}"#).is_err());
        assert!(parse_token_json("not-json").is_err());
    }
}
