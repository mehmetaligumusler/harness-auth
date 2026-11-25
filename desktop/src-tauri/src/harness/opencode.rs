//! OpenCode CLI (`opencode`) adapter for HarnessAuth v2.
//!
//! Manages `~/.config/opencode/opencode.jsonc` (and legacy `opencode.json`) +
//! `~/.local/share/opencode/auth.json`:
//! - JSONC parser (`parse_jsonc`) preserving user `skills`, `mcp`, `$schema`,
//!   and existing custom `providers`
//! - Live inspection of all configured providers (`providers` plural or legacy `provider`)
//! - One-click import of existing `opencode.jsonc` providers into `UnifiedVault`
//! - Live add / edit / delete / default-model selection for `custom_provider`
//! - One-click activation of `zcode_oauth` (via ZCode proxy) or `z_api`

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::harness::{atomic_write, detect_cli, launch_in_terminal, mask_secret, read_json_object, user_home, CliInfo};
use crate::unified_vault::{sanitize_provider_slug, CustomModelDef, UnifiedAccount, UnifiedVault};

#[derive(Debug, Clone)]
pub struct OpenCodePaths {
    pub auth_json: PathBuf,
    pub config_jsonc: PathBuf,
    pub config_json: PathBuf,
}

impl OpenCodePaths {
    pub fn live() -> Result<Self, String> {
        let home = user_home()?;
        Ok(Self::under(&home))
    }

    pub fn under(home: &Path) -> Self {
        let cfg_dir = home.join(".config").join("opencode");
        let data_home = std::env::var("XDG_DATA_HOME")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("share"));
        let auth_json = std::env::var("OPENCODE_AUTH_PATH")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| data_home.join("opencode").join("auth.json"));

        Self {
            auth_json,
            config_jsonc: cfg_dir.join("opencode.jsonc"),
            config_json: cfg_dir.join("opencode.json"),
        }
    }

    /// Resolve active config file path: `OPENCODE_CONFIG` -> existing `.jsonc`
    /// -> existing `.json` -> default `.jsonc`.
    pub fn active_config_path(&self) -> PathBuf {
        if let Ok(p) = std::env::var("OPENCODE_CONFIG") {
            let trimmed = p.trim();
            if !trimmed.is_empty() {
                return PathBuf::from(trimmed);
            }
        }
        if self.config_jsonc.is_file() {
            return self.config_jsonc.clone();
        }
        if self.config_json.is_file() {
            return self.config_json.clone();
        }
        self.config_jsonc.clone()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeProviderView {
    pub id: String,
    pub name: String,
    pub package: Option<String>,
    pub base_url: Option<String>,
    pub key_hint: String,
    pub models: Vec<CustomModelDef>,
    pub is_managed: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeLiveState {
    pub cli: CliInfo,
    pub config_path: String,
    pub config_exists: bool,
    /// "harness_auth" | "custom_provider" | "auth_json" | "none"
    pub config_source: String,
    pub active_model: Option<String>,
    pub base_url: Option<String>,
    pub providers: Vec<OpenCodeProviderView>,
    pub mcp_names: Vec<String>,
    pub skills_count: usize,
    pub parse_error: Option<String>,
}

pub fn detect_opencode_cli() -> CliInfo {
    let mut extra = Vec::new();
    if let Ok(home) = user_home() {
        extra.push(home.join(".opencode").join("bin").join("opencode.exe"));
        extra.push(home.join(".opencode").join("bin").join("opencode.cmd"));
        extra.push(home.join("AppData").join("Roaming").join("npm").join("opencode.cmd"));
        extra.push(home.join("AppData").join("Local").join("pnpm").join("opencode.cmd"));
        extra.push(home.join("AppData").join("Local").join("Programs").join("opencode").join("opencode.exe"));
    }
    detect_cli("opencode", &extra)
}

/// Strip `//` line comments, `/* ... */` block comments, and trailing commas
/// outside JSON strings so standard `serde_json` can parse `.jsonc` files.
pub fn parse_jsonc(raw: &str) -> Result<Value, String> {
    let stripped = strip_jsonc_comments(raw);
    let no_trailing = strip_trailing_commas(&stripped);
    serde_json::from_str(&no_trailing).map_err(|e| format!("JSONC ayrıştırma hatası: {e}"))
}

fn strip_jsonc_comments(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;

    while i < bytes.len() {
        let ch = bytes[i] as char;
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        if ch == '"' {
            in_string = true;
            out.push(ch);
            i += 1;
            continue;
        }

        if ch == '/' && i + 1 < bytes.len() {
            let next = bytes[i + 1] as char;
            if next == '/' {
                i += 2;
                while i < bytes.len() && bytes[i] as char != '\n' {
                    i += 1;
                }
                continue;
            }
            if next == '*' {
                i += 2;
                while i + 1 < bytes.len() {
                    if bytes[i] as char == '*' && bytes[i + 1] as char == '/' {
                        i += 2;
                        break;
                    }
                    if bytes[i] as char == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
                continue;
            }
        }

        out.push(ch);
        i += 1;
    }
    out
}

fn strip_trailing_commas(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    let mut in_string = false;
    let mut escaped = false;

    while i < bytes.len() {
        let ch = bytes[i] as char;
        if in_string {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if ch == '"' {
            in_string = true;
            out.push(ch);
            i += 1;
            continue;
        }
        if ch == ',' {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j] as char).is_whitespace() {
                j += 1;
            }
            if j < bytes.len() && matches!(bytes[j] as char, '}' | ']') {
                i += 1;
                continue;
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn read_config_object(path: &Path) -> Result<Map<String, Value>, String> {
    if !path.is_file() {
        return Ok(Map::new());
    }
    let raw = std::fs::read_to_string(path).map_err(|e| format!("Okuma hatası: {e}"))?;
    if raw.trim().is_empty() {
        return Ok(Map::new());
    }
    let val = parse_jsonc(&raw)?;
    Ok(val.as_object().cloned().unwrap_or_default())
}

fn extract_providers_map<'a>(cfg: &'a Map<String, Value>) -> Option<(&'static str, &'a Map<String, Value>)> {
    if let Some(m) = cfg.get("providers").and_then(|v| v.as_object()) {
        return Some(("providers", m));
    }
    if let Some(m) = cfg.get("provider").and_then(|v| v.as_object()) {
        return Some(("provider", m));
    }
    None
}

fn parse_provider_view(id: &str, entry: &Value) -> OpenCodeProviderView {
    let name = entry
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(id)
        .to_string();
    let package = entry
        .get("package")
        .and_then(|v| v.as_str())
        .map(String::from);
    let settings_or_opts = entry.get("settings").or_else(|| entry.get("options"));
    let base_url = settings_or_opts
        .and_then(|s| s.get("baseURL").or_else(|| s.get("baseUrl")))
        .and_then(|v| v.as_str())
        .map(String::from);
    let raw_key = settings_or_opts
        .and_then(|s| s.get("apiKey"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let models = entry
        .get("models")
        .and_then(|v| v.as_object())
        .map(|obj| {
            obj.iter()
                .map(|(mid, mv)| CustomModelDef {
                    id: mid.clone(),
                    name: mv
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or(mid)
                        .to_string(),
                    context_limit: mv.pointer("/limit/context").and_then(|v| v.as_u64()),
                    output_limit: mv.pointer("/limit/output").and_then(|v| v.as_u64()),
                })
                .collect()
        })
        .unwrap_or_default();

    OpenCodeProviderView {
        id: id.to_string(),
        name,
        package,
        base_url,
        key_hint: mask_secret(raw_key),
        models,
        is_managed: id == "harness-auth" || id.starts_with("harness-auth-") || id == "zcode",
    }
}

pub fn inspect_live() -> OpenCodeLiveState {
    let cli = detect_opencode_cli();
    let paths = match OpenCodePaths::live() {
        Ok(p) => p,
        Err(_) => {
            return OpenCodeLiveState {
                cli,
                config_source: "none".into(),
                ..Default::default()
            }
        }
    };
    inspect_live_at_with_cli(&paths, cli)
}

pub fn inspect_live_at(paths: &OpenCodePaths) -> OpenCodeLiveState {
    inspect_live_at_with_cli(paths, CliInfo::default())
}

fn inspect_live_at_with_cli(paths: &OpenCodePaths, cli: CliInfo) -> OpenCodeLiveState {
    let cfg_path = paths.active_config_path();
    let config_exists = cfg_path.is_file();
    let mut parse_error = None;
    let cfg = match read_config_object(&cfg_path) {
        Ok(m) => m,
        Err(e) => {
            parse_error = Some(e);
            Map::new()
        }
    };

    let active_model = cfg
        .get("model")
        .and_then(|v| v.as_str())
        .map(String::from);

    let mut providers = Vec::new();
    if let Some((_, pmap)) = extract_providers_map(&cfg) {
        for (pid, pval) in pmap {
            providers.push(parse_provider_view(pid, pval));
        }
    }

    let mcp_names = cfg
        .get("mcp")
        .and_then(|v| v.as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();

    let skills_count = cfg
        .get("skills")
        .and_then(|s| s.get("paths"))
        .and_then(|p| p.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let base_url = active_model
        .as_deref()
        .and_then(|m| m.split('/').next())
        .and_then(|pid| providers.iter().find(|p| p.id == pid))
        .and_then(|p| p.base_url.clone())
        .or_else(|| providers.first().and_then(|p| p.base_url.clone()));

    let auth_obj = read_json_object(&paths.auth_json);
    let config_source = if providers.iter().any(|p| p.is_managed)
        || active_model
            .as_deref()
            .map(|m| m.starts_with("harness-auth/") || m.starts_with("zcode/"))
            .unwrap_or(false)
    {
        "harness_auth"
    } else if !providers.is_empty() {
        "custom_provider"
    } else if !auth_obj.is_empty() {
        "auth_json"
    } else {
        "none"
    };

    OpenCodeLiveState {
        cli,
        config_path: cfg_path.display().to_string(),
        config_exists,
        config_source: config_source.into(),
        active_model,
        base_url,
        providers,
        mcp_names,
        skills_count,
        parse_error,
    }
}

/// Import any custom providers from `opencode.jsonc` into `UnifiedVault` if
/// not already tracked by slug + baseURL. Returns count imported.
pub fn import_providers_into_vault(
    paths: &OpenCodePaths,
    vault: &mut UnifiedVault,
    vault_key: &[u8; 32],
) -> Result<usize, String> {
    let cfg_path = paths.active_config_path();
    let cfg = read_config_object(&cfg_path)?;
    let Some((_, pmap)) = extract_providers_map(&cfg) else {
        return Ok(0);
    };
    let active_model = cfg.get("model").and_then(|v| v.as_str()).unwrap_or("");

    let mut imported = 0usize;
    for (pid, entry) in pmap {
        if pid == "harness-auth" || pid == "zcode" {
            continue;
        }
        let view = parse_provider_view(pid, entry);
        let settings_or_opts = entry.get("settings").or_else(|| entry.get("options"));
        let api_key = settings_or_opts
            .and_then(|s| s.get("apiKey"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        let Some(base_url) = view.base_url.clone().filter(|u| !u.trim().is_empty()) else {
            continue;
        };

        let slug = sanitize_provider_slug(pid);
        let exists = vault.accounts.iter().any(|a| {
            a.kind == "custom_provider"
                && (a.provider == slug || a.provider == *pid)
                && a.base_url.as_deref() == Some(base_url.as_str())
        });
        if exists {
            continue;
        }

        let default_model = active_model
            .strip_prefix(&format!("{pid}/"))
            .map(String::from)
            .or_else(|| view.models.first().map(|m| m.id.clone()));

        let pkg = view
            .package
            .unwrap_or_else(|| "@opencode/ai/providers/openai-compatible".into());
        let rec = UnifiedAccount::from_custom_provider(
            &view.name,
            pid,
            &pkg,
            &base_url,
            &api_key,
            default_model.as_deref(),
            view.models,
            vault_key,
        )?;
        vault.accounts.push(rec);
        imported += 1;
    }
    Ok(imported)
}

/// Upsert a provider entry in `opencode.jsonc` preserving `skills`, `mcp`, `$schema`,
/// and all unrelated user providers.
pub fn upsert_provider_in(
    paths: &OpenCodePaths,
    slug: &str,
    package: &str,
    name: &str,
    base_url: &str,
    api_key: &str,
    models: &[CustomModelDef],
    set_default_model: Option<&str>,
) -> Result<(), String> {
    let cfg_path = paths.active_config_path();
    let mut root = read_config_object(&cfg_path)?;
    if !root.contains_key("$schema") {
        root.insert(
            "$schema".into(),
            json!("https://opencode.ai/config.json"),
        );
    }

    let providers_key = if root.contains_key("providers") || !root.contains_key("provider") {
        "providers"
    } else {
        "provider"
    };

    let mut models_obj = Map::new();
    for m in models {
        if m.id.trim().is_empty() {
            continue;
        }
        let mut model_val = json!({
            "name": if m.name.trim().is_empty() { &m.id } else { &m.name },
        });
        if m.context_limit.is_some() || m.output_limit.is_some() {
            let mut limit = Map::new();
            if let Some(ctx) = m.context_limit {
                limit.insert("context".into(), json!(ctx));
            }
            if let Some(out) = m.output_limit {
                limit.insert("output".into(), json!(out));
            }
            model_val["limit"] = Value::Object(limit);
        }
        models_obj.insert(m.id.trim().to_string(), model_val);
    }

    let mut settings_obj = Map::new();
    settings_obj.insert("baseURL".into(), json!(base_url.trim().trim_end_matches('/')));
    if !api_key.trim().is_empty() {
        settings_obj.insert("apiKey".into(), json!(api_key.trim()));
    }

    let mut provider_entry = Map::new();
    if !package.trim().is_empty() {
        provider_entry.insert("package".into(), json!(package.trim()));
    }
    provider_entry.insert(
        "name".into(),
        json!(if name.trim().is_empty() { slug } else { name.trim() }),
    );
    provider_entry.insert("settings".into(), Value::Object(settings_obj));
    if !models_obj.is_empty() {
        provider_entry.insert("models".into(), Value::Object(models_obj));
    }

    let pmap = root
        .entry(providers_key)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("opencode config providers alanı nesne olmalı")?;
    pmap.insert(slug.to_string(), Value::Object(provider_entry));

    if let Some(model_id) = set_default_model.filter(|s| !s.trim().is_empty()) {
        root.insert("model".into(), json!(format!("{slug}/{}", model_id.trim())));
    }

    atomic_write(
        &cfg_path,
        &serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?,
    )
}

pub fn remove_provider_in(paths: &OpenCodePaths, slug: &str) -> Result<(), String> {
    let cfg_path = paths.active_config_path();
    if !cfg_path.is_file() {
        return Ok(());
    }
    let mut root = read_config_object(&cfg_path)?;
    for key in ["providers", "provider"] {
        if let Some(pmap) = root.get_mut(key).and_then(|v| v.as_object_mut()) {
            pmap.remove(slug);
        }
    }
    let prefix = format!("{slug}/");
    if root
        .get("model")
        .and_then(|v| v.as_str())
        .map(|m| m.starts_with(&prefix))
        .unwrap_or(false)
    {
        root.remove("model");
    }
    atomic_write(
        &cfg_path,
        &serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?,
    )
}

pub fn set_active_model_in(paths: &OpenCodePaths, model_ref: &str) -> Result<(), String> {
    let cfg_path = paths.active_config_path();
    let mut root = read_config_object(&cfg_path)?;
    root.insert("model".into(), json!(model_ref.trim()));
    atomic_write(
        &cfg_path,
        &serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?,
    )
}

pub fn provider_slug_for_account(account: &UnifiedAccount) -> String {
    match account.kind.as_str() {
        "custom_provider" => {
            let s = sanitize_provider_slug(&account.provider);
            if s.is_empty() {
                format!("custom-{}", &account.id[..account.id.len().min(6)])
            } else {
                s
            }
        }
        "z_api" => format!("zai-api-{}", &account.id[..account.id.len().min(6)]),
        "zcode_oauth" => format!("zcode-oauth-{}", &account.id[..account.id.len().min(6)]),
        _ => sanitize_provider_slug(&account.provider),
    }
}

pub fn apply_unified_account_at(
    paths: &OpenCodePaths,
    account: &UnifiedAccount,
    vault_key: &[u8; 32],
    zcode_proxy: Option<(&str, &str)>,
    chosen_model: Option<&str>,
    set_as_default: bool,
) -> Result<(), String> {
    let cfg_path = paths.active_config_path();
    let has_model_already = read_config_object(&cfg_path)
        .map(|m| m.get("model").and_then(|v| v.as_str()).map(|s| !s.trim().is_empty()).unwrap_or(false))
        .unwrap_or(false);
    let write_default = set_as_default || !has_model_already;
    let slug = provider_slug_for_account(account);

    match account.kind.as_str() {
        "zcode_oauth" => {
            let (endpoint, proxy_key) = zcode_proxy
                .ok_or_else(|| "ZCode OAuth için arka plan proxy çalışıyor olmalı".to_string())?;
            let model = chosen_model
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .or(account.default_model.as_deref())
                .unwrap_or("glm-5.3");
            let models = vec![
                CustomModelDef {
                    id: model.into(),
                    name: format!("{model} (ZCode OAuth)"),
                    context_limit: Some(200_000),
                    output_limit: Some(16_384),
                },
                CustomModelDef {
                    id: "glm-5.3-flash".into(),
                    name: "GLM-5.3 Flash (ZCode OAuth)".into(),
                    context_limit: Some(200_000),
                    output_limit: Some(16_384),
                },
            ];
            let base = format!("{}/v1", endpoint.trim_end_matches('/'));
            upsert_provider_in(
                paths,
                &slug,
                "@opencode/ai/providers/anthropic",
                &format!("{} (ZCode OAuth)", account.name),
                &base,
                proxy_key,
                &models,
                write_default.then_some(model),
            )
        }
        "z_api" => {
            let api_key = account.extract_api_key(vault_key)?;
            let raw_base = account
                .base_url
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(if account.provider == "bigmodel" {
                    "https://open.bigmodel.cn/api/anthropic"
                } else {
                    "https://api.z.ai/api/anthropic"
                });
            let base = raw_base.trim().trim_end_matches('/');
            let v1_base = if base.ends_with("/v1") {
                base.to_string()
            } else {
                format!("{base}/v1")
            };
            let model = chosen_model
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .or(account.default_model.as_deref())
                .unwrap_or("glm-5.3");
            let models = vec![
                CustomModelDef {
                    id: model.into(),
                    name: format!("{model} (Z.AI)"),
                    context_limit: Some(200_000),
                    output_limit: Some(16_384),
                },
                CustomModelDef {
                    id: "glm-5.2".into(),
                    name: "GLM-5.2 (Z.AI)".into(),
                    context_limit: Some(200_000),
                    output_limit: Some(16_384),
                },
            ];
            upsert_provider_in(
                paths,
                &slug,
                "@opencode/ai/providers/anthropic",
                &format!("{} (Z.AI API)", account.name),
                &v1_base,
                &api_key,
                &models,
                write_default.then_some(model),
            )
        }
        "custom_provider" => {
            let api_key = account.extract_api_key(vault_key)?;
            let base_url = account
                .base_url
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .ok_or("Özel sağlayıcıda Base URL eksik")?;
            let package = account
                .package
                .as_deref()
                .unwrap_or("@opencode/ai/providers/openai-compatible");
            let model = chosen_model
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .or(account.default_model.as_deref())
                .or_else(|| account.models.first().map(|m| m.id.as_str()));
            upsert_provider_in(
                paths,
                &slug,
                package,
                &account.name,
                base_url,
                &api_key,
                &account.models,
                if write_default { model } else { None },
            )
        }
        other => Err(format!("OpenCode bu hesap türünü desteklemiyor: {other}")),
    }
}

pub fn remove_unified_account_at(
    paths: &OpenCodePaths,
    account: &UnifiedAccount,
) -> Result<(), String> {
    let slug = provider_slug_for_account(account);
    remove_provider_in(paths, &slug)
}

pub fn disable_at(paths: &OpenCodePaths) -> Result<(), String> {
    let cfg_path = paths.active_config_path();
    if cfg_path.is_file() {
        let live = inspect_live_at(paths);
        for p in live.providers {
            if p.id == "harness-auth"
                || p.id == "zcode"
                || p.id.starts_with("zai-api-")
                || p.id.starts_with("zcode-oauth-")
            {
                let _ = remove_provider_in(paths, &p.id);
            }
        }
    }
    if paths.auth_json.is_file() {
        let _ = std::fs::remove_file(&paths.auth_json);
    }
    Ok(())
}

pub fn launch_opencode_folder(launches_dir: &Path, folder: &Path) -> Result<(), String> {
    launch_in_terminal(launches_dir, "OpenCode", folder, &[], "opencode", true).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto;

    fn temp_home(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "ha-opencode-v2-{tag}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(p.join(".config").join("opencode")).unwrap();
        p
    }

    #[test]
    fn jsonc_parses_comments_urls_and_trailing_commas() {
        let sample = r#"{
          // line comment with https://example.com
          "$schema": "https://opencode.ai/config.json",
          "skills": { "paths": ["C:/skills/a"] },
          "providers": {
            "colab-qwen": {
              "package": "@opencode/ai/providers/openai-compatible",
              "name": "Colab Qwen3.5",
              "settings": {
                "baseURL": "https://test.trycloudflare.com/v1",
                "apiKey": "local-colab",
              },
              "models": {
                "Qwen3.5.gguf": {
                  "name": "Qwen 3.5",
                  "limit": { "context": 32768, "output": 4096 },
                },
              },
            },
          },
          /* block comment */
          "model": "colab-qwen/Qwen3.5.gguf",
          "mcp": { "chrome": { "type": "local" } },
        }"#;

        let val = parse_jsonc(sample).unwrap();
        assert_eq!(
            val.pointer("/providers/colab-qwen/settings/baseURL")
                .and_then(|v| v.as_str()),
            Some("https://test.trycloudflare.com/v1")
        );
        assert_eq!(
            val.pointer("/providers/colab-qwen/models/Qwen3.5.gguf/limit/context")
                .and_then(|v| v.as_u64()),
            Some(32768)
        );
    }

    #[test]
    fn preserves_skills_and_mcp_while_importing_and_switching() {
        let home = temp_home("preserve");
        let paths = OpenCodePaths::under(&home);
        atomic_write(
            &paths.config_jsonc,
            r#"{
              "skills": { "paths": ["~/.agents/skills"] },
              "providers": {
                "colab-qwen": {
                  "package": "@opencode/ai/providers/openai-compatible",
                  "name": "Colab Qwen3.5",
                  "settings": { "baseURL": "https://abc.trycloudflare.com/v1", "apiKey": "k-123" },
                  "models": {
                    "Qwen3.5.gguf": { "name": "Qwen 3.5", "limit": { "context": 32768, "output": 4096 } }
                  }
                }
              },
              "model": "colab-qwen/Qwen3.5.gguf",
              "mcp": { "playwright": { "enabled": true } }
            }"#,
        )
        .unwrap();

        let key = crypto::derive_vault_key("test");
        let mut vault = UnifiedVault::default();
        let imported = import_providers_into_vault(&paths, &mut vault, &key).unwrap();
        assert_eq!(imported, 1);
        assert_eq!(vault.accounts[0].models.len(), 1);
        assert_eq!(vault.accounts[0].models[0].context_limit, Some(32768));

        let z_acc = UnifiedAccount::from_z_api_key("ZAI Key", "zai", "kid.ksec", None, &key).unwrap();
        let z_slug = provider_slug_for_account(&z_acc);
        apply_unified_account_at(&paths, &z_acc, &key, None, Some("glm-5.3"), true).unwrap();

        let live = inspect_live_at(&paths);
        assert_eq!(live.active_model.as_deref(), Some(format!("{z_slug}/glm-5.3").as_str()));
        assert_eq!(live.skills_count, 1);
        assert_eq!(live.mcp_names, vec!["playwright".to_string()]);
        assert_eq!(live.providers.len(), 2);

        // Second custom provider can coexist simultaneously without disturbing first two.
        let cp2 = UnifiedAccount::from_custom_provider(
            "Local Ollama",
            "local-ollama",
            "@opencode/ai/providers/openai-compatible",
            "http://127.0.0.1:11434/v1",
            "ollama",
            Some("qwen2.5-coder"),
            vec![CustomModelDef {
                id: "qwen2.5-coder".into(),
                name: "Qwen 2.5 Coder".into(),
                context_limit: Some(32768),
                output_limit: Some(4096),
            }],
            &key,
        )
        .unwrap();
        apply_unified_account_at(&paths, &cp2, &key, None, None, false).unwrap();
        let multi = inspect_live_at(&paths);
        assert_eq!(multi.providers.len(), 3);
        assert_eq!(multi.active_model.as_deref(), Some(format!("{z_slug}/glm-5.3").as_str()));

        remove_unified_account_at(&paths, &z_acc).unwrap();
        let after = inspect_live_at(&paths);
        assert_eq!(after.providers.len(), 2);
        assert!(after.providers.iter().any(|p| p.id == "colab-qwen"));
        assert!(after.providers.iter().any(|p| p.id == "local-ollama"));

        let _ = std::fs::remove_dir_all(&home);
    }
}
