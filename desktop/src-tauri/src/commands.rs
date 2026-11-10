//! Tauri command surface — the only bridge between the React UI and the
//! Rust backend. Secrets (credentials, master secret) never leave this layer;
//! the frontend receives masked summaries only.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::claude;
use crate::credstore;
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
    pub proxy: ProxyStatusDto,
    pub claude: claude::ClaudeInfo,
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
    let (claude_info, proxy_version) = tauri::async_runtime::spawn_blocking(move || {
        (claude::detect(), claude::proxy_exe_version(&proxy_exe))
    })
    .await
    .map_err(|e| e.to_string())?;

    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let accounts = state.vault.lock().unwrap().summaries();

    Ok(AppStateDto {
        settings,
        accounts,
        proxy: build_status(&app),
        claude: claude_info,
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
    {
        let mut s = state.settings.lock().unwrap();
        // Panel identity is managed by the backend; never accept client edits.
        let (panel_port, panel_token) = (s.panel_port, s.panel_token.clone());
        *s = new_settings.clone();
        s.panel_port = panel_port;
        s.panel_token = panel_token;
        s.sync_autostart()?;
        s.save(&state.paths)?;
    }
    emit_proxy_state(&app);
    Ok(new_settings)
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
) -> Result<Vec<AccountSummary>, String> {
    if let Some(m) = &default_model {
        check_model(m)?;
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
    vault.save(&state.paths.vault_file())?;
    let summaries = vault.summaries();
    drop(vault);
    emit_proxy_state(&app);
    Ok(summaries)
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
