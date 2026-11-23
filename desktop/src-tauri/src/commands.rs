//! Tauri command surface — the only bridge between the React UI and the
//! Rust backend. Secrets (credentials, master secret) never leave this layer;
//! the frontend receives masked summaries only.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::claude;
use crate::credstore;
use crate::harness::{
    self,
    antigravity::{self, AntigravityLiveState},
    claude_harness::{self, ClaudeLiveState, ClaudePaths},
    opencode::{self, OpenCodeLiveState, OpenCodePaths},
    HarnessSessionSummary,
};
use crate::http;
use crate::oauth;
use crate::panel::PanelClient;
use crate::paths;
use crate::proxy::{self, ProxyStatus};
use crate::settings::Settings;
use crate::state::AppState;
use crate::vault::{self, AccountSummary, VaultData};
use crate::yamlpatch::{self, YamlUpdate};

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatusDto {
    #[serde(flatten)]
    pub status: ProxyStatus,
    pub active_account_id: Option<String>,
    pub active_account_name: Option<String>,
    pub account_count: usize,
    pub oauth_running: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateDto {
    pub settings: Settings,
    pub accounts: Vec<AccountSummary>,
    pub sessions: Vec<HarnessSessionSummary>,
    pub proxy: ProxyStatusDto,
    pub claude: claude::ClaudeInfo,
    pub claude_live: ClaudeLiveState,
    pub opencode_live: OpenCodeLiveState,
    pub antigravity_live: AntigravityLiveState,
    pub harness_oauth_running: bool,
    pub harness_oauth_target: Option<String>,
    pub versions: Versions,
    pub paths: PathsDto,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Versions {
    pub app: String,
    pub proxy: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathsDto {
    pub config: String,
    pub errors_log: String,
    pub data_dir: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub ok: bool,
    pub message: String,
    pub latency_ms: Option<u64>,
}

// ---------------------------------------------------------------------------
// Status helpers
// ---------------------------------------------------------------------------

fn build_status(app: &AppHandle) -> ProxyStatusDto {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let managed = state.proxy.lock().unwrap().clone();
    let oauth_running = state.oauth.lock().unwrap().running;
    let (active_account_id, active_account_name, account_count) = {
        let vault = state.vault.lock().unwrap();
        (
            vault.active_id.clone(),
            vault.active().map(|a| a.name.clone()),
            vault.accounts.len(),
        )
    };
    let status = proxy::status(&state.paths, &settings, &managed);
    ProxyStatusDto {
        status,
        active_account_id,
        active_account_name,
        account_count,
        oauth_running,
    }
}

pub fn emit_proxy_state(app: &AppHandle) {
    let status = build_status(app);
    crate::tray::update_tooltip(
        app,
        status.status.process_alive && status.status.server_running,
    );
    let _ = app.emit("proxy://state", status);
}

fn panel(app: &AppHandle) -> Result<PanelClient, String> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let port = state.proxy.lock().unwrap().panel_port;
    if port == 0 {
        return Err("proxy_not_running".into());
    }
    Ok(PanelClient::new(port, &settings.panel_token))
}

/// Validate a model id exactly like the Claude launcher does.
fn check_model(model: &str) -> Result<(), String> {
    if claude::valid_model_name(model) {
        Ok(())
    } else {
        Err(format!("Geçersiz model adı: {model}"))
    }
}

// ---------------------------------------------------------------------------
// App state / settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_app_state(app: AppHandle) -> Result<AppStateDto, String> {
    let proxy_exe = paths::resolve_proxy_exe();
    let (claude_info, claude_live, opencode_live, antigravity_live, proxy_version) =
        tauri::async_runtime::spawn_blocking(move || {
            let claude_live = claude_harness::inspect_live();
            let claude_info = claude::ClaudeInfo {
                found: claude_live.cli.found,
                version: claude_live.cli.version.clone(),
                path: claude_live.cli.path.clone(),
            };
            (
                claude_info,
                claude_live,
                opencode::inspect_live(),
                antigravity::inspect_live(),
                claude::proxy_exe_version(&proxy_exe),
            )
        })
        .await
        .map_err(|e| e.to_string())?;

    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let accounts = state.vault.lock().unwrap().summaries();
    let sessions = state.sessions.lock().unwrap().summaries(
        settings.claude_active_session_id.as_deref(),
        settings.opencode_active_session_id.as_deref(),
        settings.antigravity_active_session_id.as_deref(),
    );
    let (harness_oauth_running, harness_oauth_target) = {
        let h = state.harness_oauth.lock().unwrap();
        (h.running, h.running.then(|| h.harness.clone()))
    };

    Ok(AppStateDto {
        settings,
        accounts,
        sessions,
        proxy: build_status(&app),
        claude: claude_info,
        claude_live,
        opencode_live,
        antigravity_live,
        harness_oauth_running,
        harness_oauth_target,
        versions: Versions {
            app: app.package_info().version.to_string(),
            proxy: proxy_version,
        },
        paths: PathsDto {
            config: state.paths.proxy_config().display().to_string(),
            errors_log: state.paths.proxy_errors_log().display().to_string(),
            data_dir: state.paths.data_dir.display().to_string(),
        },
    })
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, new_settings: Settings) -> Result<Settings, String> {
    // Validate enums.
    if !matches!(new_settings.language.as_str(), "tr" | "en") {
        return Err("Desteklenmeyen dil".into());
    }
    if !matches!(new_settings.theme.as_str(), "dark" | "light") {
        return Err("Desteklenmeyen tema".into());
    }
    if !matches!(new_settings.default_provider.as_str(), "zai" | "bigmodel") {
        return Err("Desteklenmeyen sağlayıcı".into());
    }
    if !matches!(new_settings.default_plan.as_str(), "coding-plan" | "start-plan") {
        return Err("Desteklenmeyen plan".into());
    }
    if !new_settings.default_model.is_empty() {
        check_model(&new_settings.default_model)?;
    }

    let state = app.state::<AppState>();
    let saved_snapshot = {
        let mut s = state.settings.lock().unwrap();
        // Panel identity and live harness active pointers are backend-managed.
        let (panel_port, panel_token) = (s.panel_port, s.panel_token.clone());
        let (
            claude_active_method,
            claude_active_session_id,
            claude_active_model,
            opencode_active_session_id,
            opencode_active_method,
            opencode_active_model,
            antigravity_active_session_id,
        ) = (
            s.claude_active_method.clone(),
            s.claude_active_session_id.clone(),
            s.claude_active_model.clone(),
            s.opencode_active_session_id.clone(),
            s.opencode_active_method.clone(),
            s.opencode_active_model.clone(),
            s.antigravity_active_session_id.clone(),
        );
        *s = new_settings;
        s.panel_port = panel_port;
        s.panel_token = panel_token;
        s.claude_active_method = claude_active_method;
        s.claude_active_session_id = claude_active_session_id;
        s.claude_active_model = claude_active_model;
        s.opencode_active_session_id = opencode_active_session_id;
        s.opencode_active_method = opencode_active_method;
        s.opencode_active_model = opencode_active_model;
        s.antigravity_active_session_id = antigravity_active_session_id;
        s.sync_autostart()?;
        s.save(&state.paths)?;
        s.clone()
    };
    emit_proxy_state(&app);
    Ok(saved_snapshot)
}

#[tauri::command]
pub fn reset_panel_identity(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut s = state.settings.lock().unwrap();
    s.panel_token = crate::crypto::random_token();
    s.panel_port = crate::proxy::free_port();
    s.save(&state.paths)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn add_api_key_account(
    app: AppHandle,
    name: String,
    provider: String,
    api_key: String,
    default_plan: Option<String>,
    default_model: Option<String>,
    base_url: Option<String>,
) -> Result<Vec<AccountSummary>, String> {
    if provider != "zai" && provider != "bigmodel" {
        return Err("Desteklenmeyen sağlayıcı".into());
    }
    let key = api_key.trim().to_string();
    if key.len() < 8 {
        return Err("API Key çok kısa".into());
    }
    // Keys are opaque token strings; restrict to a conservative charset.
    if !key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '='))
    {
        return Err("API Key geçersiz karakterler içeriyor".into());
    }
    if let Some(m) = &default_model {
        check_model(m)?;
    }
    if let Some(p) = &default_plan {
        if p != "coding-plan" {
            return Err("API Key bağlantıları yalnızca Coding Plan (Anthropic uyumlu) endpoint'ini kullanabilir".into());
        }
    }
    let clean_base = base_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            if !s.starts_with("http://") && !s.starts_with("https://") {
                return Err("Özel Base URL http:// veya https:// ile başlamalı".to_string());
            }
            Ok(s.trim_end_matches('/').to_string())
        })
        .transpose()?;

    let state = app.state::<AppState>();
    let mut vault = state.vault.lock().unwrap();
    let cred = json!({ "apiKey": key, "provider": provider });
    let display_name = if name.trim().is_empty() {
        format!(
            "{} API Key {}",
            if provider == "zai" { "Z.AI" } else { "BigModel" },
            vault.accounts.len() + 1
        )
    } else {
        name.trim().to_string()
    };
    let record = vault::record_from_credential(
        &display_name,
        "api-key",
        &cred,
        &state.vault_key(),
    )?;
    let mut record = record;
    record.default_plan = Some("coding-plan".into());
    record.default_model = default_model;
    record.base_url = clean_base;
    let was_empty = vault.accounts.is_empty();
    let id = record.id.clone();
    vault.accounts.push(record);
    if was_empty {
        vault.active_id = Some(id);
    }
    vault.save(&state.paths.vault_file())?;
    let summaries = vault.summaries();
    drop(vault);
    emit_proxy_state(&app);
    Ok(summaries)
}

#[tauri::command]
pub fn update_account(
    app: AppHandle,
    id: String,
    name: Option<String>,
    default_plan: Option<String>,
    default_model: Option<String>,
    base_url: Option<String>,
) -> Result<Vec<AccountSummary>, String> {
    if let Some(m) = &default_model {
        if !m.is_empty() {
            check_model(m)?;
        }
    }
    let state = app.state::<AppState>();
    let mut vault = state.vault.lock().unwrap();
    let rec = vault
        .accounts
        .iter_mut()
        .find(|a| a.id == id)
        .ok_or_else(|| "Hesap bulunamadı".to_string())?;
    if let Some(n) = name {
        if !n.trim().is_empty() {
            rec.name = n.trim().to_string();
        }
    }
    if let Some(p) = default_plan {
        if p == "start-plan" && !rec.has_jwt {
            return Err("Bu hesapta Start Plan JWT bulunmuyor".into());
        }
        rec.default_plan = Some(p);
    }
    if let Some(m) = default_model {
        rec.default_model = if m.is_empty() { None } else { Some(m) };
    }
    if let Some(u) = base_url {
        let trimmed = u.trim();
        if trimmed.is_empty() {
            rec.base_url = None;
        } else {
            if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
                return Err("Özel Base URL http:// veya https:// ile başlamalı".into());
            }
            rec.base_url = Some(trimmed.trim_end_matches('/').to_string());
        }
    }
    vault.save(&state.paths.vault_file())?;
    let summaries = vault.summaries();
    drop(vault);
    emit_proxy_state(&app);
    Ok(summaries)
}

#[tauri::command]
pub async fn set_account_enabled(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<Vec<AccountSummary>, String> {
    let was_active = {
        let state = app.state::<AppState>();
        let mut vault = state.vault.lock().unwrap();
        let is_active = vault.is_active(&id);
        let rec = vault
            .accounts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| "Hesap bulunamadı".to_string())?;
        rec.enabled = enabled;
        if !enabled && is_active {
            vault.active_id = None;
        }
        vault.save(&state.paths.vault_file())?;
        is_active
    };

    if !enabled && was_active {
        stop_proxy_process(&app).await?;
        let state = app.state::<AppState>();
        credstore::clear_credential(&state.paths.proxy_store_dir());
    }

    emit_proxy_state(&app);
    let state = app.state::<AppState>();
    let summaries = state.vault.lock().unwrap().summaries();
    Ok(summaries)
}

#[tauri::command]
pub async fn deactivate_account(app: AppHandle) -> Result<ProxyStatusDto, String> {
    stop_proxy_process(&app).await?;
    let state = app.state::<AppState>();
    credstore::clear_credential(&state.paths.proxy_store_dir());
    {
        let mut vault = state.vault.lock().unwrap();
        vault.active_id = None;
        vault.save(&state.paths.vault_file())?;
    }
    emit_proxy_state(&app);
    Ok(build_status(&app))
}

#[tauri::command]
pub async fn delete_account(app: AppHandle, id: String) -> Result<Value, String> {
    let was_active = {
        let state = app.state::<AppState>();
        let vault = state.vault.lock().unwrap();
        if !vault.accounts.iter().any(|a| a.id == id) {
            return Err("Hesap bulunamadı".into());
        }
        vault.is_active(&id)
    };

    if was_active {
        stop_proxy_process(&app).await?;
        let state = app.state::<AppState>();
        credstore::clear_credential(&state.paths.proxy_store_dir());
    }

    let state = app.state::<AppState>();
    {
        let mut vault = state.vault.lock().unwrap();
        vault.accounts.retain(|a| a.id != id);
        if vault.active_id.as_deref() == Some(id.as_str()) {
            vault.active_id = None;
        }
        vault.save(&state.paths.vault_file())?;
    }
    emit_proxy_state(&app);
    Ok(json!({ "ok": true }))
}

/// Write the active account's credential + config keys and (re)start the
/// proxy process so it runs under the new account.
#[tauri::command]
pub async fn activate_account(app: AppHandle, id: String) -> Result<ProxyStatusDto, String> {
    let state = app.state::<AppState>();
    // Decrypting here validates the credential before we commit the switch.
    let (_cred_json, provider, plan, model) = {
        let vault_key = state.vault_key();
        let mut vault = state.vault.lock().unwrap();
        let rec = vault
            .accounts
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| "Hesap bulunamadı".to_string())?
            .clone();
        if !rec.enabled {
            return Err("Bu hesap devre dışı. Önce hesabı etkinleştirin.".into());
        }
        let cred = rec.decrypt_credential(&vault_key)?;
        let json = serde_json::to_string(&cred).map_err(|e| e.to_string())?;
        let settings = state.settings.lock().unwrap().clone();
        let plan = rec
            .default_plan
            .clone()
            .filter(|p| p == "start-plan" && rec.has_jwt)
            .unwrap_or_else(|| {
                if settings.default_plan == "start-plan" && rec.has_jwt {
                    "start-plan".into()
                } else {
                    "coding-plan".into()
                }
            });
        vault.active_id = Some(id.clone());
        if let Some(r) = vault.accounts.iter_mut().find(|a| a.id == id) {
            r.last_used_at = Some(now_ms());
        }
        vault.save(&state.paths.vault_file())?;
        (json, rec.provider, plan, rec.default_model)
    };

    // Config keys follow the account.
    let mut updates = vec![
        yamlpatch::update("provider", provider),
        yamlpatch::update("plan", plan),
    ];
    if let Some(m) = &model {
        updates.push(yamlpatch::update("defaultModel", m.clone()));
    }
    proxy::patch_proxy_config(&state.paths, &updates)?;

    // Restart the process if it is up, so AuthManager picks the new credential.
    let alive = {
        let settings = state.settings.lock().unwrap().clone();
        let managed = state.proxy.lock().unwrap().clone();
        proxy::status(&state.paths, &settings, &managed).process_alive
    };
    if alive {
        stop_proxy_process(&app).await?;
    }
    start_proxy_process(&app).await?;
    Ok(build_status(&app))
}

/// Zero-cost connection test against the provider's models endpoint
/// (validates the key without generating tokens).
#[tauri::command]
pub async fn test_account(app: AppHandle, id: String) -> Result<TestResult, String> {
    let state = app.state::<AppState>();
    let (provider, joined_key) = {
        let vault_key = state.vault_key();
        let vault = state.vault.lock().unwrap();
        let rec = vault
            .accounts
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| "Hesap bulunamadı".to_string())?;
        let cred = rec.decrypt_credential(&vault_key)?;
        let api_key = cred
            .get("apiKey")
            .and_then(|v| v.as_str())
            .ok_or("kimlikte apiKey yok")?;
        let joined = match cred.get("secret").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => format!("{api_key}.{s}"),
            _ => api_key.to_string(),
        };
        (rec.provider.clone(), joined)
    };

    let url = if provider == "zai" {
        "https://api.z.ai/api/paas/v4/models"
    } else {
        "https://open.bigmodel.cn/api/paas/v4/models"
    };

    let started = std::time::Instant::now();
    let res = tauri::async_runtime::spawn_blocking(move || {
        http::get(
            url,
            &[("Authorization", format!("Bearer {joined_key}").as_str())],
            25,
        )
    })
    .await
    .map_err(|e| e.to_string())?;
    let latency = started.elapsed().as_millis() as u64;

    match res {
        Ok(r) if r.status == 200 => Ok(TestResult {
            ok: true,
            message: format!("Bağlantı başarılı — API Key geçerli ({latency} ms)"),
            latency_ms: Some(latency),
        }),
        Ok(r) => Ok(TestResult {
            ok: false,
            message: http::friendly_http_error(r.status, &r.body),
            latency_ms: Some(latency),
        }),
        Err(e) => Ok(TestResult {
            ok: false,
            message: format!("Ağ hatası: {e}"),
            latency_ms: None,
        }),
    }
}

// ---------------------------------------------------------------------------
// OAuth
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn start_oauth(app: AppHandle, provider: String, name: Option<String>) -> Result<(), String> {
    oauth::start(&app, &provider, name)
}

#[tauri::command]
pub fn cancel_oauth(app: AppHandle) {
    oauth::cancel(&app);
}

// ---------------------------------------------------------------------------
// Proxy lifecycle
// ---------------------------------------------------------------------------

async fn stop_proxy_process(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (settings, managed) = {
        let s = state.settings.lock().unwrap().clone();
        let m = state.proxy.lock().unwrap().clone();
        (s, m)
    };
    if managed.panel_port != 0 {
        let st = proxy::shutdown_proxy(&settings, &managed);
        state.proxy.lock().unwrap().child_pid = None;
        st?;
    }
    Ok(())
}

async fn start_proxy_process(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let exe = paths::resolve_proxy_exe();

    // Adopt a proxy left over from a previous session.
    {
        let settings = state.settings.lock().unwrap().clone();
        if let Some(adopted) = proxy::adopt_proxy(&settings) {
            *state.proxy.lock().unwrap() = adopted;
            return Ok(());
        }
    }

    let (settings, cred_json) = {
        let vault_key = state.vault_key();
        let vault = state.vault.lock().unwrap();
        let rec = vault
            .active()
            .ok_or_else(|| "active_account_missing: Önce bir hesap ekleyin/etkinleştirin".to_string())?
            .clone();
        let cred = rec
            .decrypt_credential(&vault_key)
            .map_err(|e| format!("hesap kimliği çözülemedi: {e}"))?;
        (
            state.settings.lock().unwrap().clone(),
            serde_json::to_string(&cred).map_err(|e| e.to_string())?,
        )
    };

    let paths = state.paths.clone();
    let secret = state.secret.clone();
    let settings_for_spawn = settings.clone();
    let managed = tauri::async_runtime::spawn_blocking(move || {
        proxy::spawn_proxy(&exe, &paths, &settings_for_spawn, &secret, &cred_json)
    })
    .await
    .map_err(|e| e.to_string())??;

    {
        // Persist the panel port we actually got.
        let mut s = state.settings.lock().unwrap();
        if s.panel_port != managed.panel_port {
            s.panel_port = managed.panel_port;
            let _ = s.save(&state.paths);
        }
        *state.proxy.lock().unwrap() = managed;
    }
    Ok(())
}

#[tauri::command]
pub async fn proxy_start(app: AppHandle) -> Result<ProxyStatusDto, String> {
    let state = app.state::<AppState>();

    // Already running? Just make sure the server is up.
    {
        let settings = state.settings.lock().unwrap().clone();
        let managed = state.proxy.lock().unwrap().clone();
        let st = proxy::status(&state.paths, &settings, &managed);
        if st.process_alive {
            drop((settings, managed));
            if !st.server_running {
                panel(&app)?.start_server()?;
            }
            emit_proxy_state(&app);
            return Ok(build_status(&app));
        }
    }

    start_proxy_process(&app).await?;
    emit_proxy_state(&app);
    Ok(build_status(&app))
}

#[tauri::command]
pub async fn proxy_stop(app: AppHandle) -> Result<ProxyStatusDto, String> {
    // Server-level stop; the process (and panel: quota/logs) stays alive.
    panel(&app)?.stop_server()?;
    emit_proxy_state(&app);
    Ok(build_status(&app))
}

#[tauri::command]
pub async fn proxy_restart(app: AppHandle) -> Result<ProxyStatusDto, String> {
    stop_proxy_process(&app).await?;
    start_proxy_process(&app).await?;
    emit_proxy_state(&app);
    Ok(build_status(&app))
}

#[tauri::command]
pub async fn proxy_kill(app: AppHandle) -> Result<ProxyStatusDto, String> {
    stop_proxy_process(&app).await?;
    emit_proxy_state(&app);
    Ok(build_status(&app))
}

#[tauri::command]
pub fn proxy_status(app: AppHandle) -> ProxyStatusDto {
    build_status(&app)
}

/// Toggle used by the tray menu.
pub fn toggle_proxy(app: &AppHandle) {
    let state = app.state::<AppState>();
    let st = {
        let settings = state.settings.lock().unwrap().clone();
        let managed = state.proxy.lock().unwrap().clone();
        proxy::status(&state.paths, &settings, &managed)
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if st.process_alive && st.server_running {
            let _ = proxy_stop(app.clone()).await;
        } else {
            let _ = proxy_start(app).await;
        }
    });
}

#[tauri::command]
pub async fn proxy_test(app: AppHandle) -> Result<Value, String> {
    let state = app.state::<AppState>();
    let (port, key) = {
        let cfg = proxy::read_proxy_config(&state.paths)?;
        (
            cfg.pointer("/server/port")
                .and_then(|v| v.as_u64())
                .unwrap_or_default() as u16,
            cfg.pointer("/auth/proxyApiKey")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
        )
    };
    if port == 0 {
        return Err("Proxy portu yapılandırılmamış".into());
    }
    let base = format!("http://127.0.0.1:{port}");
    let key2 = key.clone();
    let health = tauri::async_runtime::spawn_blocking(move || {
        http::get(&format!("{base}/health"), &[("x-api-key", key2.as_str())], 8)
    })
    .await
    .map_err(|e| e.to_string())?;
    let models = tauri::async_runtime::spawn_blocking(move || {
        http::get(
            &format!("http://127.0.0.1:{port}/v1/models"),
            &[("x-api-key", key.as_str())],
            8,
        )
    })
    .await
    .map_err(|e| e.to_string())?;

    let health_ok = health.map(|r| r.status == 200).unwrap_or(false);
    let models_count = models
        .ok()
        .and_then(|r| {
            serde_json::from_str::<Value>(&r.body)
                .ok()
                .and_then(|v| v.get("data").and_then(|d| d.as_array().map(|a| a.len())))
        })
        .unwrap_or(0);

    Ok(json!({
        "ok": health_ok,
        "health": health_ok,
        "modelsCount": models_count,
        "message": if health_ok {
            format!("Proxy yanıt veriyor — {models_count} model listeleniyor")
        } else {
            "Proxy yanıt vermiyor".into()
        }
    }))
}

// ---------------------------------------------------------------------------
// Quota / logs
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn fetch_quota(app: AppHandle) -> Result<Value, String> {
    let p = panel(&app)?;
    // Unwrap the panel envelope: the frontend expects the inner QuotaSnapshot.
    let envelope = tauri::async_runtime::spawn_blocking(move || p.quota())
        .await
        .map_err(|e| e.to_string())??;
    Ok(envelope.get("quota").cloned().unwrap_or(envelope))
}

#[tauri::command]
pub async fn fetch_logs(app: AppHandle, since: i64) -> Result<Value, String> {
    let p = panel(&app)?;
    tauri::async_runtime::spawn_blocking(move || p.logs(since))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn read_error_logs(app: AppHandle, limit: usize) -> Result<Vec<Value>, String> {
    let state = app.state::<AppState>();
    let path = state.paths.proxy_errors_log();
    let lines = proxy::tail_file(&path, limit.max(50).min(2000));
    Ok(lines
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect())
}

// ---------------------------------------------------------------------------
// Models
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsInfo {
    pub available: bool,
    pub endpoint_models: Vec<String>,
    pub config_models: Vec<String>,
    pub default_model: String,
}

#[tauri::command]
pub async fn list_models(app: AppHandle) -> Result<ModelsInfo, String> {
    let state = app.state::<AppState>();
    let cfg = proxy::read_proxy_config(&state.paths)?;
    let config_models: Vec<String> = cfg
        .get("models")
        .and_then(|v| v.as_array())
        .map(|seq| {
            seq.iter()
                .filter_map(|m| m.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let default_model = cfg
        .get("defaultModel")
        .and_then(|v| v.as_str())
        .unwrap_or("glm-5.3")
        .to_string();
    let key = cfg
        .pointer("/auth/proxyApiKey")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let port = cfg
        .pointer("/server/port")
        .and_then(|v| v.as_u64())
        .unwrap_or_default() as u16;

    let settings_models = state.settings.lock().unwrap().recent_models.clone();
    let _ = settings_models; // surfaced to the UI via get_app_state

    let endpoint_models = if port > 0 {
        let url = format!("http://127.0.0.1:{port}/v1/models");
        tauri::async_runtime::spawn_blocking(move || {
            http::get(&url, &[("x-api-key", key.as_str())], 8).ok()
        })
        .await
        .ok()
        .flatten()
        .and_then(|r| {
            serde_json::from_str::<Value>(&r.body).ok().and_then(|v| {
                v.get("data").and_then(|d| d.as_array()).map(|arr| {
                    arr.iter()
                        .filter_map(|m| m.get("id").and_then(|i| i.as_str()))
                        .map(String::from)
                        .collect::<Vec<_>>()
                })
            })
        })
        .unwrap_or_default()
    } else {
        Vec::new()
    };

    Ok(ModelsInfo {
        available: port > 0,
        endpoint_models,
        config_models,
        default_model,
    })
}

#[tauri::command]
pub async fn save_model_prefs(
    app: AppHandle,
    default_model: Option<String>,
    custom_models: Option<Vec<String>>,
    recent_model: Option<String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut updates: Vec<YamlUpdate> = Vec::new();
    if let Some(m) = &default_model {
        check_model(m)?;
        updates.push(yamlpatch::update("defaultModel", m.clone()));
    }
    if let Some(list) = &custom_models {
        for m in list {
            check_model(m)?;
        }
        updates.push(yamlpatch::update_list("models", list.clone()));
    }
    if !updates.is_empty() {
        proxy::patch_proxy_config(&state.paths, &updates)?;
    }
    if let Some(m) = recent_model {
        if claude::valid_model_name(&m) {
            let mut s = state.settings.lock().unwrap();
            s.recent_models.retain(|x| x != &m);
            s.recent_models.insert(0, m);
            s.recent_models.truncate(8);
            s.save(&state.paths)?;
        }
    }
    Ok(())
}

/// Real model-access test: a 1-token generation through the running proxy.
#[tauri::command]
pub async fn test_model(app: AppHandle, model: String) -> Result<TestResult, String> {
    check_model(&model)?;
    let state = app.state::<AppState>();
    let cfg = proxy::read_proxy_config(&state.paths)?;
    let port = cfg
        .pointer("/server/port")
        .and_then(|v| v.as_u64())
        .unwrap_or_default() as u16;
    let key = cfg
        .pointer("/auth/proxyApiKey")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    if port == 0 {
        return Err("Proxy kapalı — önce proxy'yi başlatın".into());
    }
    {
        let st = build_status(&app);
        if !st.status.process_alive || !st.status.server_running {
            return Err("Proxy çalışmıyor — önce proxy'yi başlatın".into());
        }
    }

    let url = format!("http://127.0.0.1:{port}/v1/messages");
    let body = json!({
        "model": model,
        "max_tokens": 1,
        "messages": [{ "role": "user", "content": "hi" }]
    });
    let started = std::time::Instant::now();
    let res = tauri::async_runtime::spawn_blocking(move || {
        http::post_json(
            &url,
            &[("x-api-key", key.as_str())],
            &body,
            90,
        )
    })
    .await
    .map_err(|e| e.to_string())?;
    let latency = started.elapsed().as_millis() as u64;

    match res {
        Ok(r) if r.status == 200 => {
            let input = serde_json::from_str::<Value>(&r.body)
                .ok()
                .and_then(|v| {
                    v.pointer("/usage/input_tokens")
                        .and_then(|u| u.as_u64())
                });
            Ok(TestResult {
                ok: true,
                message: match input {
                    Some(n) => format!("Model erişilebilir — gerçek yanıt alındı ({n} giriş token'i, {latency} ms)"),
                    None => format!("Model erişilebilir — gerçek yanıt alındı ({latency} ms)"),
                },
                latency_ms: Some(latency),
            })
        }
        Ok(r) => Ok(TestResult {
            ok: false,
            message: http::friendly_http_error(r.status, &r.body),
            latency_ms: Some(latency),
        }),
        Err(e) => Ok(TestResult {
            ok: false,
            message: format!("İstek gönderilemedi: {e}"),
            latency_ms: None,
        }),
    }
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigUpdates {
    port: Option<u16>,
    host: Option<String>,
    proxy_api_key: Option<String>,
    provider: Option<String>,
    plan: Option<String>,
    plan_auto_switch: Option<bool>,
    plan_priority: Option<Vec<String>>,
    default_model: Option<String>,
    models: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSaveResult {
    pub restarted: bool,
    pub config: Value,
}

fn sanitized_config(cfg: &Value) -> Value {
    json!({
        "port": cfg.pointer("/server/port").cloned().unwrap_or(json!(8317)),
        "host": cfg.pointer("/server/host").cloned().unwrap_or(json!("127.0.0.1")),
        "proxyApiKey": cfg.pointer("/auth/proxyApiKey").cloned().unwrap_or(Value::Null),
        "provider": cfg.get("provider").cloned().unwrap_or(json!("zai")),
        "plan": cfg.get("plan").cloned().unwrap_or(json!("coding-plan")),
        "planAutoSwitch": cfg.get("planAutoSwitch").cloned().unwrap_or(json!(false)),
        "planPriority": cfg.get("planPriority").cloned().unwrap_or(json!(["start-plan", "coding-plan"])),
        "planSwitchRules": cfg.get("planSwitchRules").cloned().unwrap_or(Value::Null),
        "planPollIntervalSec": cfg.get("planPollIntervalSec").cloned().unwrap_or(json!(30)),
        "defaultModel": cfg.get("defaultModel").cloned().unwrap_or(json!("glm-5.3")),
        "models": cfg.get("models").cloned().unwrap_or(json!([])),
    })
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> Result<Value, String> {
    let state = app.state::<AppState>();
    let cfg = proxy::read_proxy_config(&state.paths)?;
    Ok(sanitized_config(&cfg))
}

#[tauri::command]
pub async fn save_config(app: AppHandle, updates: ConfigUpdates) -> Result<ConfigSaveResult, String> {
    let state = app.state::<AppState>();

    if let Some(p) = &updates.plan_priority {
        if p.is_empty() || p.len() != 2 || !p.iter().all(|x| x == "coding-plan" || x == "start-plan") {
            return Err("planPriority coding-plan ve start-plan'ın bir sıralaması olmalı".into());
        }
    }
    if let Some(p) = &updates.provider {
        if p != "zai" && p != "bigmodel" {
            return Err("Desteklenmeyen sağlayıcı".into());
        }
    }
    if let Some(p) = &updates.plan {
        if p != "coding-plan" && p != "start-plan" {
            return Err("Desteklenmeyen plan".into());
        }
    }
    if let Some(m) = &updates.default_model {
        check_model(m)?;
    }
    if let Some(list) = &updates.models {
        for m in list {
            check_model(m)?;
        }
    }

    let mut yaml_updates: Vec<YamlUpdate> = Vec::new();
    let mut needs_restart = false;

    if let Some(port) = updates.port {
        if port == 0 {
            return Err("Port 0 olamaz".into());
        }
        let current = proxy::config_port(&state.paths);
        if port != current {
            // Refuse a port someone else already holds (unless it is us).
            let managed = state.proxy.lock().unwrap().clone();
            if proxy::port_in_use(port) && managed.panel_port != port {
                return Err(format!(
                    "{port} portu başka bir uygulama tarafından kullanılıyor"
                ));
            }
            needs_restart = true;
        }
        yaml_updates.push(yamlpatch::update_under("server", "port", port.to_string()));
    }
    if let Some(host) = &updates.host {
        let h = host.trim();
        if h != "127.0.0.1" && h != "localhost" && h != "0.0.0.0" && h != "::1" {
            return Err("Host yalnızca 127.0.0.1, localhost, ::1 veya 0.0.0.0 olabilir".into());
        }
        needs_restart = true;
        yaml_updates.push(yamlpatch::update_under("server", "host", format!("\"{h}\"")));
    }
    if let Some(key) = &updates.proxy_api_key {
        let k = key.trim();
        if k.is_empty() {
            yaml_updates.push(yamlpatch::update_under("auth", "proxyApiKey", "null"));
        } else {
            if k.len() < 8 {
                return Err("Proxy anahtarı en az 8 karakter olmalı (güvenlik)".into());
            }
            yaml_updates.push(yamlpatch::update_under("auth", "proxyApiKey", format!("\"{k}\"")));
        }
        // The running server already enforces the old key; restart to apply.
        needs_restart = true;
    }
    if let Some(p) = &updates.provider {
        yaml_updates.push(yamlpatch::update("provider", p.clone()));
    }
    if let Some(p) = &updates.plan {
        yaml_updates.push(yamlpatch::update("plan", p.clone()));
    }
    if let Some(b) = updates.plan_auto_switch {
        yaml_updates.push(yamlpatch::update("planAutoSwitch", b.to_string()));
    }
    if let Some(list) = &updates.plan_priority {
        yaml_updates.push(yamlpatch::update_list("planPriority", list.clone()));
    }
    if let Some(m) = &updates.default_model {
        yaml_updates.push(yamlpatch::update("defaultModel", m.clone()));
    }
    if let Some(list) = &updates.models {
        yaml_updates.push(yamlpatch::update_list("models", list.clone()));
    }

    if !yaml_updates.is_empty() {
        proxy::patch_proxy_config(&state.paths, &yaml_updates)?;
    }

    let alive = {
        let settings = state.settings.lock().unwrap().clone();
        let managed = state.proxy.lock().unwrap().clone();
        proxy::status(&state.paths, &settings, &managed).process_alive
    };
    if needs_restart && alive {
        stop_proxy_process(&app).await?;
        start_proxy_process(&app).await?;
    }

    let cfg = proxy::read_proxy_config(&state.paths)?;
    emit_proxy_state(&app);
    Ok(ConfigSaveResult {
        restarted: needs_restart && alive,
        config: sanitized_config(&cfg),
    })
}

// ---------------------------------------------------------------------------
// Claude Code
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn claude_detect() -> Result<claude::ClaudeInfo, String> {
    tauri::async_runtime::spawn_blocking(claude::detect)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn claude_launch(
    app: AppHandle,
    mode: String,
    model: Option<String>,
    folder: Option<String>,
) -> Result<(), String> {
    if mode != "proxy" && mode != "normal" {
        return Err("Geçersiz mod".into());
    }
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();

    let folder = folder
        .filter(|f| !f.trim().is_empty())
        .or_else(|| settings.claude_default_folder.clone())
        .or_else(|| std::env::var("USERPROFILE").ok())
        .ok_or_else(|| "Proje klasörü belirlenemedi".to_string())?;

    let (endpoint, key, model) = if mode == "proxy" {
        let st = build_status(&app);
        if !st.status.process_alive || !st.status.server_running {
            return Err("Proxy çalışmıyor — önce proxy'yi başlatın ya da Normal mod kullanın".into());
        }
        let cfg = proxy::read_proxy_config(&state.paths)?;
        let port = cfg
            .pointer("/server/port")
            .and_then(|v| v.as_u64())
            .unwrap_or_default() as u16;
        let key = cfg
            .pointer("/auth/proxyApiKey")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let model = model
            .or_else(|| {
                let active = state.vault.lock().unwrap().active().cloned();
                active
                    .and_then(|a| a.default_model)
                    .or(Some(settings.default_model.clone()))
            })
            .unwrap_or_else(|| "glm-5.3".to_string());
        (Some(format!("http://127.0.0.1:{port}")), Some(key), Some(model))
    } else {
        (None, None, model.filter(|m| claude::valid_model_name(m)))
    };

    if let Some(m) = &model {
        check_model(m)?;
    }

    let launches = state.paths.launches_dir().clone();
    let folder_disp = folder.clone();
    tauri::async_runtime::spawn_blocking(move || {
        claude::launch(
            &launches,
            claude::LaunchOptions {
                mode: if mode == "proxy" { "proxy" } else { "normal" },
                model: model.as_deref(),
                folder: Some(&folder_disp),
                endpoint: endpoint.as_deref(),
                proxy_api_key: key.as_deref(),
            },
        )
    })
    .await
    .map_err(|e| e.to_string())??;

    // Remember the folder.
    {
        let mut s = state.settings.lock().unwrap();
        s.recent_folders.retain(|f| f != &folder);
        s.recent_folders.insert(0, folder.clone());
        s.recent_folders.truncate(10);
        let _ = s.save(&state.paths);
    }
    Ok(())
}

#[tauri::command]
pub fn push_recent_folder(app: AppHandle, folder: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut s = state.settings.lock().unwrap();
    s.recent_folders.retain(|f| f != &folder);
    s.recent_folders.insert(0, folder);
    s.recent_folders.truncate(10);
    s.save(&state.paths)
}

#[tauri::command]
pub fn get_versions(app: AppHandle) -> Versions {
    let exe = paths::resolve_proxy_exe();
    Versions {
        app: app.package_info().version.to_string(),
        proxy: claude::proxy_exe_version(&exe),
    }
}

// ---------------------------------------------------------------------------
// Multi-harness session management (Claude Code / OpenCode / Antigravity CLI)
// ---------------------------------------------------------------------------

fn resolve_folder(app: &AppHandle, folder: Option<String>) -> Result<std::path::PathBuf, String> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let chosen = folder
        .filter(|f| !f.trim().is_empty())
        .or_else(|| settings.claude_default_folder.clone())
        .or_else(|| std::env::var("USERPROFILE").ok())
        .ok_or_else(|| "Proje klasörü belirlenemedi".to_string())?;
    let path = std::path::PathBuf::from(&chosen);
    if !path.is_dir() {
        return Err(format!("Klasör bulunamadı: {}", path.display()));
    }
    {
        let mut s = state.settings.lock().unwrap();
        s.recent_folders.retain(|f| f != &chosen);
        s.recent_folders.insert(0, chosen);
        s.recent_folders.truncate(10);
        let _ = s.save(&state.paths);
    }
    Ok(path)
}

async fn ensure_zcode_proxy_ready(
    app: &AppHandle,
    zcode_account_id: &str,
) -> Result<(String, String), String> {
    let state = app.state::<AppState>();
    let already_active_and_up = {
        let vault = state.vault.lock().unwrap();
        let rec = vault
            .accounts
            .iter()
            .find(|a| a.id == zcode_account_id)
            .ok_or_else(|| "ZCode hesabı bulunamadı".to_string())?;
        if !rec.enabled {
            return Err("Seçilen ZCode hesabı devre dışı. Önce hesabı etkinleştirin.".into());
        }
        let is_active = vault.is_active(zcode_account_id);
        let st = build_status(app);
        is_active && st.status.process_alive && st.status.server_running
    };

    if !already_active_and_up {
        activate_account(app.clone(), zcode_account_id.to_string()).await?;
    }

    let cfg = proxy::read_proxy_config(&state.paths)?;
    let port = cfg
        .pointer("/server/port")
        .and_then(|v| v.as_u64())
        .unwrap_or_default() as u16;
    if port == 0 {
        return Err("ZCode proxy portu yapılandırılmamış".into());
    }
    let key = cfg
        .pointer("/auth/proxyApiKey")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    Ok((format!("http://127.0.0.1:{port}"), key))
}

#[tauri::command]
pub fn update_session(
    app: AppHandle,
    id: String,
    name: String,
) -> Result<Vec<HarnessSessionSummary>, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Oturum adı boş olamaz".into());
    }
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let mut sessions = state.sessions.lock().unwrap();
    let rec = sessions
        .sessions
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| "Oturum bulunamadı".to_string())?;
    rec.name = trimmed.to_string();
    sessions.save(&state.paths.sessions_file())?;
    Ok(sessions.summaries(
        settings.claude_active_session_id.as_deref(),
        settings.opencode_active_session_id.as_deref(),
        settings.antigravity_active_session_id.as_deref(),
    ))
}

#[tauri::command]
pub fn set_session_enabled(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<Vec<HarnessSessionSummary>, String> {
    let (harness, was_active) = {
        let state = app.state::<AppState>();
        let settings = state.settings.lock().unwrap().clone();
        let mut sessions = state.sessions.lock().unwrap();
        let rec = sessions
            .sessions
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| "Oturum bulunamadı".to_string())?;
        rec.enabled = enabled;
        let h = rec.harness.clone();
        let active = match h.as_str() {
            "claude" => settings.claude_active_session_id.as_deref() == Some(id.as_str()),
            "opencode" => settings.opencode_active_session_id.as_deref() == Some(id.as_str()),
            "antigravity" => settings.antigravity_active_session_id.as_deref() == Some(id.as_str()),
            _ => false,
        };
        sessions.save(&state.paths.sessions_file())?;
        (h, active)
    };

    if !enabled && was_active {
        match harness.as_str() {
            "claude" => {
                let state = app.state::<AppState>();
                let vault_key = state.vault_key();
                let sessions = state.sessions.lock().unwrap();
                if let Ok(paths) = ClaudePaths::live() {
                    let _ = claude_harness::disable_at(&paths, true, &sessions, &vault_key);
                }
                let mut s = state.settings.lock().unwrap();
                s.claude_active_method = "disabled".into();
                s.claude_active_session_id = None;
                s.claude_active_model = None;
                let _ = s.save(&state.paths);
            }
            "opencode" => {
                let state = app.state::<AppState>();
                let vault_key = state.vault_key();
                let sessions = state.sessions.lock().unwrap();
                if let Ok(paths) = OpenCodePaths::live() {
                    let _ = opencode::disable_at(&paths, &sessions, &vault_key);
                }
                let mut s = state.settings.lock().unwrap();
                s.opencode_active_session_id = None;
                let _ = s.save(&state.paths);
            }
            "antigravity" => {
                let state = app.state::<AppState>();
                let vault_key = state.vault_key();
                let sessions = state.sessions.lock().unwrap();
                let _ = antigravity::disable(&sessions, &vault_key);
                let mut s = state.settings.lock().unwrap();
                s.antigravity_active_session_id = None;
                let _ = s.save(&state.paths);
            }
            _ => {}
        }
    }

    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let sessions = state.sessions.lock().unwrap();
    Ok(sessions.summaries(
        settings.claude_active_session_id.as_deref(),
        settings.opencode_active_session_id.as_deref(),
        settings.antigravity_active_session_id.as_deref(),
    ))
}

#[tauri::command]
pub fn delete_session(app: AppHandle, id: String) -> Result<Value, String> {
    let state = app.state::<AppState>();
    {
        let mut sessions = state.sessions.lock().unwrap();
        let before = sessions.sessions.len();
        sessions.sessions.retain(|s| s.id != id);
        if sessions.sessions.len() == before {
            return Err("Oturum bulunamadı".into());
        }
        sessions.save(&state.paths.sessions_file())?;
    }
    {
        let mut s = state.settings.lock().unwrap();
        let mut changed = false;
        if s.claude_active_session_id.as_deref() == Some(id.as_str()) {
            s.claude_active_session_id = None;
            changed = true;
        }
        if s.opencode_active_session_id.as_deref() == Some(id.as_str()) {
            s.opencode_active_session_id = None;
            changed = true;
        }
        if s.antigravity_active_session_id.as_deref() == Some(id.as_str()) {
            s.antigravity_active_session_id = None;
            changed = true;
        }
        if changed {
            let _ = s.save(&state.paths);
        }
    }
    Ok(json!({ "ok": true }))
}

#[tauri::command]
pub fn cancel_harness_oauth(app: AppHandle) {
    let state = app.state::<AppState>();
    let mut flow = state.harness_oauth.lock().unwrap();
    flow.running = false;
    flow.child_pid = None;
    if let Some(dir) = flow.temp_dir.take() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

// --- Claude Code multi-method ---

#[tauri::command]
pub fn claude_capture_oauth(
    app: AppHandle,
    name: Option<String>,
) -> Result<HarnessSessionSummary, String> {
    let paths = ClaudePaths::live()?;
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let rec = claude_harness::capture_current_oauth_at(&paths, &vault_key, name.as_deref())?;
    let id = rec.id.clone();
    let mut sessions = state.sessions.lock().unwrap();
    sessions.sessions.push(rec);
    sessions.save(&state.paths.sessions_file())?;
    let settings = state.settings.lock().unwrap().clone();
    let summary = sessions
        .sessions
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.summary(settings.claude_active_session_id.as_deref() == Some(id.as_str())))
        .ok_or_else(|| "Kayıt bulunamadı".to_string())?;
    Ok(summary)
}

#[tauri::command]
pub fn claude_start_oauth_login(app: AppHandle, name: Option<String>) -> Result<(), String> {
    claude_harness::start_oauth_login(&app, name)
}

#[tauri::command]
pub fn claude_add_api_session(
    app: AppHandle,
    name: String,
    provider_id: String,
    api_key: String,
    base_url: Option<String>,
    model: Option<String>,
) -> Result<HarnessSessionSummary, String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let rec = claude_harness::create_api_session(
        &vault_key,
        &name,
        &provider_id,
        &api_key,
        base_url.as_deref(),
        model.as_deref(),
    )?;
    let id = rec.id.clone();
    let mut sessions = state.sessions.lock().unwrap();
    sessions.sessions.push(rec);
    sessions.save(&state.paths.sessions_file())?;
    let settings = state.settings.lock().unwrap().clone();
    sessions
        .sessions
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.summary(settings.claude_active_session_id.as_deref() == Some(id.as_str())))
        .ok_or_else(|| "Kayıt bulunamadı".to_string())
}

#[tauri::command]
pub fn claude_edit_session(
    app: AppHandle,
    id: String,
    name: Option<String>,
    provider_id: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
) -> Result<Vec<HarnessSessionSummary>, String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let settings = state.settings.lock().unwrap().clone();
    let mut sessions = state.sessions.lock().unwrap();
    let rec = sessions
        .sessions
        .iter_mut()
        .find(|s| s.id == id && s.harness == "claude")
        .ok_or("Claude Code oturumu bulunamadı")?;
    claude_harness::update_api_session(
        rec,
        &vault_key,
        name.as_deref(),
        provider_id.as_deref(),
        api_key.as_deref(),
        base_url.as_deref(),
        model.as_deref(),
    )?;
    sessions.save(&state.paths.sessions_file())?;
    Ok(sessions.summaries(
        settings.claude_active_session_id.as_deref(),
        settings.opencode_active_session_id.as_deref(),
        settings.antigravity_active_session_id.as_deref(),
    ))
}

#[tauri::command]
pub async fn claude_set_method(
    app: AppHandle,
    method: String,
    target_id: Option<String>,
    model: Option<String>,
    clear_oauth_credentials: Option<bool>,
) -> Result<(), String> {
    let paths = ClaudePaths::live()?;
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();

    match method.as_str() {
        "claude_oauth" => {
            let sid = target_id
                .filter(|s| !s.trim().is_empty())
                .ok_or("Bir Claude OAuth oturumu seçin")?;
            let payload = {
                let mut sessions = state.sessions.lock().unwrap();
                let rec = sessions
                    .sessions
                    .iter_mut()
                    .find(|s| s.id == sid && s.harness == "claude")
                    .ok_or("Claude OAuth oturumu bulunamadı")?;
                if !rec.enabled {
                    return Err("Bu oturum devre dışı. Önce etkinleştirin.".into());
                }
                let payload = rec.decrypt_payload(&vault_key)?;
                rec.last_used_at = Some(now_ms());
                sessions.save(&state.paths.sessions_file())?;
                payload
            };
            claude_harness::apply_claude_oauth_at(&paths, &payload)?;
            let mut s = state.settings.lock().unwrap();
            s.claude_active_method = "claude_oauth".into();
            s.claude_active_session_id = Some(sid);
            s.claude_active_model = None;
            s.save(&state.paths)?;
        }
        "zcode_oauth" => {
            let acc_id = target_id
                .filter(|s| !s.trim().is_empty())
                .ok_or("Bir ZCode hesabı seçin")?;
            let (endpoint, proxy_key) = ensure_zcode_proxy_ready(&app, &acc_id).await?;
            let chosen_model = {
                let settings = state.settings.lock().unwrap().clone();
                let vault = state.vault.lock().unwrap();
                let acc_model = vault
                    .accounts
                    .iter()
                    .find(|a| a.id == acc_id)
                    .and_then(|a| a.default_model.clone());
                model
                    .filter(|m| !m.trim().is_empty())
                    .or(acc_model)
                    .unwrap_or_else(|| settings.default_model.clone())
            };
            let token = if proxy_key.trim().is_empty() {
                "harnessauth-local"
            } else {
                proxy_key.trim()
            };
            claude_harness::apply_env_bridge_at(&paths, &endpoint, token, Some(&chosen_model))?;
            let mut s = state.settings.lock().unwrap();
            s.claude_active_method = "zcode_oauth".into();
            s.claude_active_session_id = Some(acc_id);
            s.claude_active_model = Some(chosen_model);
            s.save(&state.paths)?;
        }
        "z_api" => {
            let acc_id = target_id
                .filter(|s| !s.trim().is_empty())
                .ok_or("Bir Z API hesabı seçin")?;
            let (base_url, token, chosen_model) = {
                let settings = state.settings.lock().unwrap().clone();
                let mut vault = state.vault.lock().unwrap();
                let rec = vault
                    .accounts
                    .iter_mut()
                    .find(|a| a.id == acc_id)
                    .ok_or("Z API hesabı bulunamadı")?;
                if !rec.enabled {
                    return Err("Bu Z API hesabı devre dışı. Önce etkinleştirin.".into());
                }
                let cred = rec.decrypt_credential(&vault_key)?;
                let api_key = cred
                    .get("apiKey")
                    .and_then(|v| v.as_str())
                    .ok_or("Hesapta apiKey bulunamadı")?;
                let joined = match cred.get("secret").and_then(|v| v.as_str()) {
                    Some(sec) if !sec.is_empty() => format!("{api_key}.{sec}"),
                    _ => api_key.to_string(),
                };
                let base_url =
                    claude_harness::resolve_z_api_base(&rec.provider, rec.base_url.as_deref())?;
                let m = model
                    .filter(|x| !x.trim().is_empty())
                    .or_else(|| rec.default_model.clone())
                    .unwrap_or_else(|| settings.default_model.clone());
                rec.last_used_at = Some(now_ms());
                vault.save(&state.paths.vault_file())?;
                (base_url, joined, m)
            };
            claude_harness::apply_env_bridge_at(&paths, &base_url, &token, Some(&chosen_model))?;
            let mut s = state.settings.lock().unwrap();
            s.claude_active_method = "z_api".into();
            s.claude_active_session_id = Some(acc_id);
            s.claude_active_model = Some(chosen_model);
            s.save(&state.paths)?;
        }
        "custom_api" => {
            let sid = target_id
                .filter(|s| !s.trim().is_empty())
                .ok_or("Bir Claude API oturumu seçin")?;
            let (payload, meta_model) = {
                let mut sessions = state.sessions.lock().unwrap();
                let rec = sessions
                    .sessions
                    .iter_mut()
                    .find(|s| s.id == sid && s.harness == "claude" && s.kind == "claude_api")
                    .ok_or("Claude API oturumu bulunamadı")?;
                if !rec.enabled {
                    return Err("Bu oturum devre dışı. Önce etkinleştirin.".into());
                }
                let payload = rec.decrypt_payload(&vault_key)?;
                let meta_model = payload
                    .get("model")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                rec.last_used_at = Some(now_ms());
                sessions.save(&state.paths.sessions_file())?;
                (payload, meta_model)
            };
            let chosen_model = model
                .filter(|m| !m.trim().is_empty())
                .or(meta_model);
            claude_harness::apply_claude_api_at(&paths, &payload, chosen_model.as_deref())?;
            let mut s = state.settings.lock().unwrap();
            s.claude_active_method = "custom_api".into();
            s.claude_active_session_id = Some(sid);
            s.claude_active_model = chosen_model;
            s.save(&state.paths)?;
        }
        "disabled" => {
            let backup = {
                let sessions = state.sessions.lock().unwrap();
                claude_harness::disable_at(
                    &paths,
                    clear_oauth_credentials.unwrap_or(true),
                    &sessions,
                    &vault_key,
                )?
            };
            if let Some(rec) = backup {
                let mut sessions = state.sessions.lock().unwrap();
                sessions.sessions.push(rec);
                sessions.save(&state.paths.sessions_file())?;
            }
            let mut s = state.settings.lock().unwrap();
            s.claude_active_method = "disabled".into();
            s.claude_active_session_id = None;
            s.claude_active_model = None;
            s.save(&state.paths)?;
        }
        other => return Err(format!("Geçersiz Claude Code yöntemi: {other}")),
    }

    emit_proxy_state(&app);
    Ok(())
}

#[tauri::command]
pub async fn claude_launch_folder(app: AppHandle, folder: Option<String>) -> Result<(), String> {
    let dir = resolve_folder(&app, folder)?;
    let launches = app.state::<AppState>().paths.launches_dir();
    tauri::async_runtime::spawn_blocking(move || claude_harness::launch_claude_folder(&launches, &dir))
        .await
        .map_err(|e| e.to_string())?
}

// --- OpenCode ---

#[tauri::command]
pub fn opencode_capture_current(
    app: AppHandle,
    name: Option<String>,
) -> Result<HarnessSessionSummary, String> {
    let paths = OpenCodePaths::live()?;
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let rec = opencode::capture_current_at(&paths, &vault_key, name.as_deref())?;
    let id = rec.id.clone();
    let mut sessions = state.sessions.lock().unwrap();
    sessions.sessions.push(rec);
    sessions.save(&state.paths.sessions_file())?;
    let settings = state.settings.lock().unwrap().clone();
    sessions
        .sessions
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.summary(settings.opencode_active_session_id.as_deref() == Some(id.as_str())))
        .ok_or_else(|| "Kayıt bulunamadı".to_string())
}

#[tauri::command]
pub fn opencode_add_api_session(
    app: AppHandle,
    name: String,
    provider_id: String,
    api_key: String,
    base_url: Option<String>,
    model: Option<String>,
) -> Result<HarnessSessionSummary, String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let rec = opencode::create_api_session(
        &vault_key,
        &name,
        &provider_id,
        &api_key,
        base_url.as_deref(),
        model.as_deref(),
    )?;
    let id = rec.id.clone();
    let mut sessions = state.sessions.lock().unwrap();
    sessions.sessions.push(rec);
    sessions.save(&state.paths.sessions_file())?;
    let settings = state.settings.lock().unwrap().clone();
    sessions
        .sessions
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.summary(settings.opencode_active_session_id.as_deref() == Some(id.as_str())))
        .ok_or_else(|| "Kayıt bulunamadı".to_string())
}

#[tauri::command]
pub fn opencode_add_zcode_session(
    app: AppHandle,
    name: String,
    zcode_account_id: String,
    model: Option<String>,
) -> Result<HarnessSessionSummary, String> {
    let state = app.state::<AppState>();
    let acc_name = {
        let vault = state.vault.lock().unwrap();
        let rec = vault
            .accounts
            .iter()
            .find(|a| a.id == zcode_account_id)
            .ok_or("ZCode hesabı bulunamadı")?;
        rec.name.clone()
    };
    let vault_key = state.vault_key();
    let rec = opencode::create_zcode_bridge_session(
        &vault_key,
        &name,
        &zcode_account_id,
        &acc_name,
        model.as_deref(),
    )?;
    let id = rec.id.clone();
    let mut sessions = state.sessions.lock().unwrap();
    sessions.sessions.push(rec);
    sessions.save(&state.paths.sessions_file())?;
    let settings = state.settings.lock().unwrap().clone();
    sessions
        .sessions
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.summary(settings.opencode_active_session_id.as_deref() == Some(id.as_str())))
        .ok_or_else(|| "Kayıt bulunamadı".to_string())
}

#[tauri::command]
pub fn opencode_start_oauth_login(app: AppHandle, name: Option<String>) -> Result<(), String> {
    opencode::start_oauth_login(&app, name)
}

#[tauri::command]
pub async fn opencode_activate_session(app: AppHandle, id: String) -> Result<(), String> {
    let paths = OpenCodePaths::live()?;
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let rec = {
        let sessions = state.sessions.lock().unwrap();
        sessions
            .sessions
            .iter()
            .find(|s| s.id == id && s.harness == "opencode")
            .cloned()
            .ok_or("OpenCode oturumu bulunamadı")?
    };
    if !rec.enabled {
        return Err("Bu OpenCode oturumu devre dışı. Önce etkinleştirin.".into());
    }

    let zcode_info = if rec.kind == "opencode_zcode_proxy" {
        let payload = rec.decrypt_payload(&vault_key)?;
        let acc_id = payload
            .get("zcodeAccountId")
            .and_then(|v| v.as_str())
            .ok_or("ZCode hesap kimliği eksik")?
            .to_string();
        Some(ensure_zcode_proxy_ready(&app, &acc_id).await?)
    } else {
        None
    };

    opencode::activate_session_at(
        &paths,
        &rec,
        &vault_key,
        zcode_info
            .as_ref()
            .map(|(e, k)| (e.as_str(), k.as_str())),
    )?;

    {
        let mut sessions = state.sessions.lock().unwrap();
        if let Some(r) = sessions.sessions.iter_mut().find(|s| s.id == id) {
            r.last_used_at = Some(now_ms());
        }
        sessions.save(&state.paths.sessions_file())?;
    }
    {
        let mut s = state.settings.lock().unwrap();
        s.opencode_active_method = "profile".into();
        s.opencode_active_session_id = Some(id);
        s.opencode_active_model = rec
            .meta
            .get("model")
            .and_then(|v| v.as_str())
            .map(String::from);
        s.save(&state.paths)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn opencode_set_method(
    app: AppHandle,
    method: String,
    target_id: Option<String>,
    model: Option<String>,
) -> Result<(), String> {
    let paths = OpenCodePaths::live()?;
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();

    match method.as_str() {
        "profile" => {
            let sid = target_id
                .filter(|s| !s.trim().is_empty())
                .ok_or("Bir OpenCode oturumu seçin")?;
            return opencode_activate_session(app, sid).await;
        }
        "zcode_oauth" => {
            let acc_id = target_id
                .filter(|s| !s.trim().is_empty())
                .ok_or("Bir ZCode OAuth hesabı seçin")?;
            let (endpoint, proxy_key) = ensure_zcode_proxy_ready(&app, &acc_id).await?;
            let chosen_model = {
                let settings = state.settings.lock().unwrap().clone();
                let vault = state.vault.lock().unwrap();
                let acc_model = vault
                    .accounts
                    .iter()
                    .find(|a| a.id == acc_id)
                    .and_then(|a| a.default_model.clone());
                model
                    .filter(|m| !m.trim().is_empty())
                    .or(acc_model)
                    .unwrap_or_else(|| settings.default_model.clone())
            };
            opencode::activate_zcode_oauth_direct_at(
                &paths,
                &endpoint,
                &proxy_key,
                Some(&chosen_model),
            )?;
            let mut s = state.settings.lock().unwrap();
            s.opencode_active_method = "zcode_oauth".into();
            s.opencode_active_session_id = Some(acc_id);
            s.opencode_active_model = Some(chosen_model);
            s.save(&state.paths)?;
        }
        "z_api" => {
            let acc_id = target_id
                .filter(|s| !s.trim().is_empty())
                .ok_or("Bir Z API hesabı seçin")?;
            let (provider, token, custom_base, chosen_model) = {
                let settings = state.settings.lock().unwrap().clone();
                let mut vault = state.vault.lock().unwrap();
                let rec = vault
                    .accounts
                    .iter_mut()
                    .find(|a| a.id == acc_id)
                    .ok_or("Z API hesabı bulunamadı")?;
                if !rec.enabled {
                    return Err("Bu Z API hesabı devre dışı. Önce etkinleştirin.".into());
                }
                let cred = rec.decrypt_credential(&vault_key)?;
                let api_key = cred
                    .get("apiKey")
                    .and_then(|v| v.as_str())
                    .ok_or("Hesapta apiKey bulunamadı")?;
                let joined = match cred.get("secret").and_then(|v| v.as_str()) {
                    Some(sec) if !sec.is_empty() => format!("{api_key}.{sec}"),
                    _ => api_key.to_string(),
                };
                let m = model
                    .filter(|x| !x.trim().is_empty())
                    .or_else(|| rec.default_model.clone())
                    .unwrap_or_else(|| settings.default_model.clone());
                rec.last_used_at = Some(now_ms());
                let provider = rec.provider.clone();
                let base_url = rec.base_url.clone();
                vault.save(&state.paths.vault_file())?;
                (provider, joined, base_url, m)
            };
            opencode::activate_z_api_direct_at(
                &paths,
                &provider,
                &token,
                custom_base.as_deref(),
                Some(&chosen_model),
            )?;
            let mut s = state.settings.lock().unwrap();
            s.opencode_active_method = "z_api".into();
            s.opencode_active_session_id = Some(acc_id);
            s.opencode_active_model = Some(chosen_model);
            s.save(&state.paths)?;
        }
        "disabled" => {
            return opencode_disable(app);
        }
        other => return Err(format!("Geçersiz OpenCode yöntemi: {other}")),
    }

    emit_proxy_state(&app);
    Ok(())
}

#[tauri::command]
pub fn opencode_edit_session(
    app: AppHandle,
    id: String,
    name: Option<String>,
    provider_id: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    zcode_account_id: Option<String>,
) -> Result<Vec<HarnessSessionSummary>, String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let settings = state.settings.lock().unwrap().clone();
    let zcode_account_name = zcode_account_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|zid| {
            state
                .vault
                .lock()
                .unwrap()
                .accounts
                .iter()
                .find(|a| a.id == zid)
                .map(|a| a.name.clone())
        });
    let mut sessions = state.sessions.lock().unwrap();
    let rec = sessions
        .sessions
        .iter_mut()
        .find(|s| s.id == id && s.harness == "opencode")
        .ok_or("OpenCode oturumu bulunamadı")?;
    opencode::update_session_record(
        rec,
        &vault_key,
        name.as_deref(),
        provider_id.as_deref(),
        api_key.as_deref(),
        base_url.as_deref(),
        model.as_deref(),
        zcode_account_id.as_deref(),
        zcode_account_name.as_deref(),
    )?;
    sessions.save(&state.paths.sessions_file())?;
    Ok(sessions.summaries(
        settings.claude_active_session_id.as_deref(),
        settings.opencode_active_session_id.as_deref(),
        settings.antigravity_active_session_id.as_deref(),
    ))
}

#[tauri::command]
pub fn opencode_disable(app: AppHandle) -> Result<(), String> {
    let paths = OpenCodePaths::live()?;
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let backup = {
        let sessions = state.sessions.lock().unwrap();
        opencode::disable_at(&paths, &sessions, &vault_key)?
    };
    if let Some(rec) = backup {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.sessions.push(rec);
        sessions.save(&state.paths.sessions_file())?;
    }
    let mut s = state.settings.lock().unwrap();
    s.opencode_active_method = "disabled".into();
    s.opencode_active_session_id = None;
    s.opencode_active_model = None;
    s.save(&state.paths)?;
    Ok(())
}

#[tauri::command]
pub async fn opencode_launch_folder(app: AppHandle, folder: Option<String>) -> Result<(), String> {
    let dir = resolve_folder(&app, folder)?;
    let launches = app.state::<AppState>().paths.launches_dir();
    tauri::async_runtime::spawn_blocking(move || opencode::launch_opencode_folder(&launches, &dir))
        .await
        .map_err(|e| e.to_string())?
}

// --- Antigravity CLI (agy) ---

#[tauri::command]
pub fn antigravity_capture_current(
    app: AppHandle,
    name: Option<String>,
) -> Result<HarnessSessionSummary, String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let rec = antigravity::capture_current(&vault_key, name.as_deref())?;
    let id = rec.id.clone();
    let mut sessions = state.sessions.lock().unwrap();
    sessions.sessions.push(rec);
    sessions.save(&state.paths.sessions_file())?;
    let settings = state.settings.lock().unwrap().clone();
    sessions
        .sessions
        .iter()
        .find(|s| s.id == id)
        .map(|s| s.summary(settings.antigravity_active_session_id.as_deref() == Some(id.as_str())))
        .ok_or_else(|| "Kayıt bulunamadı".to_string())
}

#[tauri::command]
pub fn antigravity_start_oauth_login(app: AppHandle, name: Option<String>) -> Result<(), String> {
    antigravity::start_oauth_login(&app, name)
}

#[tauri::command]
pub fn antigravity_activate_session(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let rec = {
        let sessions = state.sessions.lock().unwrap();
        sessions
            .sessions
            .iter()
            .find(|s| s.id == id && s.harness == "antigravity")
            .cloned()
            .ok_or("Antigravity CLI oturumu bulunamadı")?
    };
    if !rec.enabled {
        return Err("Bu Antigravity oturumu devre dışı. Önce etkinleştirin.".into());
    }
    antigravity::activate_session(&rec, &vault_key)?;
    {
        let mut sessions = state.sessions.lock().unwrap();
        if let Some(r) = sessions.sessions.iter_mut().find(|s| s.id == id) {
            r.last_used_at = Some(now_ms());
        }
        sessions.save(&state.paths.sessions_file())?;
    }
    {
        let mut s = state.settings.lock().unwrap();
        s.antigravity_active_session_id = Some(id);
        s.save(&state.paths)?;
    }
    Ok(())
}

#[tauri::command]
pub fn antigravity_disable(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let backup = {
        let sessions = state.sessions.lock().unwrap();
        antigravity::disable(&sessions, &vault_key)?
    };
    if let Some(rec) = backup {
        let mut sessions = state.sessions.lock().unwrap();
        sessions.sessions.push(rec);
        sessions.save(&state.paths.sessions_file())?;
    }
    let mut s = state.settings.lock().unwrap();
    s.antigravity_active_session_id = None;
    s.save(&state.paths)?;
    Ok(())
}

#[tauri::command]
pub async fn antigravity_launch_folder(
    app: AppHandle,
    folder: Option<String>,
) -> Result<(), String> {
    let dir = resolve_folder(&app, folder)?;
    let launches = app.state::<AppState>().paths.launches_dir();
    tauri::async_runtime::spawn_blocking(move || antigravity::launch_agy_folder(&launches, &dir))
        .await
        .map_err(|e| e.to_string())?
}

// Keep unused harness module import satisfied if any helper is only used conditionally.
#[allow(dead_code)]
fn _touch_harness(_: &harness::CliInfo) {}

// ---------------------------------------------------------------------------
// Vault maintenance for tests
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn vault_snapshot(state: &AppState) -> VaultData {
    state.vault.lock().unwrap().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitized_config_shapes_defaults() {
        let text = crate::proxy::template(8317, "127.0.0.1", "abc");
        let cfg: Value = serde_yaml::from_str(&text).unwrap();
        let s = sanitized_config(&serde_json::to_value(&cfg).unwrap());
        assert_eq!(s["port"], json!(8317));
        assert_eq!(s["host"], json!("127.0.0.1"));
        assert_eq!(s["planAutoSwitch"], json!(false));
    }
}
