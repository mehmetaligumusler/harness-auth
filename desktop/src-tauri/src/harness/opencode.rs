//! OpenCode (`opencode`) session and provider management.
//!
//! Manages:
//! - `%USERPROFILE%\.local\share\opencode\auth.json` (or `OPENCODE_AUTH_PATH`)
//! - `%USERPROFILE%\.config\opencode\opencode.json` (or `OPENCODE_CONFIG`)

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::harness::{
    atomic_write, detect_cli, launch_in_terminal, mask_secret, read_json_object, user_home,
    CliInfo, HarnessOAuthDone, HarnessSessionRecord, SessionVault,
};
use crate::state::AppState;

const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, Clone)]
pub struct OpenCodePaths {
    pub auth_json: PathBuf,
    pub config_json: PathBuf,
}

impl OpenCodePaths {
    pub fn live() -> Result<Self, String> {
        let home = user_home()?;
        let mut p = Self::under(&home);
        if let Ok(custom_auth) = std::env::var("OPENCODE_AUTH_PATH") {
            let trimmed = custom_auth.trim();
            if !trimmed.is_empty() {
                p.auth_json = PathBuf::from(trimmed);
            }
        }
        if let Ok(custom_cfg) = std::env::var("OPENCODE_CONFIG") {
            let trimmed = custom_cfg.trim();
            if !trimmed.is_empty() {
                p.config_json = PathBuf::from(trimmed);
            }
        }
        Ok(p)
    }

    pub fn under(home: &Path) -> Self {
        Self {
            auth_json: home.join(r".local\share\opencode\auth.json"),
            config_json: home.join(r".config\opencode\opencode.json"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeLiveState {
    pub cli: CliInfo,
    pub auth_path: String,
    pub config_path: String,
    pub has_auth_file: bool,
    pub auth_providers: Vec<String>,
    pub config_providers: Vec<String>,
    pub has_zcode_provider: bool,
    /// "zcode_oauth" | "z_api" | "profile" | "disabled"
    pub detected_mode: String,
    pub live_base_url: Option<String>,
    pub live_model: Option<String>,
}

pub fn detect_opencode_cli() -> CliInfo {
    let extra = user_home()
        .ok()
        .map(|h| {
            vec![
                h.join(r"AppData\Roaming\npm\opencode.cmd"),
                h.join(r".opencode\bin\opencode.exe"),
                h.join(r".local\bin\opencode.exe"),
            ]
        })
        .unwrap_or_default();
    detect_cli("opencode", &extra)
}

pub fn inspect_live() -> OpenCodeLiveState {
    let cli = detect_opencode_cli();
    let Ok(paths) = OpenCodePaths::live() else {
        return OpenCodeLiveState {
            cli,
            detected_mode: "disabled".into(),
            ..Default::default()
        };
    };
    let mut state = inspect_live_at(&paths);
    state.cli = cli;
    state
}

pub fn inspect_live_at(paths: &OpenCodePaths) -> OpenCodeLiveState {
    let auth_obj = read_json_object(&paths.auth_json);
    let auth_providers: Vec<String> = auth_obj.keys().cloned().collect();
    let cfg_obj = read_json_object(&paths.config_json);
    let providers_obj = cfg_obj
        .get("provider")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let config_providers: Vec<String> = providers_obj.keys().cloned().collect();
    let has_zcode_provider = providers_obj.contains_key("zcode");

    let managed_z_api_key = ["zai", "bigmodel"].iter().copied().find(|k| {
        providers_obj
            .get(*k)
            .and_then(|v| v.get("name"))
            .and_then(|n| n.as_str())
            .map(|n| n.contains("(HarnessAuth)"))
            .unwrap_or(false)
    });

    let detected_mode = if has_zcode_provider {
        "zcode_oauth"
    } else if managed_z_api_key.is_some() {
        "z_api"
    } else if !auth_providers.is_empty() {
        "profile"
    } else {
        "disabled"
    };

    let active_entry = if has_zcode_provider {
        providers_obj.get("zcode")
    } else if let Some(k) = managed_z_api_key {
        providers_obj.get(k)
    } else {
        auth_providers
            .first()
            .and_then(|k| providers_obj.get(k))
    };

    let live_base_url = active_entry
        .and_then(|v| v.pointer("/options/baseURL"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let live_model = active_entry
        .and_then(|v| v.get("models"))
        .and_then(|v| v.as_object())
        .and_then(|m| m.keys().next().cloned());

    OpenCodeLiveState {
        cli: CliInfo::default(),
        auth_path: paths.auth_json.display().to_string(),
        config_path: paths.config_json.display().to_string(),
        has_auth_file: !auth_providers.is_empty(),
        auth_providers,
        config_providers,
        has_zcode_provider,
        detected_mode: detected_mode.into(),
        live_base_url,
        live_model,
    }
}

pub fn capture_current_at(
    paths: &OpenCodePaths,
    vault_key: &[u8; 32],
    name: Option<&str>,
) -> Result<HarnessSessionRecord, String> {
    let auth_obj = read_json_object(&paths.auth_json);
    if auth_obj.is_empty() {
        return Err("Aktif OpenCode auth.json oturumu bulunamadı".into());
    }
    let providers: Vec<String> = auth_obj.keys().cloned().collect();
    let cfg_obj = read_json_object(&paths.config_json);
    let provider_config = cfg_obj.get("provider").cloned().unwrap_or(Value::Null);

    let provider_hint = Some(providers.join(", "));
    let account_hint = providers
        .first()
        .and_then(|p| auth_obj.get(p))
        .and_then(|entry| {
            entry
                .get("email")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| {
                    entry
                        .get("key")
                        .and_then(|v| v.as_str())
                        .map(mask_secret)
                })
        });

    let display_name = name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| format!("OpenCode ({})", providers.join(", ")));

    let payload = json!({
        "auth": Value::Object(auth_obj),
        "providerConfig": provider_config,
    });
    let meta = json!({
        "providers": providers,
    });

    SessionVault::build_record(
        "opencode",
        "opencode_snapshot",
        &display_name,
        provider_hint,
        account_hint,
        None,
        meta,
        &payload,
        vault_key,
    )
}

pub fn create_api_session(
    vault_key: &[u8; 32],
    name: &str,
    provider_id: &str,
    api_key: &str,
    base_url: Option<&str>,
    model: Option<&str>,
) -> Result<HarnessSessionRecord, String> {
    let pid = provider_id.trim();
    if pid.is_empty() || !pid.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_')) {
        return Err("Geçersiz sağlayıcı kimliği (yalnızca harf, rakam, - ve _)".into());
    }
    let key = api_key.trim();
    if key.len() < 4 {
        return Err("API Key çok kısa".into());
    }
    let clean_base = base_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_end_matches('/').to_string());
    if let Some(url) = &clean_base {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err("Base URL http:// veya https:// ile başlamalı".into());
        }
    }
    let clean_model = model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);

    let display_name = if name.trim().is_empty() {
        format!("OpenCode — {pid}")
    } else {
        name.trim().to_string()
    };

    let auth_obj = json!({
        pid: {
            "type": "api",
            "key": key,
        }
    });

    let mut provider_entry = Map::new();
    if let Some(url) = &clean_base {
        provider_entry.insert(
            "npm".into(),
            json!(if pid.contains("anthropic") {
                "@ai-sdk/anthropic"
            } else {
                "@ai-sdk/openai-compatible"
            }),
        );
        provider_entry.insert("name".into(), json!(display_name));
        provider_entry.insert("options".into(), json!({ "baseURL": url }));
    }
    if let Some(m) = &clean_model {
        provider_entry.insert("models".into(), json!({ (m): { "name": m } }));
    }

    let payload = json!({
        "auth": auth_obj,
        "providerId": pid,
        "providerEntry": if provider_entry.is_empty() { Value::Null } else { Value::Object(provider_entry) },
    });
    let meta = json!({
        "providers": [pid],
        "baseUrl": clean_base,
        "model": clean_model,
    });

    SessionVault::build_record(
        "opencode",
        "opencode_api",
        &display_name,
        Some(pid.to_string()),
        Some(mask_secret(key)),
        None,
        meta,
        &payload,
        vault_key,
    )
}

pub fn create_zcode_bridge_session(
    vault_key: &[u8; 32],
    name: &str,
    zcode_account_id: &str,
    zcode_account_name: &str,
    model: Option<&str>,
) -> Result<HarnessSessionRecord, String> {
    let clean_model = model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("glm-5.3")
        .to_string();
    let display_name = if name.trim().is_empty() {
        format!("OpenCode — ZCode ({zcode_account_name})")
    } else {
        name.trim().to_string()
    };
    let payload = json!({
        "zcodeAccountId": zcode_account_id,
        "model": clean_model,
    });
    let meta = json!({
        "zcodeAccountId": zcode_account_id,
        "zcodeAccountName": zcode_account_name,
        "model": clean_model,
        "providers": ["zcode"],
    });
    SessionVault::build_record(
        "opencode",
        "opencode_zcode_proxy",
        &display_name,
        Some("zcode".into()),
        Some(zcode_account_name.to_string()),
        None,
        meta,
        &payload,
        vault_key,
    )
}

fn update_opencode_config_providers(
    config_path: &Path,
    mutate: impl FnOnce(&mut Map<String, Value>),
) -> Result<(), String> {
    let mut root = read_json_object(config_path);
    if !root.contains_key("$schema") {
        root.insert("$schema".into(), json!("https://opencode.ai/config.json"));
    }
    let mut providers = root
        .get("provider")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    mutate(&mut providers);

    if providers.is_empty() {
        root.remove("provider");
    } else {
        root.insert("provider".into(), Value::Object(providers));
    }
    let text = serde_json::to_string_pretty(&Value::Object(root)).map_err(|e| e.to_string())?;
    atomic_write(config_path, &text)
}

pub fn activate_zcode_oauth_direct_at(
    paths: &OpenCodePaths,
    endpoint: &str,
    api_key: &str,
    model: Option<&str>,
) -> Result<(), String> {
    let key = if api_key.trim().is_empty() {
        "harnessauth-local"
    } else {
        api_key.trim()
    };
    let chosen_model = model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("glm-5.3");

    let auth_obj = json!({
        "zcode": {
            "type": "api",
            "key": key,
        }
    });
    let auth_text = serde_json::to_string_pretty(&auth_obj).map_err(|e| e.to_string())?;
    atomic_write(&paths.auth_json, &auth_text)?;

    let base_v1 = format!("{}/v1", endpoint.trim_end_matches('/'));
    let zcode_cfg = json!({
        "npm": "@ai-sdk/openai-compatible",
        "name": "ZCode Proxy (HarnessAuth)",
        "options": {
            "baseURL": base_v1,
            "apiKey": key,
        },
        "models": {
            (chosen_model): {
                "name": chosen_model,
            }
        }
    });
    update_opencode_config_providers(&paths.config_json, |providers| {
        remove_managed_z_providers(providers);
        providers.insert("zcode".into(), zcode_cfg);
    })
}

pub fn activate_z_api_direct_at(
    paths: &OpenCodePaths,
    provider: &str,
    api_key: &str,
    custom_base_url: Option<&str>,
    model: Option<&str>,
) -> Result<(), String> {
    let base_url = crate::harness::claude_harness::resolve_z_api_base(provider, custom_base_url)?;
    let chosen_model = model
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("glm-5.3");
    let label = if provider == "zai" {
        "Z.AI API (HarnessAuth)"
    } else {
        "BigModel API (HarnessAuth)"
    };

    let auth_obj = json!({
        provider: {
            "type": "api",
            "key": api_key.trim(),
        }
    });
    let auth_text = serde_json::to_string_pretty(&auth_obj).map_err(|e| e.to_string())?;
    atomic_write(&paths.auth_json, &auth_text)?;

    let cfg_entry = json!({
        "npm": "@ai-sdk/anthropic",
        "name": label,
        "options": {
            "baseURL": base_url,
            "apiKey": api_key.trim(),
        },
        "models": {
            (chosen_model): {
                "name": chosen_model,
            }
        }
    });
    update_opencode_config_providers(&paths.config_json, |providers| {
        remove_managed_z_providers(providers);
        providers.insert(provider.to_string(), cfg_entry);
    })
}

fn remove_managed_z_providers(providers: &mut Map<String, Value>) {
    providers.remove("zcode");
    for key in ["zai", "bigmodel"] {
        let is_managed = providers
            .get(key)
            .and_then(|v| v.get("name"))
            .and_then(|n| n.as_str())
            .map(|n| n.contains("(HarnessAuth)"))
            .unwrap_or(false);
        if is_managed {
            providers.remove(key);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_session_record(
    rec: &mut HarnessSessionRecord,
    vault_key: &[u8; 32],
    name: Option<&str>,
    provider_id: Option<&str>,
    api_key: Option<&str>,
    base_url: Option<&str>,
    model: Option<&str>,
    zcode_account_id: Option<&str>,
    zcode_account_name: Option<&str>,
) -> Result<(), String> {
    if let Some(n) = name.map(str::trim).filter(|s| !s.is_empty()) {
        rec.name = n.to_string();
    }
    match rec.kind.as_str() {
        "opencode_api" => {
            let mut payload = rec.decrypt_payload(vault_key)?;
            let old_pid = payload
                .get("providerId")
                .and_then(|v| v.as_str())
                .unwrap_or("anthropic")
                .to_string();
            let pid = match provider_id.map(str::trim).filter(|s| !s.is_empty()) {
                Some(p) => {
                    if !p
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                    {
                        return Err(
                            "Geçersiz sağlayıcı kimliği (yalnızca harf, rakam, - ve _)".into()
                        );
                    }
                    p.to_string()
                }
                None => old_pid.clone(),
            };
            let existing_key = payload
                .pointer(&format!("/auth/{old_pid}/key"))
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_default();
            let key = match api_key.map(str::trim).filter(|s| !s.is_empty()) {
                Some(k) => {
                    if k.len() < 4 {
                        return Err("API Key çok kısa".into());
                    }
                    k.to_string()
                }
                None => {
                    if existing_key.is_empty() {
                        return Err("API Key bulunamadı".into());
                    }
                    existing_key
                }
            };
            let clean_base = match base_url {
                Some(raw) => {
                    let b = raw.trim().trim_end_matches('/').to_string();
                    if b.is_empty() {
                        None
                    } else {
                        if !b.starts_with("http://") && !b.starts_with("https://") {
                            return Err("Base URL http:// veya https:// ile başlamalı".into());
                        }
                        Some(b)
                    }
                }
                None => rec
                    .meta
                    .get("baseUrl")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            };
            let clean_model = match model {
                Some(raw) => {
                    let m = raw.trim();
                    if m.is_empty() {
                        None
                    } else {
                        Some(m.to_string())
                    }
                }
                None => rec
                    .meta
                    .get("model")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            };

            let auth_obj = json!({
                (pid.as_str()): {
                    "type": "api",
                    "key": key,
                }
            });
            let mut provider_entry = Map::new();
            if let Some(url) = &clean_base {
                provider_entry.insert(
                    "npm".into(),
                    json!(if pid.contains("anthropic") {
                        "@ai-sdk/anthropic"
                    } else {
                        "@ai-sdk/openai-compatible"
                    }),
                );
                provider_entry.insert("name".into(), json!(rec.name));
                provider_entry.insert("options".into(), json!({ "baseURL": url }));
            }
            if let Some(m) = &clean_model {
                provider_entry.insert("models".into(), json!({ (m): { "name": m } }));
            }
            payload = json!({
                "auth": auth_obj,
                "providerId": pid,
                "providerEntry": if provider_entry.is_empty() { Value::Null } else { Value::Object(provider_entry) },
            });
            rec.encrypted = crate::crypto::seal(
                vault_key,
                &serde_json::to_string(&payload).map_err(|e| e.to_string())?,
            )?;
            rec.provider_hint = Some(pid.clone());
            rec.account_hint = Some(mask_secret(&key));
            rec.meta = json!({
                "providers": [pid],
                "baseUrl": clean_base,
                "model": clean_model,
            });
            Ok(())
        }
        "opencode_zcode_proxy" => {
            let mut payload = rec.decrypt_payload(vault_key)?;
            let acc_id = zcode_account_id
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .or_else(|| {
                    payload
                        .get("zcodeAccountId")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
                .ok_or("ZCode hesap kimliği eksik")?;
            let acc_name = zcode_account_name
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .or_else(|| rec.account_hint.clone())
                .unwrap_or_else(|| acc_id.clone());
            let clean_model = model
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .or_else(|| {
                    payload
                        .get("model")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
                .unwrap_or_else(|| "glm-5.3".into());
            payload = json!({
                "zcodeAccountId": acc_id,
                "model": clean_model,
            });
            rec.encrypted = crate::crypto::seal(
                vault_key,
                &serde_json::to_string(&payload).map_err(|e| e.to_string())?,
            )?;
            rec.account_hint = Some(acc_name.clone());
            rec.meta = json!({
                "zcodeAccountId": acc_id,
                "zcodeAccountName": acc_name,
                "model": clean_model,
                "providers": ["zcode"],
            });
            Ok(())
        }
        _ => Ok(()),
    }
}

pub fn activate_session_at(
    paths: &OpenCodePaths,
    record: &HarnessSessionRecord,
    vault_key: &[u8; 32],
    zcode_proxy_info: Option<(&str, &str)>,
) -> Result<(), String> {
    let payload = record.decrypt_payload(vault_key)?;
    match record.kind.as_str() {
        "opencode_snapshot" => {
            let auth = payload
                .get("auth")
                .filter(|v| v.is_object())
                .ok_or("Snapshot oturumunda auth verisi eksik")?;
            let auth_text = serde_json::to_string_pretty(auth).map_err(|e| e.to_string())?;
            atomic_write(&paths.auth_json, &auth_text)?;

            update_opencode_config_providers(&paths.config_json, |providers| {
                remove_managed_z_providers(providers);
                if let Some(pcfg) = payload.get("providerConfig").and_then(|v| v.as_object()) {
                    for (k, v) in pcfg {
                        providers.insert(k.clone(), v.clone());
                    }
                }
            })?;
            Ok(())
        }
        "opencode_api" => {
            let auth = payload
                .get("auth")
                .filter(|v| v.is_object())
                .ok_or("API oturumunda auth verisi eksik")?;
            let auth_text = serde_json::to_string_pretty(auth).map_err(|e| e.to_string())?;
            atomic_write(&paths.auth_json, &auth_text)?;

            let pid = payload
                .get("providerId")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let entry = payload.get("providerEntry").cloned().unwrap_or(Value::Null);
            update_opencode_config_providers(&paths.config_json, |providers| {
                remove_managed_z_providers(providers);
                if !pid.is_empty() && entry.is_object() {
                    providers.insert(pid, entry);
                }
            })?;
            Ok(())
        }
        "opencode_zcode_proxy" => {
            let (endpoint, api_key) =
                zcode_proxy_info.ok_or("ZCode proxy endpoint hazır değil")?;
            let model = payload.get("model").and_then(|v| v.as_str());
            activate_zcode_oauth_direct_at(paths, endpoint, api_key, model)
        }
        other => Err(format!("Bilinmeyen OpenCode oturum tipi: {other}")),
    }
}

pub fn disable_at(
    paths: &OpenCodePaths,
    vault: &SessionVault,
    vault_key: &[u8; 32],
) -> Result<Option<HarnessSessionRecord>, String> {
    let mut backup: Option<HarnessSessionRecord> = None;
    if paths.auth_json.is_file() {
        if let Ok(rec) = capture_current_at(paths, vault_key, Some("OpenCode (Yedek)")) {
            if let Ok(payload) = rec.decrypt_payload(vault_key) {
                if !vault.has_matching_payload("opencode", vault_key, &payload) {
                    backup = Some(rec);
                }
            }
        }
        let _ = std::fs::remove_file(&paths.auth_json);
    }

    if paths.config_json.is_file() {
        update_opencode_config_providers(&paths.config_json, remove_managed_z_providers)?;
    }

    Ok(backup)
}

pub fn start_oauth_login(app: &AppHandle, name_hint: Option<String>) -> Result<(), String> {
    let cli = detect_opencode_cli();
    if !cli.found {
        return Err("OpenCode CLI (`opencode`) sistemde bulunamadı".into());
    }

    let (launches_dir, temp_dir, temp_auth) = {
        let state = app.state::<AppState>();
        let mut flow = state.harness_oauth.lock().unwrap();
        if flow.running {
            return Err("Zaten devam eden bir oturum ekleme akışı var".into());
        }
        let temp_dir = state
            .paths
            .tmp_dir()
            .join(format!("opencode-login-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Geçici oturum klasörü oluşturulamadı: {e}"))?;
        let temp_auth = temp_dir.join("auth.json");
        flow.running = true;
        flow.harness = "opencode".into();
        flow.label = name_hint.clone().unwrap_or_default();
        flow.started_at = Some(chrono::Utc::now().timestamp_millis());
        flow.temp_dir = Some(temp_dir.clone());
        (state.paths.launches_dir(), temp_dir, temp_auth)
    };

    let home = user_home().unwrap_or_else(|_| launches_dir.clone());
    let auth_str = temp_auth.display().to_string();
    let pid = launch_in_terminal(
        &launches_dir,
        "OpenCode OAuth Giriş",
        &home,
        &[("OPENCODE_AUTH_PATH", &auth_str)],
        "opencode auth login",
        true,
    )?;

    {
        let state = app.state::<AppState>();
        state.harness_oauth.lock().unwrap().child_pid = Some(pid);
    }

    let app = app.clone();
    let name_hint = name_hint.unwrap_or_default();
    std::thread::spawn(move || poll_opencode_login(app, temp_dir, temp_auth, name_hint));
    Ok(())
}

fn poll_opencode_login(app: AppHandle, temp_dir: PathBuf, temp_auth: PathBuf, name_hint: String) {
    let isolated = OpenCodePaths {
        auth_json: temp_auth.clone(),
        config_json: temp_dir.join("opencode.json"),
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
        if temp_auth.is_file() {
            let obj = read_json_object(&temp_auth);
            if !obj.is_empty() {
                let state = app.state::<AppState>();
                let vault_key = state.vault_key();
                if let Ok(rec) = capture_current_at(
                    &isolated,
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
            harness: "opencode".into(),
            session_id,
            error: None,
        },
        Err(e) => HarnessOAuthDone {
            ok: false,
            harness: "opencode".into(),
            session_id: None,
            error: Some(e),
        },
    };
    let _ = app.emit("harness-oauth://done", &done);
}

pub fn launch_opencode_folder(launches_dir: &Path, folder: &Path) -> Result<(), String> {
    launch_in_terminal(launches_dir, "OpenCode", folder, &[], "opencode", true).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "ha-opencode-test-{tag}-{}-{}",
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
    fn api_and_zcode_sessions_preserve_user_opencode_settings() {
        let home = temp_home("cfg");
        let paths = OpenCodePaths::under(&home);
        std::fs::create_dir_all(paths.config_json.parent().unwrap()).unwrap();
        std::fs::write(
            &paths.config_json,
            r#"{
              "theme": "tokyo-night",
              "provider": {
                "existing-custom": { "options": { "baseURL": "https://example.com" } }
              }
            }"#,
        )
        .unwrap();

        let api_rec = create_api_session(
            &key(),
            "Anthropic API",
            "anthropic",
            "sk-ant-api03-123456",
            None,
            None,
        )
        .unwrap();
        activate_session_at(&paths, &api_rec, &key(), None).unwrap();

        let live = inspect_live_at(&paths);
        assert!(live.has_auth_file);
        assert_eq!(live.auth_providers, vec!["anthropic".to_string()]);
        assert!(!live.has_zcode_provider);

        let zcode_rec =
            create_zcode_bridge_session(&key(), "ZCode Bridge", "acc-1", "Main ZCode", Some("glm-5.3"))
                .unwrap();
        activate_session_at(&paths, &zcode_rec, &key(), Some(("http://127.0.0.1:8317", "px-key")))
            .unwrap();

        let cfg = read_json_object(&paths.config_json);
        assert_eq!(cfg["theme"], "tokyo-night");
        assert!(cfg["provider"]["existing-custom"].is_object());
        assert_eq!(
            cfg["provider"]["zcode"]["options"]["baseURL"],
            "http://127.0.0.1:8317/v1"
        );

        let empty_vault = SessionVault::default();
        let backup = disable_at(&paths, &empty_vault, &key()).unwrap();
        assert!(backup.is_some());
        assert!(!paths.auth_json.exists());
        let cfg_after = read_json_object(&paths.config_json);
        assert!(cfg_after["provider"].get("zcode").is_none());
        assert!(cfg_after["provider"]["existing-custom"].is_object());

        let _ = std::fs::remove_dir_all(&home);
    }

    #[test]
    fn opencode_direct_zcode_oauth_and_z_api_and_edit() {
        let home = temp_home("direct-modes");
        let paths = OpenCodePaths::under(&home);

        // 1) Direct ZCode OAuth proxy
        activate_zcode_oauth_direct_at(&paths, "http://127.0.0.1:8317", "px-99", Some("glm-5.3"))
            .unwrap();
        let live1 = inspect_live_at(&paths);
        assert_eq!(live1.detected_mode, "zcode_oauth");
        assert_eq!(live1.live_base_url.as_deref(), Some("http://127.0.0.1:8317/v1"));
        assert_eq!(live1.live_model.as_deref(), Some("glm-5.3"));

        // 2) Direct Z API (no proxy)
        activate_z_api_direct_at(&paths, "zai", "zk-direct-1234", None, Some("glm-4.7")).unwrap();
        let live2 = inspect_live_at(&paths);
        assert_eq!(live2.detected_mode, "z_api");
        assert!(!live2.has_zcode_provider);
        assert_eq!(live2.live_base_url.as_deref(), Some("https://api.z.ai/api/anthropic"));
        assert_eq!(live2.live_model.as_deref(), Some("glm-4.7"));

        // 3) Edit API session preserves key when omitted
        let mut api_rec =
            create_api_session(&key(), "Custom", "myprov", "sk-secret-9999", None, None).unwrap();
        update_session_record(
            &mut api_rec,
            &key(),
            Some("Updated Prov"),
            Some("openrouter"),
            None,
            Some("https://openrouter.ai/api/v1"),
            Some("anthropic/claude-sonnet-4"),
            None,
            None,
        )
        .unwrap();
        assert_eq!(api_rec.name, "Updated Prov");
        let p = api_rec.decrypt_payload(&key()).unwrap();
        assert_eq!(p["auth"]["openrouter"]["key"], "sk-secret-9999");

        // 4) Disable cleans managed zai provider
        let empty_vault = SessionVault::default();
        let _ = disable_at(&paths, &empty_vault, &key()).unwrap();
        let live3 = inspect_live_at(&paths);
        assert_eq!(live3.detected_mode, "disabled");

        let _ = std::fs::remove_dir_all(&home);
    }
}
