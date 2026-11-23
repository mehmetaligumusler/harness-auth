//! Antigravity CLI (`agy`) multi-account session management.
//!
//! Manages:
//! - Windows Credential Manager entry: `service = "gemini"`, `account = "antigravity"`
//! - File mirror: `%USERPROFILE%\.gemini\antigravity-cli\antigravity-oauth-token`
//! - Settings path: `%USERPROFILE%\.gemini\antigravity-cli\settings.json`

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::harness::{
    atomic_write, detect_cli, launch_in_terminal, mask_secret, user_home, CliInfo,
    HarnessOAuthDone, HarnessSessionRecord, SessionVault,
};
use crate::state::AppState;

const KEYRING_SERVICE: &str = "gemini";
const KEYRING_ACCOUNT: &str = "antigravity";
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
pub struct AntigravityPaths {
    pub token_mirror: PathBuf,
    pub settings_json: PathBuf,
}

impl AntigravityPaths {
    pub fn live() -> Result<Self, String> {
        let home = user_home()?;
        Ok(Self::under(&home))
    }

    pub fn under(home: &Path) -> Self {
        let dir = home.join(r".gemini\antigravity-cli");
        Self {
            token_mirror: dir.join("antigravity-oauth-token"),
            settings_json: dir.join("settings.json"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AntigravityLiveState {
    pub cli: CliInfo,
    pub has_keyring_token: bool,
    pub has_file_token: bool,
    pub token_hint: Option<String>,
    pub expiry_date: Option<i64>,
    pub is_expired: bool,
    pub scope: Option<String>,
}

pub fn detect_agy_cli() -> CliInfo {
    let extra = user_home()
        .ok()
        .map(|h| {
            vec![
                h.join(r"AppData\Roaming\npm\agy.cmd"),
                h.join(r"AppData\Local\Programs\Antigravity\agy.exe"),
                h.join(r".local\bin\agy.exe"),
            ]
        })
        .unwrap_or_default();
    detect_cli("agy", &extra)
}

fn read_keyring_token() -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT).ok()?;
    entry.get_password().ok().filter(|s| !s.trim().is_empty())
}

fn write_keyring_token(raw: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .map_err(|e| format!("Keyring girişi açılamadı: {e}"))?;
    entry
        .set_password(raw)
        .map_err(|e| format!("Keyring yazılamadı: {e}"))
}

fn delete_keyring_token() {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT) {
        let _ = entry.delete_credential();
    }
}

pub fn parse_token_json(raw: &str) -> Result<Value, String> {
    let v: Value =
        serde_json::from_str(raw.trim()).map_err(|e| format!("Geçersiz Google OAuth token JSON: {e}"))?;
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

pub fn inspect_live() -> AntigravityLiveState {
    let cli = detect_agy_cli();
    let kr_raw = read_keyring_token();
    let has_keyring_token = kr_raw.is_some();

    let paths = AntigravityPaths::live().ok();
    let file_raw = paths
        .as_ref()
        .and_then(|p| std::fs::read_to_string(&p.token_mirror).ok())
        .filter(|s| !s.trim().is_empty());
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
) -> Result<HarnessSessionRecord, String> {
    let parsed = parse_token_json(raw)?;
    let (token_hint, expiry_date, _is_expired, scope) = inspect_token_value(&parsed);

    let display_name = name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| match &token_hint {
            Some(h) => format!("Antigravity ({h})"),
            None => "Antigravity OAuth".to_string(),
        });

    let meta = json!({
        "scope": scope,
        "tokenType": parsed.get("token_type").cloned().unwrap_or(json!("Bearer")),
    });

    SessionVault::build_record(
        "antigravity",
        "antigravity_oauth",
        &display_name,
        Some("google".into()),
        token_hint,
        expiry_date,
        meta,
        &parsed,
        vault_key,
    )
}

pub fn capture_current(
    vault_key: &[u8; 32],
    name: Option<&str>,
) -> Result<HarnessSessionRecord, String> {
    let paths = AntigravityPaths::live()?;
    let raw = read_keyring_token()
        .or_else(|| {
            std::fs::read_to_string(&paths.token_mirror)
                .ok()
                .filter(|s| !s.trim().is_empty())
        })
        .ok_or_else(|| "Aktif Antigravity CLI oturumu bulunamadı (keyring ve dosya boş)".to_string())?;
    capture_from_raw(&raw, vault_key, name)
}

pub fn write_token_mirror_at(paths: &AntigravityPaths, token_val: &Value) -> Result<(), String> {
    let raw = serde_json::to_string(token_val).map_err(|e| e.to_string())?;
    atomic_write(&paths.token_mirror, &raw)
}

pub fn clear_token_mirror_at(paths: &AntigravityPaths) {
    if paths.token_mirror.is_file() {
        let _ = std::fs::remove_file(&paths.token_mirror);
    }
}

pub fn activate_session(
    record: &HarnessSessionRecord,
    vault_key: &[u8; 32],
) -> Result<(), String> {
    let paths = AntigravityPaths::live()?;
    let token_val = record.decrypt_payload(vault_key)?;
    let compact = serde_json::to_string(&token_val).map_err(|e| e.to_string())?;
    write_keyring_token(&compact)?;
    write_token_mirror_at(&paths, &token_val)?;
    Ok(())
}

pub fn disable(
    vault: &SessionVault,
    vault_key: &[u8; 32],
) -> Result<Option<HarnessSessionRecord>, String> {
    let paths = AntigravityPaths::live()?;
    let mut backup: Option<HarnessSessionRecord> = None;

    if let Ok(rec) = capture_current(vault_key, Some("Antigravity (Yedek)")) {
        if let Ok(payload) = rec.decrypt_payload(vault_key) {
            if !vault.has_matching_payload("antigravity", vault_key, &payload) {
                backup = Some(rec);
            }
        }
    }

    delete_keyring_token();
    clear_token_mirror_at(&paths);
    Ok(backup)
}

pub fn start_oauth_login(app: &AppHandle, name_hint: Option<String>) -> Result<(), String> {
    let cli = detect_agy_cli();
    if !cli.found {
        return Err("Antigravity CLI (`agy`) sistemde bulunamadı".into());
    }

    let paths = AntigravityPaths::live()?;
    let prev_keyring = read_keyring_token();
    let prev_mirror = std::fs::read_to_string(&paths.token_mirror).ok();

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

    // Clear live slot temporarily so `agy` triggers a fresh Google OAuth flow.
    delete_keyring_token();
    clear_token_mirror_at(&paths);

    let home = user_home().unwrap_or_else(|_| launches_dir.clone());
    let pid = match launch_in_terminal(&launches_dir, "Antigravity CLI Giriş", &home, &[], "agy", true) {
        Ok(pid) => pid,
        Err(e) => {
            restore_live_slot(&paths, prev_keyring.as_deref(), prev_mirror.as_deref());
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
        poll_agy_login(app, paths, prev_keyring, prev_mirror, name_hint)
    });
    Ok(())
}

fn restore_live_slot(paths: &AntigravityPaths, prev_kr: Option<&str>, prev_file: Option<&str>) {
    if let Some(kr) = prev_kr {
        let _ = write_keyring_token(kr);
    } else {
        delete_keyring_token();
    }
    if let Some(f) = prev_file {
        let _ = atomic_write(&paths.token_mirror, f);
    } else {
        clear_token_mirror_at(paths);
    }
}

fn poll_agy_login(
    app: AppHandle,
    paths: AntigravityPaths,
    prev_kr: Option<String>,
    prev_mirror: Option<String>,
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
            restore_live_slot(&paths, prev_kr.as_deref(), prev_mirror.as_deref());
            captured = Err("Giriş iptal edildi".into());
            break;
        }
        if Instant::now() > deadline {
            restore_live_slot(&paths, prev_kr.as_deref(), prev_mirror.as_deref());
            break;
        }

        let candidate = read_keyring_token().or_else(|| {
            std::fs::read_to_string(&paths.token_mirror)
                .ok()
                .filter(|s| !s.trim().is_empty())
        });
        if let Some(raw) = candidate {
            let state = app.state::<AppState>();
            let vault_key = state.vault_key();
            if let Ok(rec) = capture_from_raw(
                &raw,
                &vault_key,
                (!name_hint.trim().is_empty()).then_some(name_hint.as_str()),
            ) {
                let id = rec.id.clone();
                let mut sessions = state.sessions.lock().unwrap();
                sessions.sessions.push(rec);
                if let Err(e) = sessions.save(&state.paths.sessions_file()) {
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
    fn parse_capture_and_mirror_roundtrip() {
        let home = temp_home("mirror");
        let paths = AntigravityPaths::under(&home);
        let raw = r#"{
          "access_token": "ya29.a0AfB_example_token",
          "refresh_token": "1//04_example_refresh",
          "token_type": "Bearer",
          "expiry_date": 1999999999000,
          "scope": "https://www.googleapis.com/auth/cloud-platform"
        }"#;
        let rec = capture_from_raw(raw, &key(), Some("Work Google")).unwrap();
        assert_eq!(rec.name, "Work Google");
        assert_eq!(rec.harness, "antigravity");
        assert_eq!(rec.expires_at, Some(1999999999000));

        let payload = rec.decrypt_payload(&key()).unwrap();
        let (hint, exp, expired, scope) = inspect_token_value(&payload);
        assert!(hint.is_some());
        assert_eq!(exp, Some(1999999999000));
        assert!(!expired);
        assert!(scope.unwrap().contains("cloud-platform"));

        write_token_mirror_at(&paths, &payload).unwrap();
        assert!(paths.token_mirror.is_file());
        clear_token_mirror_at(&paths);
        assert!(!paths.token_mirror.exists());

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn reject_invalid_token_json() {
        assert!(parse_token_json(r#"{"token_type":"Bearer"}"#).is_err());
        assert!(parse_token_json("not-json").is_err());
    }
}
