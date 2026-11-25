//! Unified encrypted vault (`vault.json`) for HarnessAuth v2.
//!
//! Stores all accounts and custom model providers in a single AES-256-GCM
//! encrypted vault, replacing separate `accounts.json` and `sessions.json`
//! silos with automatic first-run migration:
//!
//! - `zcode_oauth`: Z.AI / BigModel OAuth (Coding Plan & Start Plan)
//! - `z_api`: Z.AI / BigModel API Key
//! - `claude_oauth`: Anthropic Claude Pro / Max OAuth
//! - `antigravity_oauth`: Google Antigravity CLI (`agy`) OAuth
//! - `custom_provider`: `opencode.jsonc`-compatible custom provider + models

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::crypto;
use crate::harness::{self, SessionVault};
use crate::paths::Paths;
use crate::settings::Settings;
use crate::vault::{self, VaultData};

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomModelDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub context_limit: Option<u64>,
    #[serde(default)]
    pub output_limit: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedAccount {
    pub id: String,
    pub name: String,
    /// "zcode_oauth" | "z_api" | "claude_oauth" | "antigravity_oauth" | "custom_provider"
    pub kind: String,
    /// "zai" | "bigmodel" | "anthropic" | "google" | "<provider-slug>"
    pub provider: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub key_hint: String,
    #[serde(default)]
    pub user_hint: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub package: Option<String>,
    #[serde(default)]
    pub default_plan: Option<String>,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub models: Vec<CustomModelDef>,
    #[serde(default)]
    pub has_jwt: bool,
    #[serde(default)]
    pub has_secret: bool,
    #[serde(default)]
    pub expires_at: Option<i64>,
    pub created_at: i64,
    #[serde(default)]
    pub last_used_at: Option<i64>,
    pub encrypted: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedAccountSummary {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub provider: String,
    pub enabled: bool,
    pub key_hint: String,
    pub user_hint: Option<String>,
    pub base_url: Option<String>,
    pub package: Option<String>,
    pub default_plan: Option<String>,
    pub default_model: Option<String>,
    pub models: Vec<CustomModelDef>,
    pub has_jwt: bool,
    pub has_secret: bool,
    pub expires_at: Option<i64>,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub supported_harnesses: Vec<String>,
    pub active_in: Vec<String>,
}

pub fn supported_harnesses_for_kind(kind: &str) -> Vec<String> {
    match kind {
        "zcode_oauth" | "z_api" => vec!["claude".into(), "opencode".into(), "zcode".into()],
        "claude_oauth" => vec!["claude".into()],
        "antigravity_oauth" => vec!["antigravity".into()],
        "custom_provider" => vec!["claude".into(), "opencode".into()],
        _ => Vec::new(),
    }
}

impl UnifiedAccount {
    pub fn supports(&self, harness_name: &str) -> bool {
        supported_harnesses_for_kind(&self.kind)
            .iter()
            .any(|h| h == harness_name)
    }

    pub fn summary(&self, settings: &Settings) -> UnifiedAccountSummary {
        let mut active_in = Vec::new();
        if settings.claude_active_session_id.as_deref() == Some(self.id.as_str())
            && settings.claude_active_method != "disabled"
        {
            active_in.push("claude".to_string());
        }
        if settings.opencode_active_session_id.as_deref() == Some(self.id.as_str())
            && settings.opencode_active_method != "disabled"
        {
            active_in.push("opencode".to_string());
        }
        if settings.zcode_app_active_account_id.as_deref() == Some(self.id.as_str())
            && settings.zcode_app_active_method != "disabled"
        {
            active_in.push("zcode".to_string());
        }
        if settings.antigravity_active_session_id.as_deref() == Some(self.id.as_str()) {
            active_in.push("antigravity".to_string());
        }

        UnifiedAccountSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.kind.clone(),
            provider: self.provider.clone(),
            enabled: self.enabled,
            key_hint: self.key_hint.clone(),
            user_hint: self.user_hint.clone(),
            base_url: self.base_url.clone(),
            package: self.package.clone(),
            default_plan: self.default_plan.clone(),
            default_model: self.default_model.clone(),
            models: self.models.clone(),
            has_jwt: self.has_jwt,
            has_secret: self.has_secret,
            expires_at: self.expires_at,
            created_at: self.created_at,
            last_used_at: self.last_used_at,
            supported_harnesses: supported_harnesses_for_kind(&self.kind),
            active_in,
        }
    }

    pub fn decrypt_payload(&self, vault_key: &[u8; 32]) -> Result<Value, String> {
        let raw = crypto::open(vault_key, &self.encrypted)?;
        serde_json::from_str(&raw).map_err(|e| format!("hesap verisi çözülemedi: {e}"))
    }

    pub fn reencrypt_payload(&mut self, payload: &Value, vault_key: &[u8; 32]) -> Result<(), String> {
        let raw = serde_json::to_string(payload).map_err(|e| e.to_string())?;
        self.encrypted = crypto::seal(vault_key, &raw)?;
        Ok(())
    }

    /// Build a `zcode_oauth` or `z_api` record from a proxy Credential JSON value.
    pub fn from_zcode_credential(
        name: &str,
        kind: &str, // "zcode_oauth" | "z_api"
        cred: &Value,
        base_url: Option<String>,
        vault_key: &[u8; 32],
    ) -> Result<Self, String> {
        let legacy_kind = if kind == "zcode_oauth" || kind == "oauth" {
            "oauth"
        } else {
            "api-key"
        };
        let legacy = vault::record_from_credential(name, legacy_kind, cred, vault_key)?;
        Ok(Self {
            id: legacy.id,
            name: legacy.name,
            kind: if legacy_kind == "oauth" {
                "zcode_oauth".into()
            } else {
                "z_api".into()
            },
            provider: legacy.provider,
            enabled: true,
            key_hint: legacy.key_hint,
            user_hint: legacy.user_hint,
            base_url,
            package: None,
            default_plan: legacy.default_plan,
            default_model: legacy.default_model,
            models: Vec::new(),
            has_jwt: legacy.has_jwt,
            has_secret: legacy.has_secret,
            expires_at: None,
            created_at: legacy.created_at,
            last_used_at: None,
            encrypted: legacy.encrypted,
        })
    }

    /// Build a `z_api` account directly from provider + API key + optional baseURL.
    pub fn from_z_api_key(
        name: &str,
        provider: &str,
        api_key: &str,
        base_url: Option<&str>,
        vault_key: &[u8; 32],
    ) -> Result<Self, String> {
        let prov = match provider.trim().to_ascii_lowercase().as_str() {
            "bigmodel" => "bigmodel",
            _ => "zai",
        };
        let key = api_key.trim();
        if key.is_empty() {
            return Err("API anahtarı boş olamaz".into());
        }
        let default_base = if prov == "bigmodel" {
            "https://open.bigmodel.cn/api/anthropic"
        } else {
            "https://api.z.ai/api/anthropic"
        };
        let resolved_base = base_url
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(default_base)
            .trim_end_matches('/')
            .to_string();

        let cred = if let Some((id, secret)) = key.split_once('.') {
            if !id.is_empty() && !secret.is_empty() {
                json!({
                    "provider": prov,
                    "apiKey": key,
                    "secret": secret,
                    "baseUrl": resolved_base,
                    "createdAt": chrono::Utc::now().to_rfc3339(),
                })
            } else {
                json!({
                    "provider": prov,
                    "apiKey": key,
                    "baseUrl": resolved_base,
                    "createdAt": chrono::Utc::now().to_rfc3339(),
                })
            }
        } else {
            json!({
                "provider": prov,
                "apiKey": key,
                "baseUrl": resolved_base,
                "createdAt": chrono::Utc::now().to_rfc3339(),
            })
        };

        Self::from_zcode_credential(name, "z_api", &cred, Some(resolved_base), vault_key)
    }

    pub fn from_custom_provider(
        name: &str,
        provider_slug: &str,
        package: &str,
        base_url: &str,
        api_key: &str,
        default_model: Option<&str>,
        models: Vec<CustomModelDef>,
        vault_key: &[u8; 32],
    ) -> Result<Self, String> {
        let slug = sanitize_provider_slug(provider_slug);
        if slug.is_empty() {
            return Err("Sağlayıcı kimliği (provider ID) boş olamaz".into());
        }
        let clean_name = name.trim();
        let display_name = if clean_name.is_empty() {
            slug.clone()
        } else {
            clean_name.to_string()
        };
        let clean_base = base_url.trim().trim_end_matches('/').to_string();
        if clean_base.is_empty() {
            return Err("Base URL boş olamaz".into());
        }
        let pkg = if package.trim().is_empty() {
            "@opencode/ai/providers/openai-compatible".to_string()
        } else {
            package.trim().to_string()
        };
        let normalized_models: Vec<CustomModelDef> = models
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
        let chosen_default = default_model
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .or_else(|| normalized_models.first().map(|m| m.id.clone()));

        let clean_key = api_key.trim().to_string();
        let payload = json!({
            "apiKey": clean_key,
            "baseURL": clean_base,
            "package": pkg,
        });
        let encrypted = crypto::seal(vault_key, &payload.to_string())?;
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: display_name,
            kind: "custom_provider".into(),
            provider: slug,
            enabled: true,
            key_hint: harness::mask_secret(&clean_key),
            user_hint: chosen_default.clone(),
            base_url: Some(clean_base),
            package: Some(pkg),
            default_plan: None,
            default_model: chosen_default,
            models: normalized_models,
            has_jwt: false,
            has_secret: !clean_key.is_empty(),
            expires_at: None,
            created_at: chrono::Utc::now().timestamp_millis(),
            last_used_at: None,
            encrypted,
        })
    }

    pub fn extract_zcode_credential(&self, vault_key: &[u8; 32]) -> Result<Value, String> {
        let mut payload = self.decrypt_payload(vault_key)?;
        if self.kind == "z_api" {
            // Ensure `baseUrl` override is reflected inside proxy credential if set.
            if let Some(url) = &self.base_url {
                if !url.trim().is_empty() {
                    if let Some(obj) = payload.as_object_mut() {
                        obj.insert("baseUrl".into(), Value::String(url.clone()));
                    }
                }
            }
        }
        Ok(payload)
    }

    pub fn extract_api_key(&self, vault_key: &[u8; 32]) -> Result<String, String> {
        let payload = self.decrypt_payload(vault_key)?;
        if let Some(k) = payload.get("apiKey").and_then(|v| v.as_str()) {
            if !k.trim().is_empty() {
                return Ok(k.to_string());
            }
        }
        if let (Some(id), Some(sec)) = (
            payload.get("apiKeyId").and_then(|v| v.as_str()),
            payload.get("apiKeySecret").and_then(|v| v.as_str()),
        ) {
            if !id.is_empty() && !sec.is_empty() {
                return Ok(format!("{id}.{sec}"));
            }
        }
        if let Some(id) = payload.get("apiKeyId").and_then(|v| v.as_str()) {
            if !id.is_empty() {
                return Ok(id.to_string());
            }
        }
        Ok(String::new())
    }
}

pub fn sanitize_provider_slug(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() || ch == '/' {
            if !out.ends_with('-') {
                out.push('-');
            }
        }
    }
    out.trim_matches('-').to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct UnifiedVault {
    pub version: u32,
    pub accounts: Vec<UnifiedAccount>,
}

impl UnifiedVault {
    pub fn load(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut copy = self.clone();
        copy.version = 2;
        let json = serde_json::to_string_pretty(&copy).map_err(|e| e.to_string())?;
        harness::atomic_write(path, &json)
    }

    pub fn summaries(&self, settings: &Settings) -> Vec<UnifiedAccountSummary> {
        self.accounts.iter().map(|a| a.summary(settings)).collect()
    }

    pub fn find(&self, id: &str) -> Option<&UnifiedAccount> {
        self.accounts.iter().find(|a| a.id == id)
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut UnifiedAccount> {
        self.accounts.iter_mut().find(|a| a.id == id)
    }

    pub fn first_enabled_for(&self, harness_name: &str) -> Option<&UnifiedAccount> {
        self.accounts
            .iter()
            .find(|a| a.enabled && a.supports(harness_name))
    }

    pub fn first_enabled_zcode_service(&self) -> Option<&UnifiedAccount> {
        self.accounts
            .iter()
            .find(|a| a.enabled && a.kind == "zcode_oauth")
    }

    /// Reconcile active settings pointers against accounts currently in the vault.
    pub fn reconcile_settings(&self, settings: &mut Settings) -> bool {
        let valid: HashSet<&str> = self.accounts.iter().map(|a| a.id.as_str()).collect();
        let mut changed = false;

        if settings
            .zcode_service_account_id
            .as_deref()
            .map(|id| !valid.contains(id))
            .unwrap_or(false)
        {
            settings.zcode_service_account_id = None;
            changed = true;
        }
        if settings
            .claude_active_session_id
            .as_deref()
            .map(|id| !valid.contains(id))
            .unwrap_or(false)
        {
            settings.claude_active_session_id = None;
            if matches!(
                settings.claude_active_method.as_str(),
                "claude_oauth" | "custom_provider"
            ) {
                settings.claude_active_method = "disabled".into();
            }
            changed = true;
        }
        if settings
            .opencode_active_session_id
            .as_deref()
            .map(|id| !valid.contains(id))
            .unwrap_or(false)
        {
            settings.opencode_active_session_id = None;
            if settings.opencode_active_method == "custom_provider" {
                settings.opencode_active_method = "disabled".into();
            }
            changed = true;
        }
        if settings
            .zcode_app_active_account_id
            .as_deref()
            .map(|id| !valid.contains(id))
            .unwrap_or(false)
        {
            settings.zcode_app_active_account_id = None;
            settings.zcode_app_active_method = "disabled".into();
            changed = true;
        }
        if settings
            .antigravity_active_session_id
            .as_deref()
            .map(|id| !valid.contains(id))
            .unwrap_or(false)
        {
            settings.antigravity_active_session_id = None;
            changed = true;
        }
        changed
    }

    /// Load `vault.json` if present, or perform one-time migration from v1
    /// `accounts.json` + `sessions.json` (and legacy `zcode-manager` if present).
    pub fn load_or_migrate(
        paths: &Paths,
        vault_key: &[u8; 32],
        settings: &mut Settings,
    ) -> (Self, bool) {
        let vault_path = paths.unified_vault_file();
        if vault_path.exists() {
            let v = Self::load(&vault_path);
            let s_changed = v.reconcile_settings(settings);
            return (v, s_changed);
        }

        let mut unified = Self {
            version: 2,
            accounts: Vec::new(),
        };

        // 1. Legacy ZCode accounts (`accounts.json` or legacy zcode-manager)
        let old_vault = if paths.vault_file().exists() {
            VaultData::load(&paths.vault_file())
        } else {
            vault::migrate_legacy_accounts(&paths.vault_file(), vault_key)
                .unwrap_or_else(|| VaultData::load(&paths.vault_file()))
        };

        for acc in &old_vault.accounts {
            unified.accounts.push(UnifiedAccount {
                id: acc.id.clone(),
                name: acc.name.clone(),
                kind: if acc.kind == "oauth" {
                    "zcode_oauth".into()
                } else {
                    "z_api".into()
                },
                provider: acc.provider.clone(),
                enabled: acc.enabled,
                key_hint: acc.key_hint.clone(),
                user_hint: acc.user_hint.clone(),
                base_url: acc.base_url.clone(),
                package: None,
                default_plan: acc.default_plan.clone(),
                default_model: acc.default_model.clone(),
                models: Vec::new(),
                has_jwt: acc.has_jwt,
                has_secret: acc.has_secret,
                expires_at: None,
                created_at: acc.created_at,
                last_used_at: acc.last_used_at,
                encrypted: acc.encrypted.clone(),
            });
        }
        if settings.zcode_service_account_id.is_none() {
            settings.zcode_service_account_id = old_vault
                .active_id
                .clone()
                .or_else(|| unified.first_enabled_zcode_service().map(|a| a.id.clone()));
        }

        // 2. Legacy multi-harness sessions (`sessions.json`)
        let old_sessions = SessionVault::load(&paths.sessions_file());
        for sess in &old_sessions.sessions {
            match sess.kind.as_str() {
                "claude_oauth" | "antigravity_oauth" => {
                    unified.accounts.push(UnifiedAccount {
                        id: sess.id.clone(),
                        name: sess.name.clone(),
                        kind: sess.kind.clone(),
                        provider: sess
                            .provider_hint
                            .clone()
                            .unwrap_or_else(|| {
                                if sess.kind == "claude_oauth" {
                                    "anthropic".into()
                                } else {
                                    "google".into()
                                }
                            }),
                        enabled: sess.enabled,
                        key_hint: sess.account_hint.clone().unwrap_or_default(),
                        user_hint: sess.account_hint.clone(),
                        base_url: None,
                        package: None,
                        default_plan: None,
                        default_model: None,
                        models: Vec::new(),
                        has_jwt: false,
                        has_secret: true,
                        expires_at: sess.expires_at,
                        created_at: sess.created_at,
                        last_used_at: sess.last_used_at,
                        encrypted: sess.encrypted.clone(),
                    });
                }
                "opencode_api" | "claude_api" => {
                    let payload = sess.decrypt_payload(vault_key).unwrap_or_else(|_| json!({}));
                    let api_key = payload
                        .get("apiKey")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let base_url = payload
                        .get("baseUrl")
                        .or_else(|| payload.get("baseURL"))
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let model = payload
                        .get("model")
                        .and_then(|v| v.as_str())
                        .or_else(|| sess.meta.get("model").and_then(|v| v.as_str()))
                        .map(String::from);
                    let slug = sanitize_provider_slug(
                        sess.provider_hint
                            .as_deref()
                            .unwrap_or(&sess.name),
                    );
                    let slug = if slug.is_empty() {
                        format!("custom-{}", &sess.id[..8.min(sess.id.len())])
                    } else {
                        slug
                    };
                    let pkg = if base_url.contains("anthropic") || sess.kind == "claude_api" {
                        "@opencode/ai/providers/anthropic"
                    } else {
                        "@opencode/ai/providers/openai-compatible"
                    };
                    let models = model
                        .as_ref()
                        .map(|m| {
                            vec![CustomModelDef {
                                id: m.clone(),
                                name: m.clone(),
                                context_limit: None,
                                output_limit: None,
                            }]
                        })
                        .unwrap_or_default();
                    let normalized = json!({
                        "apiKey": api_key,
                        "baseURL": base_url,
                        "package": pkg,
                    });
                    if let Ok(enc) = crypto::seal(vault_key, &normalized.to_string()) {
                        unified.accounts.push(UnifiedAccount {
                            id: sess.id.clone(),
                            name: sess.name.clone(),
                            kind: "custom_provider".into(),
                            provider: slug,
                            enabled: sess.enabled,
                            key_hint: harness::mask_secret(&api_key),
                            user_hint: model.clone(),
                            base_url: (!base_url.is_empty()).then_some(base_url),
                            package: Some(pkg.into()),
                            default_plan: None,
                            default_model: model,
                            models,
                            has_jwt: false,
                            has_secret: !api_key.is_empty(),
                            expires_at: None,
                            created_at: sess.created_at,
                            last_used_at: sess.last_used_at,
                            encrypted: enc,
                        });
                    }
                }
                "opencode_zcode_proxy" => {
                    // Already represented directly by the migrated zcode_oauth / z_api accounts.
                }
                _ => {}
            }
        }

        if settings.opencode_active_method == "profile" {
            settings.opencode_active_method = "custom_provider".into();
        }
        unified.reconcile_settings(settings);

        if let Err(e) = unified.save(&vault_path) {
            eprintln!("unified vault migration save failed: {e}");
        }
        (unified, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "harness-auth-uv-{tag}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn custom_provider_roundtrip_and_migration() {
        let dir = temp_dir("roundtrip");
        let paths = Paths { data_dir: dir.clone() };
        paths.ensure_all().unwrap();
        let key = crypto::derive_vault_key("test-secret");

        // Seed legacy VaultData
        let mut legacy_v = VaultData::default();
        let acc = vault::record_from_credential(
            "Legacy ZAI",
            "api-key",
            &json!({ "provider": "zai", "apiKey": "id1.sec2", "secret": "sec2" }),
            &key,
        )
        .unwrap();
        let legacy_id = acc.id.clone();
        legacy_v.accounts.push(acc);
        legacy_v.active_id = Some(legacy_id.clone());
        legacy_v.save(&paths.vault_file()).unwrap();

        let mut settings = Settings::default();
        let (mut uv, migrated) = UnifiedVault::load_or_migrate(&paths, &key, &mut settings);
        assert!(migrated);
        assert_eq!(uv.accounts.len(), 1);
        assert_eq!(uv.accounts[0].kind, "z_api");
        assert_eq!(settings.zcode_service_account_id.as_deref(), Some(legacy_id.as_str()));

        let custom = UnifiedAccount::from_custom_provider(
            "Colab Qwen 3.5",
            "colab-qwen3-5-9b",
            "@opencode/ai/providers/openai-compatible",
            "https://example.trycloudflare.com/v1",
            "local-colab",
            Some("Qwen3.5-9B.gguf"),
            vec![CustomModelDef {
                id: "Qwen3.5-9B.gguf".into(),
                name: "Qwen3.5 9B Uncensored".into(),
                context_limit: Some(32768),
                output_limit: Some(4096),
            }],
            &key,
        )
        .unwrap();
        assert!(custom.supports("claude"));
        assert!(custom.supports("opencode"));
        assert!(!custom.supports("zcode"));
        uv.accounts.push(custom);
        uv.save(&paths.unified_vault_file()).unwrap();

        let (reloaded, _) = UnifiedVault::load_or_migrate(&paths, &key, &mut settings);
        assert_eq!(reloaded.accounts.len(), 2);
        let q = reloaded.accounts[1].extract_api_key(&key).unwrap();
        assert_eq!(q, "local-colab");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
