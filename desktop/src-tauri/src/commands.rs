//! Tauri command surface for HarnessAuth v2.
//!
//! Centralized encrypted vault (`vault.json`) + one-click switching across:
//! - Claude Code (`claude`)
//! - OpenCode (`opencode`, `opencode.jsonc`)
//! - ZCode Desktop (`~/.zcode/v2/`)
//! - Antigravity CLI (`agy`)
//! - Background ZCode Service (Start Plan / Coding Plan + quota + logs)

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::claude;
use crate::credstore;
use crate::harness::{
    self,
    antigravity::{self, AntigravityLiveState},
    claude_harness::{self, ClaudeLiveState, ClaudePaths},
    openai_bridge::BridgeTarget,
    opencode::{self, OpenCodeLiveState, OpenCodePaths},
    zcode_harness::{self, ZCodeLiveState, ZCodePaths},
};
use crate::http;
use crate::oauth;
use crate::panel::PanelClient;
use crate::proxy::{self, ProxyStatus};
use crate::settings::Settings;
use crate::state::AppState;
use crate::unified_vault::{CustomModelDef, UnifiedAccount, UnifiedAccountSummary};
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
    pub zcode_account_count: usize,
    pub oauth_running: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStateDto {
    pub settings: Settings,
    pub accounts: Vec<UnifiedAccountSummary>,
    pub proxy: ProxyStatusDto,
    pub claude_live: ClaudeLiveState,
    pub opencode_live: OpenCodeLiveState,
    pub zcode_live: ZCodeLiveState,
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
    pub vault: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub ok: bool,
    pub message: String,
    pub latency_ms: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResultDto {
    pub imported: usize,
    pub details: Vec<String>,
}

// ---------------------------------------------------------------------------
// Status & config helpers
// ---------------------------------------------------------------------------

fn build_status(app: &AppHandle) -> ProxyStatusDto {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let managed = state.proxy.lock().unwrap().clone();
    let oauth_running = state.oauth.lock().unwrap().running;
    let (active_account_id, active_account_name, zcode_account_count) = {
        let vault = state.vault.lock().unwrap();
        let active_id = settings.zcode_service_account_id.clone();
        let active_name = active_id
            .as_deref()
            .and_then(|id| vault.find(id))
            .map(|a| a.name.clone());
        let count = vault
            .accounts
            .iter()
            .filter(|a| a.kind == "zcode_oauth")
            .count();
        (active_id, active_name, count)
    };
    let status = proxy::status(&state.paths, &settings, &managed);
    ProxyStatusDto {
        status,
        active_account_id,
        active_account_name,
        zcode_account_count,
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

fn check_model(model: &str) -> Result<(), String> {
    if claude::valid_model_name(model) {
        Ok(())
    } else {
        Err(format!("Geçersiz model adı: {model}"))
    }
}

fn save_vault_and_settings(state: &AppState) -> Result<(), String> {
    {
        let mut s = state.settings.lock().unwrap();
        let vault = state.vault.lock().unwrap();
        vault.reconcile_settings(&mut s);
        vault.save(&state.paths.unified_vault_file())?;
        s.save(&state.paths)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// App state / settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_app_state(app: AppHandle) -> Result<AppStateDto, String> {
    let state = app.state::<AppState>();
    let proxy_exe = state.paths.proxy_exe();
    let (claude_live, opencode_live, zcode_live, antigravity_live, proxy_version) =
        tauri::async_runtime::spawn_blocking(move || {
            (
                claude_harness::inspect_live(),
                opencode::inspect_live(),
                zcode_harness::inspect_live(),
                antigravity::inspect_live(),
                claude::proxy_exe_version(&proxy_exe),
            )
        })
        .await
        .map_err(|e| e.to_string())?;

    let settings = state.settings.lock().unwrap().clone();
    let accounts = state.vault.lock().unwrap().summaries(&settings);
    let (harness_oauth_running, harness_oauth_target) = {
        let h = state.harness_oauth.lock().unwrap();
        (h.running, h.running.then(|| h.harness.clone()))
    };

    Ok(AppStateDto {
        settings,
        accounts,
        proxy: build_status(&app),
        claude_live,
        opencode_live,
        zcode_live,
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
            vault: state.paths.unified_vault_file().display().to_string(),
        },
    })
}

#[tauri::command]
pub async fn save_settings(app: AppHandle, new_settings: Settings) -> Result<Settings, String> {
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
        let (
            panel_port,
            panel_token,
            zcode_service_account_id,
            claude_active_method,
            claude_active_session_id,
            claude_active_model,
            opencode_active_session_id,
            opencode_active_method,
            opencode_active_model,
            zcode_app_active_account_id,
            zcode_app_active_method,
            antigravity_active_session_id,
        ) = (
            s.panel_port,
            s.panel_token.clone(),
            s.zcode_service_account_id.clone(),
            s.claude_active_method.clone(),
            s.claude_active_session_id.clone(),
            s.claude_active_model.clone(),
            s.opencode_active_session_id.clone(),
            s.opencode_active_method.clone(),
            s.opencode_active_model.clone(),
            s.zcode_app_active_account_id.clone(),
            s.zcode_app_active_method.clone(),
            s.antigravity_active_session_id.clone(),
        );
        *s = new_settings;
        s.panel_port = panel_port;
        s.panel_token = panel_token;
        s.zcode_service_account_id = zcode_service_account_id;
        s.claude_active_method = claude_active_method;
        s.claude_active_session_id = claude_active_session_id;
        s.claude_active_model = claude_active_model;
        s.opencode_active_session_id = opencode_active_session_id;
        s.opencode_active_method = opencode_active_method;
        s.opencode_active_model = opencode_active_model;
        s.zcode_app_active_account_id = zcode_app_active_account_id;
        s.zcode_app_active_method = zcode_app_active_method;
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

#[tauri::command]
pub fn get_versions(app: AppHandle) -> Versions {
    let exe = app.state::<AppState>().paths.proxy_exe();
    Versions {
        app: app.package_info().version.to_string(),
        proxy: claude::proxy_exe_version(&exe),
    }
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

// ---------------------------------------------------------------------------
// Centralized Unified Vault Accounts & Custom Models
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn add_z_api_account(
    app: AppHandle,
    name: String,
    provider: String,
    api_key: String,
    default_model: Option<String>,
    base_url: Option<String>,
) -> Result<Vec<UnifiedAccountSummary>, String> {
    if provider != "zai" && provider != "bigmodel" {
        return Err("Desteklenmeyen sağlayıcı".into());
    }
    let key = api_key.trim().to_string();
    if key.len() < 8 {
        return Err("API Key çok kısa".into());
    }
    if let Some(m) = &default_model {
        if !m.trim().is_empty() {
            check_model(m.trim())?;
        }
    }
    if let Some(u) = base_url.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if !u.starts_with("http://") && !u.starts_with("https://") {
            return Err("Base URL http:// veya https:// ile başlamalı".into());
        }
    }

    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let display_name = {
        let vault = state.vault.lock().unwrap();
        if name.trim().is_empty() {
            format!(
                "{} API {}",
                if provider == "zai" { "Z.AI" } else { "BigModel" },
                vault.accounts.len() + 1
            )
        } else {
            name.trim().to_string()
        }
    };

    let mut rec = UnifiedAccount::from_z_api_key(
        &display_name,
        &provider,
        &key,
        base_url.as_deref(),
        &vault_key,
    )?;
    rec.default_plan = Some("coding-plan".into());
    rec.default_model = default_model
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let new_id = rec.id.clone();
    {
        let mut vault = state.vault.lock().unwrap();
        vault.accounts.push(rec);
        let mut s = state.settings.lock().unwrap();
        if s.zcode_service_account_id.is_none() {
            s.zcode_service_account_id = Some(new_id);
        }
        vault.save(&state.paths.unified_vault_file())?;
        s.save(&state.paths)?;
    }
    let settings = state.settings.lock().unwrap().clone();
    let summaries = state.vault.lock().unwrap().summaries(&settings);
    emit_proxy_state(&app);
    Ok(summaries)
}

#[tauri::command]
pub fn add_custom_provider(
    app: AppHandle,
    name: String,
    provider_slug: String,
    package: String,
    base_url: String,
    api_key: String,
    default_model: Option<String>,
    models: Vec<CustomModelDef>,
    sync_to_opencode: bool,
) -> Result<Vec<UnifiedAccountSummary>, String> {
    let clean_url = base_url.trim();
    if !clean_url.starts_with("http://") && !clean_url.starts_with("https://") {
        return Err("Base URL http:// veya https:// ile başlamalı".into());
    }
    if let Some(m) = default_model.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        check_model(m)?;
    }
    for m in &models {
        if !m.id.trim().is_empty() {
            check_model(m.id.trim())?;
        }
    }

    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let rec = UnifiedAccount::from_custom_provider(
        &name,
        &provider_slug,
        &package,
        clean_url,
        &api_key,
        default_model.as_deref(),
        models,
        &vault_key,
    )?;

    if sync_to_opencode {
        if let Ok(oc_paths) = OpenCodePaths::live() {
            let _ = opencode::upsert_provider_in(
                &oc_paths,
                &rec.provider,
                rec.package
                    .as_deref()
                    .unwrap_or("@opencode/ai/providers/openai-compatible"),
                &rec.name,
                rec.base_url.as_deref().unwrap_or(clean_url),
                api_key.trim(),
                &rec.models,
                rec.default_model.as_deref(),
            );
        }
    }

    {
        let mut vault = state.vault.lock().unwrap();
        vault.accounts.push(rec);
        vault.save(&state.paths.unified_vault_file())?;
    }
    let settings = state.settings.lock().unwrap().clone();
    let res = state.vault.lock().unwrap().summaries(&settings);
    Ok(res)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn update_unified_account(
    app: AppHandle,
    id: String,
    name: Option<String>,
    provider_slug: Option<String>,
    package: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    default_plan: Option<String>,
    default_model: Option<String>,
    models: Option<Vec<CustomModelDef>>,
) -> Result<Vec<UnifiedAccountSummary>, String> {
    if let Some(m) = default_model.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        check_model(m)?;
    }
    if let Some(ms) = &models {
        for m in ms {
            if !m.id.trim().is_empty() {
                check_model(m.id.trim())?;
            }
        }
    }

    let state = app.state::<AppState>();
    let vault_key = state.vault_key();

    let updated_rec = {
        let mut vault = state.vault.lock().unwrap();
        let rec = vault
            .find_mut(&id)
            .ok_or_else(|| "Hesap bulunamadı".to_string())?;

        if let Some(n) = name {
            if !n.trim().is_empty() {
                rec.name = n.trim().to_string();
            }
        }
        if let Some(p) = default_plan {
            if !p.is_empty() {
                if rec.kind == "zcode_oauth" && (p == "coding-plan" || p == "start-plan") {
                    rec.default_plan = Some(p);
                } else if p == "coding-plan" {
                    rec.default_plan = Some(p);
                }
            }
        }
        if let Some(m) = default_model {
            let trimmed = m.trim();
            rec.default_model = (!trimmed.is_empty()).then(|| trimmed.to_string());
        }
        if let Some(u) = base_url {
            let trimmed = u.trim().trim_end_matches('/').to_string();
            if !trimmed.is_empty() {
                if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
                    return Err("Base URL http:// veya https:// ile başlamalı".into());
                }
                rec.base_url = Some(trimmed);
            } else if rec.kind == "z_api" {
                rec.base_url = None;
            }
        }
        if rec.kind == "custom_provider" {
            if let Some(slug) = provider_slug {
                let s = crate::unified_vault::sanitize_provider_slug(&slug);
                if !s.is_empty() {
                    rec.provider = s;
                }
            }
            if let Some(pkg) = package {
                if !pkg.trim().is_empty() {
                    rec.package = Some(pkg.trim().to_string());
                }
            }
            if let Some(ms) = models {
                rec.models = ms
                    .into_iter()
                    .filter(|m| !m.id.trim().is_empty())
                    .map(|m| CustomModelDef {
                        id: m.id.trim().to_string(),
                        name: if m.name.trim().is_empty() {
                            m.id.trim().to_string()
                        } else {
                            m.name.trim().to_string()
                        },
                        context_limit: m.context_limit,
                        output_limit: m.output_limit,
                    })
                    .collect();
            }
            rec.user_hint = rec.default_model.clone();
        }

        let mut payload = rec.decrypt_payload(&vault_key)?;
        if let Some(k) = api_key.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("apiKey".into(), json!(k));
                if rec.kind == "z_api" {
                    if let Some((kid, ksec)) = k.split_once('.') {
                        if !kid.is_empty() && !ksec.is_empty() {
                            obj.insert("secret".into(), json!(ksec));
                        }
                    } else {
                        obj.remove("secret");
                    }
                }
            }
            rec.key_hint = harness::mask_secret(k);
            rec.has_secret = true;
        }
        if let Some(u) = &rec.base_url {
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("baseURL".into(), json!(u));
                if rec.kind == "z_api" {
                    obj.insert("baseUrl".into(), json!(u));
                }
            }
        }
        rec.reencrypt_payload(&payload, &vault_key)?;
        let clone = rec.clone();
        vault.save(&state.paths.unified_vault_file())?;
        clone
    };

    // Auto re-sync whichever harness currently has this account active.
    let (active_in_opencode, active_in_claude, active_in_zcode) = {
        let s = state.settings.lock().unwrap();
        (
            s.opencode_active_session_id.as_deref() == Some(id.as_str()),
            s.claude_active_session_id.as_deref() == Some(id.as_str()),
            s.zcode_app_active_account_id.as_deref() == Some(id.as_str()),
        )
    };

    if matches!(updated_rec.kind.as_str(), "custom_provider" | "z_api") {
        if let Ok(oc_paths) = OpenCodePaths::live() {
            let slug = opencode::provider_slug_for_account(&updated_rec);
            let live_oc = opencode::inspect_live_at(&oc_paths);
            let exists_in_oc = live_oc.providers.iter().any(|p| p.id == slug);
            if active_in_opencode || exists_in_oc {
                let chosen = updated_rec
                    .default_model
                    .as_deref()
                    .or_else(|| updated_rec.models.first().map(|m| m.id.as_str()));
                let _ = opencode::apply_unified_account_at(
                    &oc_paths,
                    &updated_rec,
                    &vault_key,
                    None,
                    chosen,
                    active_in_opencode,
                );
                if active_in_opencode {
                    let mut s = state.settings.lock().unwrap();
                    s.opencode_active_model = chosen.map(String::from);
                    let _ = s.save(&state.paths);
                }
            }
        }
    }

    if active_in_claude && matches!(updated_rec.kind.as_str(), "custom_provider" | "z_api") {
        if let Ok(cl_paths) = ClaudePaths::live() {
            let api_key = updated_rec.extract_api_key(&vault_key).unwrap_or_default();
            let chosen_model = updated_rec
                .default_model
                .clone()
                .or_else(|| updated_rec.models.first().map(|m| m.id.clone()));
            if updated_rec.kind == "z_api" {
                if let Ok(base) = claude_harness::resolve_z_api_base(
                    &updated_rec.provider,
                    updated_rec.base_url.as_deref(),
                ) {
                    let eff = chosen_model.as_deref().unwrap_or("glm-5.3");
                    let _ = claude_harness::apply_env_bridge_at(
                        &cl_paths,
                        &base,
                        &api_key,
                        Some(eff),
                    );
                }
            } else if let Some(base_url) = updated_rec
                .base_url
                .as_deref()
                .filter(|s| !s.trim().is_empty())
            {
                let pkg = updated_rec.package.as_deref().unwrap_or("");
                let allowed: Vec<String> =
                    updated_rec.models.iter().map(|m| m.id.clone()).collect();
                if pkg.contains("anthropic") {
                    let _ = claude_harness::apply_env_bridge_at(
                        &cl_paths,
                        base_url.trim().trim_end_matches('/'),
                        if api_key.is_empty() { "custom" } else { &api_key },
                        chosen_model.as_deref(),
                    );
                } else if let Ok((bridge_url, bridge_key)) = state.openai_bridge.configure(
                    BridgeTarget {
                        base_url: base_url.to_string(),
                        api_key,
                        default_model: chosen_model.clone(),
                        allowed_models: allowed,
                        local_api_key: String::new(),
                    },
                ) {
                    let _ = claude_harness::apply_env_bridge_at(
                        &cl_paths,
                        &bridge_url,
                        &bridge_key,
                        chosen_model.as_deref(),
                    );
                }
            }
            let mut s = state.settings.lock().unwrap();
            s.claude_active_model = chosen_model;
            let _ = s.save(&state.paths);
        }
    }

    if active_in_zcode && updated_rec.kind == "z_api" {
        if let Ok(zc_paths) = ZCodePaths::live() {
            let _ = zcode_harness::activate_zcode_account_at(&zc_paths, &updated_rec, &vault_key);
        }
    }

    let settings = state.settings.lock().unwrap().clone();
    emit_proxy_state(&app);
    let res = state.vault.lock().unwrap().summaries(&settings);
    Ok(res)
}

#[tauri::command]
pub fn get_account_secret(app: AppHandle, id: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let vault = state.vault.lock().unwrap();
    let rec = vault
        .find(&id)
        .ok_or_else(|| "Hesap bulunamadı".to_string())?;
    rec.extract_api_key(&vault_key)
}

#[tauri::command]
pub async fn set_account_enabled(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<Vec<UnifiedAccountSummary>, String> {
    let state = app.state::<AppState>();
    let (was_service, was_claude, was_opencode, was_zcode, was_agy) = {
        let s = state.settings.lock().unwrap();
        let mut vault = state.vault.lock().unwrap();
        let rec = vault
            .find_mut(&id)
            .ok_or_else(|| "Hesap bulunamadı".to_string())?;
        rec.enabled = enabled;
        vault.save(&state.paths.unified_vault_file())?;
        (
            s.zcode_service_account_id.as_deref() == Some(id.as_str()),
            s.claude_active_session_id.as_deref() == Some(id.as_str()),
            s.opencode_active_session_id.as_deref() == Some(id.as_str()),
            s.zcode_app_active_account_id.as_deref() == Some(id.as_str()),
            s.antigravity_active_session_id.as_deref() == Some(id.as_str()),
        )
    };

    if !enabled {
        if was_service {
            let _ = stop_proxy_process(&app).await;
            credstore::clear_credential(&state.paths.proxy_store_dir());
            let mut s = state.settings.lock().unwrap();
            s.zcode_service_account_id = None;
            let _ = s.save(&state.paths);
        }
        if was_claude {
            let _ = disable_harness(app.clone(), "claude".into()).await;
        }
        if was_opencode {
            let _ = disable_harness(app.clone(), "opencode".into()).await;
        }
        if was_zcode {
            let _ = disable_harness(app.clone(), "zcode".into()).await;
        }
        if was_agy {
            let _ = disable_harness(app.clone(), "antigravity".into()).await;
        }
    }

    let settings = state.settings.lock().unwrap().clone();
    emit_proxy_state(&app);
    let res = state.vault.lock().unwrap().summaries(&settings);
    Ok(res)
}

#[tauri::command]
pub async fn delete_account(app: AppHandle, id: String) -> Result<Vec<UnifiedAccountSummary>, String> {
    let state = app.state::<AppState>();
    let was_service = {
        let s = state.settings.lock().unwrap();
        s.zcode_service_account_id.as_deref() == Some(id.as_str())
    };
    if was_service {
        let _ = stop_proxy_process(&app).await;
        credstore::clear_credential(&state.paths.proxy_store_dir());
    }

    {
        let mut vault = state.vault.lock().unwrap();
        vault.accounts.retain(|a| a.id != id);
    }
    save_vault_and_settings(&state)?;

    let settings = state.settings.lock().unwrap().clone();
    emit_proxy_state(&app);
    let res = state.vault.lock().unwrap().summaries(&settings);
    Ok(res)
}

#[tauri::command]
pub async fn test_account(app: AppHandle, id: String) -> Result<TestResult, String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let rec = {
        let vault = state.vault.lock().unwrap();
        vault
            .find(&id)
            .cloned()
            .ok_or_else(|| "Hesap bulunamadı".to_string())?
    };

    match rec.kind.as_str() {
        "zcode_oauth" | "z_api" => {
            let cred = rec.extract_zcode_credential(&vault_key)?;
            let api_key = cred.get("apiKey").and_then(|v| v.as_str()).unwrap_or("");
            let joined = match cred.get("secret").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => format!("{api_key}.{s}"),
                _ => api_key.to_string(),
            };
            if joined.is_empty() {
                return Ok(TestResult {
                    ok: false,
                    message: "Hesapta test edilecek API anahtarı bulunamadı".into(),
                    latency_ms: None,
                });
            }
            let url = if rec.provider == "bigmodel" {
                "https://open.bigmodel.cn/api/paas/v4/models".to_string()
            } else {
                "https://api.z.ai/api/paas/v4/models".to_string()
            };
            let started = std::time::Instant::now();
            let res = tauri::async_runtime::spawn_blocking(move || {
                http::get(&url, &[("Authorization", format!("Bearer {joined}").as_str())], 20)
            })
            .await
            .map_err(|e| e.to_string())?;
            let latency = started.elapsed().as_millis() as u64;
            match res {
                Ok(r) if r.status == 200 => Ok(TestResult {
                    ok: true,
                    message: format!("Bağlantı başarılı ({latency} ms)"),
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
        "custom_provider" => {
            let key = rec.extract_api_key(&vault_key).unwrap_or_default();
            let base = rec
                .base_url
                .as_deref()
                .ok_or("Base URL tanımlı değil")?
                .trim_end_matches('/');
            let url = if rec.package.as_deref().unwrap_or("").contains("anthropic") {
                if base.ends_with("/v1") {
                    format!("{base}/models")
                } else {
                    format!("{base}/v1/models")
                }
            } else if base.ends_with("/v1") {
                format!("{base}/models")
            } else {
                format!("{base}/v1/models")
            };
            let started = std::time::Instant::now();
            let res = tauri::async_runtime::spawn_blocking(move || {
                let auth = format!("Bearer {key}");
                let headers: Vec<(&str, &str)> = if key.is_empty() {
                    Vec::new()
                } else {
                    vec![("Authorization", auth.as_str()), ("x-api-key", key.as_str())]
                };
                http::get(&url, &headers, 15)
            })
            .await
            .map_err(|e| e.to_string())?;
            let latency = started.elapsed().as_millis() as u64;
            match res {
                Ok(r) if (200..300).contains(&r.status) => Ok(TestResult {
                    ok: true,
                    message: format!("Özel uç nokta yanıt verdi ({latency} ms)"),
                    latency_ms: Some(latency),
                }),
                Ok(r) => Ok(TestResult {
                    ok: false,
                    message: format!("HTTP {} yanıtı döndü ({latency} ms)", r.status),
                    latency_ms: Some(latency),
                }),
                Err(e) => Ok(TestResult {
                    ok: false,
                    message: format!("Uç noktaya ulaşılamadı: {e}"),
                    latency_ms: None,
                }),
            }
        }
        _ => Ok(TestResult {
            ok: true,
            message: "OAuth oturumu kasada şifreli olarak hazır".into(),
            latency_ms: None,
        }),
    }
}

#[tauri::command]
pub fn import_system_accounts(app: AppHandle) -> Result<ImportResultDto, String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let mut details = Vec::new();
    let mut total = 0usize;

    {
        let mut vault = state.vault.lock().unwrap();

        // 1. OpenCode providers (opencode.jsonc)
        if let Ok(oc_paths) = OpenCodePaths::live() {
            if let Ok(n) = opencode::import_providers_into_vault(&oc_paths, &mut vault, &vault_key) {
                if n > 0 {
                    total += n;
                    details.push(format!("OpenCode: {n} özel model/sağlayıcı içe aktarıldı"));
                }
            }
        }

        // 2. ZCode Desktop personal API key rules (~/.zcode/v2/provider_config.json)
        if let Ok(zc_paths) = ZCodePaths::live() {
            if let Ok(n) =
                zcode_harness::import_personal_rules_into_vault(&zc_paths, &mut vault, &vault_key)
            {
                if n > 0 {
                    total += n;
                    details.push(format!("ZCode: {n} API anahtarı içe aktarıldı"));
                }
            }
        }

        // 3. Claude Code active OAuth (~/.claude/.credentials.json)
        if let Ok(cl_paths) = ClaudePaths::live() {
            if let Ok(rec) =
                claude_harness::capture_unified_claude_oauth_at(&cl_paths, &vault_key, None)
            {
                if let Ok(payload) = rec.decrypt_payload(&vault_key) {
                    let dup = vault
                        .accounts
                        .iter()
                        .filter(|a| a.kind == "claude_oauth")
                        .any(|a| a.decrypt_payload(&vault_key).ok().as_ref() == Some(&payload));
                    if !dup {
                        vault.accounts.push(rec);
                        total += 1;
                        details.push("Claude Code: aktif OAuth oturumu içe aktarıldı".into());
                    }
                }
            }
        }

        // 4. Antigravity CLI (`agy`) active OAuth
        if let Ok(rec) = antigravity::capture_unified_current(&vault_key, None) {
            if let Ok(payload) = rec.decrypt_payload(&vault_key) {
                let dup = vault
                    .accounts
                    .iter()
                    .filter(|a| a.kind == "antigravity_oauth")
                    .any(|a| {
                        a.decrypt_payload(&vault_key)
                            .ok()
                            .map(|p| {
                                p.get("refresh_token") == payload.get("refresh_token")
                                    || p == payload
                            })
                            .unwrap_or(false)
                    });
                if !dup {
                    vault.accounts.push(rec);
                    total += 1;
                    details.push("Antigravity CLI: aktif Google oturumu içe aktarıldı".into());
                }
            }
        }

        // 5. ZCode Desktop OAuth (`~/.zcode/v2/credentials.json`)
        let has_zcode_oauth = vault.accounts.iter().any(|a| a.kind == "zcode_oauth");
        if !has_zcode_oauth {
            if let Ok(zc_paths) = ZCodePaths::live() {
                if zc_paths.v2_dir.join("credentials.json").is_file() {
                    let tmp_store = state
                        .paths
                        .tmp_dir()
                        .join(format!("zcode-import-{}", uuid::Uuid::new_v4().simple()));
                    if std::fs::create_dir_all(&tmp_store).is_ok() {
                        let exe = state.paths.proxy_exe();
                        let _ = proxy::ensure_proxy_config(&state.paths);
                        let mut cmd = std::process::Command::new(&exe);
                        cmd.args(["--cli", "auth", "login", "zai", "--import"])
                            .env("ZCODE_PROXY_CONFIG", state.paths.proxy_config())
                            .env("ZCODE_PROXY_STORE_DIR", &tmp_store)
                            .env("ZCODE_MASTER_SECRET", &state.secret)
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null());
                        #[cfg(windows)]
                        {
                            use std::os::windows::process::CommandExt;
                            cmd.creation_flags(0x0800_0000);
                        }
                        if cmd.status().map(|s| s.success()).unwrap_or(false) {
                            if let Ok(Some(cred)) =
                                credstore::read_credential(&tmp_store, &state.secret)
                            {
                                if let Ok(rec) = UnifiedAccount::from_zcode_credential(
                                    "ZCode OAuth (Sistem)",
                                    "zcode_oauth",
                                    &cred,
                                    None,
                                    &vault_key,
                                ) {
                                    vault.accounts.push(rec);
                                    total += 1;
                                    details.push("ZCode: aktif OAuth hesabı içe aktarıldı".into());
                                }
                            }
                        }
                        let _ = std::fs::remove_dir_all(&tmp_store);
                    }
                }
            }
        }
    }

    save_vault_and_settings(&state)?;
    emit_proxy_state(&app);
    Ok(ImportResultDto {
        imported: total,
        details,
    })
}

// ---------------------------------------------------------------------------
// OAuth Capture / Login Flows
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn start_oauth(app: AppHandle, provider: String, name: Option<String>) -> Result<(), String> {
    oauth::start(&app, &provider, name)
}

#[tauri::command]
pub fn cancel_oauth(app: AppHandle) {
    oauth::cancel(&app);
}

#[tauri::command]
pub fn start_claude_oauth(app: AppHandle, name: Option<String>) -> Result<(), String> {
    claude_harness::start_oauth_login(&app, name)
}

#[tauri::command]
pub fn capture_claude_oauth(app: AppHandle, name: Option<String>) -> Result<UnifiedAccountSummary, String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let paths = ClaudePaths::live()?;
    let rec = claude_harness::capture_unified_claude_oauth_at(
        &paths,
        &vault_key,
        name.as_deref().filter(|s| !s.trim().is_empty()),
    )?;
    let id = rec.id.clone();
    {
        let mut vault = state.vault.lock().unwrap();
        vault.accounts.push(rec);
        vault.save(&state.paths.unified_vault_file())?;
    }
    let settings = state.settings.lock().unwrap().clone();
    let res = state
        .vault
        .lock()
        .unwrap()
        .find(&id)
        .map(|a| a.summary(&settings))
        .ok_or_else(|| "Kayıt bulunamadı".into());
    res
}

#[tauri::command]
pub fn start_agy_oauth(app: AppHandle, name: Option<String>) -> Result<(), String> {
    antigravity::start_oauth_login(&app, name)
}

#[tauri::command]
pub fn capture_agy_oauth(app: AppHandle, name: Option<String>) -> Result<UnifiedAccountSummary, String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let rec = antigravity::capture_unified_current(
        &vault_key,
        name.as_deref().filter(|s| !s.trim().is_empty()),
    )?;
    let id = rec.id.clone();
    {
        let mut vault = state.vault.lock().unwrap();
        vault.accounts.push(rec);
        vault.save(&state.paths.unified_vault_file())?;
    }
    let settings = state.settings.lock().unwrap().clone();
    let res = state
        .vault
        .lock()
        .unwrap()
        .find(&id)
        .map(|a| a.summary(&settings))
        .ok_or_else(|| "Kayıt bulunamadı".into());
    res
}

#[tauri::command]
pub fn cancel_harness_oauth(app: AppHandle) {
    let (pid, temp_dir) = {
        let state = app.state::<AppState>();
        let mut flow = state.harness_oauth.lock().unwrap();
        flow.running = false;
        (flow.child_pid.take(), flow.temp_dir.take())
    };
    if let Some(pid) = pid {
        proxy::kill_pid(pid);
    }
    if let Some(dir) = temp_dir {
        let _ = std::fs::remove_dir_all(dir);
    }
}

// ---------------------------------------------------------------------------
// One-Click Harness Activation, Disable, OpenCode Config, & Launch
// ---------------------------------------------------------------------------

async fn ensure_zcode_service_ready(
    app: &AppHandle,
    account_id: &str,
) -> Result<(String, String), String> {
    let state = app.state::<AppState>();
    let need_switch_or_start = {
        let s = state.settings.lock().unwrap().clone();
        let vault = state.vault.lock().unwrap();
        let rec = vault
            .find(account_id)
            .ok_or_else(|| "ZCode hesabı bulunamadı".to_string())?;
        if !rec.enabled {
            return Err("Seçilen hesap devre dışı".into());
        }
        if rec.kind != "zcode_oauth" {
            return Err("ZCode Proxy yalnızca ZCode OAuth abonelik hesapları içindir".into());
        }
        let st = build_status(app);
        s.zcode_service_account_id.as_deref() != Some(account_id)
            || !st.status.process_alive
            || !st.status.server_running
    };

    if need_switch_or_start {
        activate_account(app.clone(), account_id.to_string()).await?;
    }

    let cfg = proxy::read_proxy_config(&state.paths)?;
    let port = cfg
        .pointer("/server/port")
        .and_then(|v| v.as_u64())
        .unwrap_or_default() as u16;
    if port == 0 {
        return Err("ZCode servis portu yapılandırılmamış".into());
    }
    let key = cfg
        .pointer("/auth/proxyApiKey")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    Ok((format!("http://127.0.0.1:{port}"), key))
}

#[tauri::command]
pub async fn activate_for_harness(
    app: AppHandle,
    harness: String,
    account_id: String,
    model: Option<String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let rec = {
        let vault = state.vault.lock().unwrap();
        vault
            .find(&account_id)
            .cloned()
            .ok_or_else(|| "Hesap bulunamadı".to_string())?
    };
    if !rec.enabled {
        return Err("Seçilen hesap devre dışı — önce Hesaplar sekmesinden etkinleştirin.".into());
    }

    let clean_model = model
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from);
    if let Some(m) = &clean_model {
        check_model(m)?;
    }

    match harness.as_str() {
        "claude" => {
            if !rec.supports("claude") {
                return Err(format!("{} hesabı Claude Code ile kullanılamaz", rec.name));
            }
            let cl_paths = ClaudePaths::live()?;

            // Auto-backup live Claude OAuth credentials into the vault before switching away,
            // so switching back to Claude OAuth later never loses the active session.
            if rec.kind != "claude_oauth" && cl_paths.credentials_json.is_file() {
                if let Ok(backup) = claude_harness::capture_unified_claude_oauth_at(
                    &cl_paths,
                    &vault_key,
                    None,
                ) {
                    if let Ok(b_payload) = backup.decrypt_payload(&vault_key) {
                        let mut vault = state.vault.lock().unwrap();
                        let dup = vault
                            .accounts
                            .iter()
                            .filter(|a| a.kind == "claude_oauth")
                            .any(|a| {
                                a.decrypt_payload(&vault_key)
                                    .ok()
                                    .as_ref()
                                    .and_then(|p| p.get("credentials"))
                                    == b_payload.get("credentials")
                            });
                        if !dup {
                            vault.accounts.push(backup);
                            let _ = vault.save(&state.paths.unified_vault_file());
                        }
                    }
                }
            }

            let payload = rec.decrypt_payload(&vault_key)?;
            let chosen_model = clean_model.clone().or_else(|| rec.default_model.clone());

            match rec.kind.as_str() {
                "claude_oauth" => {
                    claude_harness::apply_claude_oauth_at(&cl_paths, &payload)?;
                }
                "zcode_oauth" => {
                    let (endpoint, key) = ensure_zcode_service_ready(&app, &rec.id).await?;
                    let eff_model = chosen_model.as_deref().unwrap_or("glm-5.3");
                    claude_harness::apply_env_bridge_at(&cl_paths, &endpoint, &key, Some(eff_model))?;
                }
                "z_api" => {
                    let api_key = rec.extract_api_key(&vault_key)?;
                    let base = claude_harness::resolve_z_api_base(
                        &rec.provider,
                        rec.base_url.as_deref(),
                    )?;
                    let eff_model = chosen_model.as_deref().unwrap_or("glm-5.3");
                    claude_harness::apply_env_bridge_at(&cl_paths, &base, &api_key, Some(eff_model))?;
                }
                "custom_provider" => {
                    let api_key = rec.extract_api_key(&vault_key)?;
                    let base_url = rec
                        .base_url
                        .as_deref()
                        .filter(|s| !s.trim().is_empty())
                        .ok_or("Özel sağlayıcıda Base URL eksik")?;
                    let pkg = rec.package.as_deref().unwrap_or("");
                    let allowed_models: Vec<String> =
                        rec.models.iter().map(|m| m.id.clone()).collect();
                    let eff_model = chosen_model
                        .clone()
                        .or_else(|| allowed_models.first().cloned());

                    if pkg.contains("anthropic") {
                        let base = base_url.trim().trim_end_matches('/');
                        claude_harness::apply_env_bridge_at(
                            &cl_paths,
                            base,
                            if api_key.is_empty() { "custom" } else { &api_key },
                            eff_model.as_deref(),
                        )?;
                    } else {
                        let (bridge_url, bridge_key) = state.openai_bridge.configure(BridgeTarget {
                            base_url: base_url.to_string(),
                            api_key,
                            default_model: eff_model.clone(),
                            allowed_models,
                            local_api_key: String::new(),
                        })?;
                        claude_harness::apply_env_bridge_at(
                            &cl_paths,
                            &bridge_url,
                            &bridge_key,
                            eff_model.as_deref(),
                        )?;
                    }
                }
                _ => return Err("Desteklenmeyen hesap türü".into()),
            }

            let mut s = state.settings.lock().unwrap();
            s.claude_active_method = rec.kind.clone();
            s.claude_active_session_id = Some(rec.id.clone());
            s.claude_active_model = chosen_model;
            s.save(&state.paths)?;
        }
        "opencode" => {
            if !rec.supports("opencode") {
                return Err(format!("{} hesabı OpenCode ile kullanılamaz", rec.name));
            }
            let oc_paths = OpenCodePaths::live()?;
            let zcode_info = if rec.kind == "zcode_oauth" {
                Some(ensure_zcode_service_ready(&app, &rec.id).await?)
            } else {
                None
            };
            opencode::apply_unified_account_at(
                &oc_paths,
                &rec,
                &vault_key,
                zcode_info.as_ref().map(|(e, k)| (e.as_str(), k.as_str())),
                clean_model.as_deref(),
                true,
            )?;

            let chosen = clean_model
                .or_else(|| rec.default_model.clone())
                .or_else(|| rec.models.first().map(|m| m.id.clone()));
            let mut s = state.settings.lock().unwrap();
            s.opencode_active_method = rec.kind.clone();
            s.opencode_active_session_id = Some(rec.id.clone());
            s.opencode_active_model = chosen;
            s.save(&state.paths)?;
        }
        "zcode" => {
            if !rec.supports("zcode") {
                return Err(format!("{} hesabı ZCode Desktop ile kullanılamaz", rec.name));
            }
            let zc_paths = ZCodePaths::live()?;
            zcode_harness::activate_zcode_account_at(&zc_paths, &rec, &vault_key)?;
            let mut s = state.settings.lock().unwrap();
            s.zcode_app_active_method = rec.kind.clone();
            s.zcode_app_active_account_id = Some(rec.id.clone());
            s.save(&state.paths)?;
        }
        "antigravity" => {
            if !rec.supports("antigravity") {
                return Err(format!("{} hesabı Antigravity CLI ile kullanılamaz", rec.name));
            }
            antigravity::activate_unified(&rec, &vault_key)?;
            let mut s = state.settings.lock().unwrap();
            s.antigravity_active_session_id = Some(rec.id.clone());
            s.save(&state.paths)?;
        }
        other => return Err(format!("Bilinmeyen araç: {other}")),
    }

    {
        let mut vault = state.vault.lock().unwrap();
        if let Some(r) = vault.find_mut(&account_id) {
            r.last_used_at = Some(now_ms());
        }
        let _ = vault.save(&state.paths.unified_vault_file());
    }
    emit_proxy_state(&app);
    Ok(())
}

#[tauri::command]
pub async fn disable_harness(app: AppHandle, harness: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();

    match harness.as_str() {
        "claude" => {
            let cl_paths = ClaudePaths::live()?;
            let clear_oauth = {
                let s = state.settings.lock().unwrap();
                s.claude_active_method == "claude_oauth" || s.claude_active_method == "disabled"
            };
            let backup = {
                let vault = state.vault.lock().unwrap();
                claude_harness::disable_unified_at(&cl_paths, clear_oauth, &vault, &vault_key)?
            };
            if let Some(rec) = backup {
                let mut vault = state.vault.lock().unwrap();
                vault.accounts.push(rec);
                vault.save(&state.paths.unified_vault_file())?;
            }
            let mut s = state.settings.lock().unwrap();
            s.claude_active_method = "disabled".into();
            s.claude_active_session_id = None;
            s.claude_active_model = None;
            s.save(&state.paths)?;
        }
        "opencode" => {
            let oc_paths = OpenCodePaths::live()?;
            opencode::disable_at(&oc_paths)?;
            let mut s = state.settings.lock().unwrap();
            s.opencode_active_method = "disabled".into();
            s.opencode_active_session_id = None;
            s.opencode_active_model = None;
            s.save(&state.paths)?;
        }
        "zcode" => {
            let zc_paths = ZCodePaths::live()?;
            zcode_harness::disable_at(&zc_paths)?;
            let mut s = state.settings.lock().unwrap();
            s.zcode_app_active_method = "disabled".into();
            s.zcode_app_active_account_id = None;
            s.save(&state.paths)?;
        }
        "antigravity" => {
            let backup = {
                let vault = state.vault.lock().unwrap();
                antigravity::disable_unified(&vault, &vault_key)?
            };
            if let Some(rec) = backup {
                let mut vault = state.vault.lock().unwrap();
                vault.accounts.push(rec);
                vault.save(&state.paths.unified_vault_file())?;
            }
            let mut s = state.settings.lock().unwrap();
            s.antigravity_active_session_id = None;
            s.save(&state.paths)?;
        }
        other => return Err(format!("Bilinmeyen araç: {other}")),
    }

    emit_proxy_state(&app);
    Ok(())
}

#[tauri::command]
pub async fn launch_harness(
    app: AppHandle,
    harness: String,
    folder: Option<String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let launches = state.paths.launches_dir();

    if harness == "zcode" {
        return tauri::async_runtime::spawn_blocking(zcode_harness::launch_zcode_app)
            .await
            .map_err(|e| e.to_string())?;
    }

    let dir = resolve_folder(&app, folder)?;
    tauri::async_runtime::spawn_blocking(move || match harness.as_str() {
        "claude" => claude_harness::launch_claude_folder(&launches, &dir),
        "opencode" => opencode::launch_opencode_folder(&launches, &dir),
        "antigravity" => antigravity::launch_agy_folder(&launches, &dir),
        other => Err(format!("Bilinmeyen araç: {other}")),
    })
    .await
    .map_err(|e| e.to_string())?
}

fn resolve_folder(app: &AppHandle, folder: Option<String>) -> Result<std::path::PathBuf, String> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().clone();
    let chosen = folder
        .filter(|f| !f.trim().is_empty())
        .or_else(|| settings.claude_default_folder.clone())
        .or_else(|| settings.recent_folders.first().cloned())
        .or_else(|| std::env::var("USERPROFILE").ok())
        .ok_or_else(|| "Başlatılacak klasör seçilmedi".to_string())?;
    let p = std::path::PathBuf::from(&chosen);
    if !p.is_dir() {
        return Err(format!("Klasör bulunamadı: {}", p.display()));
    }
    Ok(p)
}

#[tauri::command]
pub fn opencode_set_live_model(model_ref: String) -> Result<(), String> {
    let paths = OpenCodePaths::live()?;
    opencode::set_active_model_in(&paths, &model_ref)
}

#[tauri::command]
pub fn opencode_upsert_live_provider(
    slug: String,
    package: String,
    name: String,
    base_url: String,
    api_key: String,
    models: Vec<CustomModelDef>,
    set_default_model: Option<String>,
) -> Result<(), String> {
    let clean_slug = crate::unified_vault::sanitize_provider_slug(&slug);
    if clean_slug.is_empty() {
        return Err("Sağlayıcı kimliği boş olamaz".into());
    }
    let paths = OpenCodePaths::live()?;
    opencode::upsert_provider_in(
        &paths,
        &clean_slug,
        &package,
        &name,
        &base_url,
        &api_key,
        &models,
        set_default_model.as_deref(),
    )
}

#[tauri::command]
pub fn opencode_remove_live_provider(slug: String) -> Result<(), String> {
    let paths = OpenCodePaths::live()?;
    opencode::remove_provider_in(&paths, &slug)
}

#[tauri::command]
pub async fn opencode_toggle_account(
    app: AppHandle,
    account_id: String,
    enabled: bool,
    model: Option<String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();
    let rec = {
        let vault = state.vault.lock().unwrap();
        vault
            .find(&account_id)
            .cloned()
            .ok_or_else(|| "Hesap bulunamadı".to_string())?
    };
    if !rec.supports("opencode") {
        return Err(format!("{} hesabı OpenCode ile kullanılamaz", rec.name));
    }
    let oc_paths = OpenCodePaths::live()?;

    if enabled {
        if !rec.enabled {
            return Err("Seçilen hesap devre dışı — önce Hesaplar sekmesinden etkinleştirin.".into());
        }
        let clean_model = model
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);
        if let Some(m) = &clean_model {
            check_model(m)?;
        }
        let zcode_info = if rec.kind == "zcode_oauth" {
            Some(ensure_zcode_service_ready(&app, &rec.id).await?)
        } else {
            None
        };
        opencode::apply_unified_account_at(
            &oc_paths,
            &rec,
            &vault_key,
            zcode_info.as_ref().map(|(e, k)| (e.as_str(), k.as_str())),
            clean_model.as_deref(),
            false,
        )?;
    } else {
        opencode::remove_unified_account_at(&oc_paths, &rec)?;
        let mut s = state.settings.lock().unwrap();
        if s.opencode_active_session_id.as_deref() == Some(rec.id.as_str()) {
            s.opencode_active_session_id = None;
            s.opencode_active_model = None;
            s.opencode_active_method = "disabled".into();
            let _ = s.save(&state.paths);
        }
    }
    emit_proxy_state(&app);
    Ok(())
}

#[tauri::command]
pub async fn opencode_set_default_account(
    app: AppHandle,
    account_id: String,
    model: Option<String>,
) -> Result<(), String> {
    activate_for_harness(app, "opencode".into(), account_id, model).await
}

// ---------------------------------------------------------------------------
// ZCode Proxy Service Lifecycle, Quota, Logs & Config
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn activate_account(app: AppHandle, id: String) -> Result<ProxyStatusDto, String> {
    let state = app.state::<AppState>();
    let vault_key = state.vault_key();

    let (provider, plan, model) = {
        let settings = state.settings.lock().unwrap().clone();
        let mut vault = state.vault.lock().unwrap();
        let rec = vault
            .find(&id)
            .ok_or_else(|| "Hesap bulunamadı".to_string())?
            .clone();
        if !rec.enabled {
            return Err("Bu hesap devre dışı. Önce etkinleştirin.".into());
        }
        if rec.kind != "zcode_oauth" {
            return Err("ZCode Proxy yalnızca ZCode OAuth hesapları ile çalışır (API anahtarları doğrudan bağlanır)".into());
        }
        let _ = rec.extract_zcode_credential(&vault_key)?;
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
        if let Some(r) = vault.find_mut(&id) {
            r.last_used_at = Some(now_ms());
        }
        vault.save(&state.paths.unified_vault_file())?;
        (rec.provider, plan, rec.default_model)
    };

    {
        let mut s = state.settings.lock().unwrap();
        s.zcode_service_account_id = Some(id);
        s.save(&state.paths)?;
    }

    let mut updates = vec![
        yamlpatch::update("provider", provider),
        yamlpatch::update("plan", plan),
    ];
    if let Some(m) = &model {
        updates.push(yamlpatch::update("defaultModel", m.clone()));
    }
    proxy::patch_proxy_config(&state.paths, &updates)?;

    let alive = {
        let settings = state.settings.lock().unwrap().clone();
        let managed = state.proxy.lock().unwrap().clone();
        proxy::status(&state.paths, &settings, &managed).process_alive
    };
    if alive {
        stop_proxy_process(&app).await?;
    }
    start_proxy_process(&app).await?;
    emit_proxy_state(&app);
    Ok(build_status(&app))
}

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
    let exe = state.paths.proxy_exe();

    {
        let settings = state.settings.lock().unwrap().clone();
        if let Some(adopted) = proxy::adopt_proxy(&settings) {
            *state.proxy.lock().unwrap() = adopted;
            return Ok(());
        }
    }

    let (settings, cred_json) = {
        let vault_key = state.vault_key();
        let mut settings = state.settings.lock().unwrap();
        let vault = state.vault.lock().unwrap();

        let chosen = settings
            .zcode_service_account_id
            .as_deref()
            .and_then(|id| vault.find(id))
            .filter(|a| a.enabled && a.kind == "zcode_oauth")
            .or_else(|| vault.first_enabled_zcode_service())
            .ok_or_else(|| {
                "active_account_missing: Önce Hesaplar sekmesinden bir ZCode OAuth hesabı ekleyin/etkinleştirin"
                    .to_string()
            })?
            .clone();

        if settings.zcode_service_account_id.as_deref() != Some(chosen.id.as_str()) {
            settings.zcode_service_account_id = Some(chosen.id.clone());
            let _ = settings.save(&state.paths);
        }

        let cred = chosen
            .extract_zcode_credential(&vault_key)
            .map_err(|e| format!("hesap kimliği çözülemedi: {e}"))?;
        (
            settings.clone(),
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
        return Ok(json!({ "ok": false, "message": "ZCode servis portu bulunamadı" }));
    }
    let health_url = format!("http://127.0.0.1:{port}/health");
    let models_url = format!("http://127.0.0.1:{port}/v1/models");
    let (health, models) = tauri::async_runtime::spawn_blocking(move || {
        let h = http::get(&health_url, &[], 5);
        let m = http::get(&models_url, &[("x-api-key", key.as_str())], 8);
        (h, m)
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
            format!("Servis yanıt veriyor — {models_count} model listeleniyor")
        } else {
            "ZCode servisi yanıt vermiyor".into()
        }
    }))
}

#[tauri::command]
pub async fn fetch_quota(app: AppHandle) -> Result<Value, String> {
    let p = panel(&app)?;
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
    let lines = proxy::tail_file(&path, limit.clamp(50, 2000));
    Ok(lines
        .iter()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect())
}

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
            let managed = state.proxy.lock().unwrap().clone();
            if proxy::port_in_use(port) && managed.panel_port != port {
                return Err(format!("{port} portu başka bir uygulama tarafından kullanılıyor"));
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
                return Err("Proxy anahtarı en az 8 karakter olmalı".into());
            }
            yaml_updates.push(yamlpatch::update_under("auth", "proxyApiKey", format!("\"{k}\"")));
        }
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
        return Err("ZCode servisi kapalı — önce servisi başlatın".into());
    }
    {
        let st = build_status(&app);
        if !st.status.process_alive || !st.status.server_running {
            return Err("ZCode servisi çalışmıyor — önce servisi başlatın".into());
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
        http::post_json(&url, &[("x-api-key", key.as_str())], &body, 90)
    })
    .await
    .map_err(|e| e.to_string())?;
    let latency = started.elapsed().as_millis() as u64;

    match res {
        Ok(r) if r.status == 200 => Ok(TestResult {
            ok: true,
            message: format!("Model yanıt verdi ({latency} ms)"),
            latency_ms: Some(latency),
        }),
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
