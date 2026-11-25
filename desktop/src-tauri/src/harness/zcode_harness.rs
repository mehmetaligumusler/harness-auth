//! ZCode Desktop (`~/.zcode/v2/`) adapter.
//!
//! Inspects live ZCode configuration (`provider_config.json`, `setting.json`,
//! `config.json`), imports existing `standard-personal` API key rules into the
//! unified vault, switches ZCode cleanly between `zcode_oauth` (Coding/Start
//! Plan) and `z_api` accounts, or disables active personal API key rules.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;
use serde_json::{json, Value};

use crate::harness::{atomic_write, mask_secret, read_json_object, user_home};
use crate::unified_vault::{UnifiedAccount, UnifiedVault};

#[derive(Debug, Clone)]
pub struct ZCodePaths {
    pub v2_dir: PathBuf,
    pub provider_config_json: PathBuf,
    pub setting_json: PathBuf,
    pub config_json: PathBuf,
}

impl ZCodePaths {
    pub fn live() -> Result<Self, String> {
        let home = user_home()?;
        Ok(Self::in_home(&home))
    }

    pub fn in_home(home: &Path) -> Self {
        let v2_dir = home.join(".zcode").join("v2");
        Self {
            provider_config_json: v2_dir.join("provider_config.json"),
            setting_json: v2_dir.join("setting.json"),
            config_json: v2_dir.join("config.json"),
            v2_dir,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZCodePersonalRuleView {
    pub provider_id: String,
    pub provider_name: String,
    pub enabled: bool,
    pub base_url: Option<String>,
    pub key_hint: String,
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ZCodeLiveState {
    pub installed: bool,
    pub app_path: Option<String>,
    pub v2_exists: bool,
    /// "oauth" | "api-key" | "unknown"
    pub zai_mode: String,
    pub selected_key: Option<String>,
    pub active_personal_name: Option<String>,
    pub personal_rules: Vec<ZCodePersonalRuleView>,
}

#[derive(Debug, Clone)]
pub struct ImportedZCodeRule {
    pub provider_id: String,
    pub provider_name: String,
    pub provider_family: String,
    pub api_key: String,
    pub base_url: String,
    pub enabled: bool,
}

pub fn detect_zcode_exe() -> Option<PathBuf> {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let candidate = PathBuf::from(&local)
            .join("Programs")
            .join("ZCode")
            .join("ZCode.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    if let Ok(pf) = std::env::var("ProgramFiles") {
        let candidate = PathBuf::from(&pf).join("ZCode").join("ZCode.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn inspect_live() -> ZCodeLiveState {
    match ZCodePaths::live() {
        Ok(paths) => inspect_live_at(&paths, detect_zcode_exe()),
        Err(_) => ZCodeLiveState::default(),
    }
}

pub fn inspect_live_at(paths: &ZCodePaths, app_exe: Option<PathBuf>) -> ZCodeLiveState {
    let v2_exists = paths.v2_dir.is_dir();
    let app_path = app_exe.map(|p| p.display().to_string());
    let installed = app_path.is_some() || v2_exists;

    let setting = read_json_object(&paths.setting_json);
    let zai_mode = setting
        .get("modelProviderFamilyModes")
        .and_then(|v| v.get("zai"))
        .and_then(|v| v.as_str())
        .unwrap_or("oauth")
        .to_string();
    let selected_key = setting
        .get("modelProviderFamilySelectedKeys")
        .and_then(|v| v.get("zai"))
        .and_then(|v| v.as_str())
        .map(String::from);

    let pcfg = read_json_object(&paths.provider_config_json);
    let rules = pcfg
        .get("config")
        .and_then(|c| c.get("providerConfigRules"))
        .and_then(|r| r.get("providerRules"))
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();

    let mut personal_rules = Vec::new();
    let mut active_personal_name = None;

    for rule in rules {
        let cfg = rule.get("config");
        let group = cfg
            .and_then(|c| c.get("group"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if group != "standard-personal" {
            continue;
        }
        let provider_id = rule
            .get("providerId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let provider_name = rule
            .get("providerName")
            .and_then(|v| v.as_str())
            .unwrap_or(&provider_id)
            .to_string();
        let enabled = rule
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let base_url = cfg
            .and_then(|c| c.get("api"))
            .and_then(|a| a.get("baseUrl"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let raw_key = cfg
            .and_then(|c| c.get("access"))
            .and_then(|a| a.get("apiKey"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let models = cfg
            .and_then(|c| c.get("personalModelIds"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        if enabled && active_personal_name.is_none() {
            active_personal_name = Some(provider_name.clone());
        }

        personal_rules.push(ZCodePersonalRuleView {
            provider_id,
            provider_name,
            enabled,
            base_url,
            key_hint: mask_secret(raw_key),
            models,
        });
    }

    ZCodeLiveState {
        installed,
        app_path,
        v2_exists,
        zai_mode,
        selected_key,
        active_personal_name,
        personal_rules,
    }
}

pub fn extract_personal_rules(paths: &ZCodePaths) -> Vec<ImportedZCodeRule> {
    let pcfg = read_json_object(&paths.provider_config_json);
    let rules = pcfg
        .get("config")
        .and_then(|c| c.get("providerConfigRules"))
        .and_then(|r| r.get("providerRules"))
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::new();
    for rule in rules {
        let cfg = match rule.get("config") {
            Some(c) => c,
            None => continue,
        };
        if cfg.get("group").and_then(|v| v.as_str()) != Some("standard-personal") {
            continue;
        }
        let api_key = cfg
            .get("access")
            .and_then(|a| a.get("apiKey"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if api_key.is_empty() {
            continue;
        }
        let base_url = cfg
            .get("api")
            .and_then(|a| a.get("baseUrl"))
            .and_then(|v| v.as_str())
            .unwrap_or("https://api.z.ai/api/anthropic")
            .trim()
            .to_string();
        let provider_family = if base_url.contains("bigmodel.cn") {
            "bigmodel".to_string()
        } else {
            "zai".to_string()
        };
        let provider_id = rule
            .get("providerId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let provider_name = rule
            .get("providerName")
            .and_then(|v| v.as_str())
            .unwrap_or(&provider_id)
            .to_string();
        let enabled = rule.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false);
        out.push(ImportedZCodeRule {
            provider_id,
            provider_name,
            provider_family,
            api_key,
            base_url,
            enabled,
        });
    }
    out
}

/// Import any ZCode `standard-personal` API-key rules that are not yet in `vault`.
pub fn import_personal_rules_into_vault(
    paths: &ZCodePaths,
    vault: &mut UnifiedVault,
    vault_key: &[u8; 32],
) -> Result<usize, String> {
    let rules = extract_personal_rules(paths);
    let mut imported = 0usize;
    for r in rules {
        let dup = vault.accounts.iter().any(|acc| {
            acc.kind == "z_api"
                && acc
                    .extract_api_key(vault_key)
                    .map(|k| k == r.api_key)
                    .unwrap_or(false)
        });
        if dup {
            continue;
        }
        let rec = UnifiedAccount::from_z_api_key(
            &format!("{} (ZCode)", r.provider_name),
            &r.provider_family,
            &r.api_key,
            Some(&r.base_url),
            vault_key,
        )?;
        vault.accounts.push(rec);
        imported += 1;
    }
    Ok(imported)
}

const MANAGED_RULE_ID: &str = "harness-auth-active";

pub fn activate_zcode_account_at(
    paths: &ZCodePaths,
    account: &UnifiedAccount,
    vault_key: &[u8; 32],
) -> Result<(), String> {
    if !paths.v2_dir.exists() {
        std::fs::create_dir_all(&paths.v2_dir)
            .map_err(|e| format!("ZCode klasörü oluşturulamadı: {e}"))?;
    }

    let prov = if account.provider == "bigmodel" {
        "bigmodel"
    } else {
        "zai"
    };

    match account.kind.as_str() {
        "z_api" => {
            let api_key = account.extract_api_key(vault_key)?;
            if api_key.trim().is_empty() {
                return Err("Seçilen Z-API hesabında anahtar bulunamadı".into());
            }
            let base_url = account
                .base_url
                .as_deref()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or(if prov == "bigmodel" {
                    "https://open.bigmodel.cn/api/anthropic"
                } else {
                    "https://api.z.ai/api/anthropic"
                });

            let mut pcfg = read_json_object(&paths.provider_config_json);
            if !pcfg.contains_key("schemaVersion") {
                pcfg.insert("schemaVersion".into(), json!(1));
            }
            let cfg = pcfg
                .entry("config")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or("provider_config.json geçersiz")?;
            let rules_wrap = cfg
                .entry("providerConfigRules")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or("providerConfigRules geçersiz")?;
            let rules = rules_wrap
                .entry("providerRules")
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .ok_or("providerRules geçersiz")?;

            // Disable other standard-personal rules so only our chosen rule runs.
            let mut updated_existing = false;
            for item in rules.iter_mut() {
                let is_personal = item
                    .pointer("/config/group")
                    .and_then(|v| v.as_str())
                    == Some("standard-personal");
                let is_managed = item.get("providerId").and_then(|v| v.as_str())
                    == Some(MANAGED_RULE_ID);
                if is_managed {
                    if let Some(obj) = item.as_object_mut() {
                        obj.insert("providerName".into(), json!(account.name));
                        obj.insert("enabled".into(), json!(true));
                        obj.insert(
                            "config".into(),
                            build_personal_rule_config(&api_key, base_url),
                        );
                        updated_existing = true;
                    }
                } else if is_personal {
                    if let Some(obj) = item.as_object_mut() {
                        obj.insert("enabled".into(), json!(false));
                    }
                }
            }
            if !updated_existing {
                rules.insert(
                    0,
                    json!({
                        "providerId": MANAGED_RULE_ID,
                        "providerName": account.name,
                        "enabled": true,
                        "config": build_personal_rule_config(&api_key, base_url),
                    }),
                );
            }
            atomic_write(
                &paths.provider_config_json,
                &serde_json::to_string_pretty(&pcfg).map_err(|e| e.to_string())?,
            )?;

            let mut setting = read_json_object(&paths.setting_json);
            let modes = setting
                .entry("modelProviderFamilyModes")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or("modelProviderFamilyModes geçersiz")?;
            modes.insert(prov.into(), json!("api-key"));
            let selected = setting
                .entry("modelProviderFamilySelectedKeys")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or("modelProviderFamilySelectedKeys geçersiz")?;
            selected.insert(prov.into(), json!(format!("standard-personal:{MANAGED_RULE_ID}")));
            atomic_write(
                &paths.setting_json,
                &serde_json::to_string_pretty(&setting).map_err(|e| e.to_string())?,
            )?;
            Ok(())
        }
        "zcode_oauth" => {
            disable_personal_rules_at(paths)?;
            let plan = account.default_plan.as_deref().unwrap_or("coding-plan");
            let selected_key = if plan == "start-plan" {
                format!("start-plan:builtin:{prov}-start-plan")
            } else {
                format!("coding-plan:builtin:{prov}-coding-plan")
            };
            let mut setting = read_json_object(&paths.setting_json);
            let modes = setting
                .entry("modelProviderFamilyModes")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or("modelProviderFamilyModes geçersiz")?;
            modes.insert(prov.into(), json!("oauth"));
            let selected = setting
                .entry("modelProviderFamilySelectedKeys")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or("modelProviderFamilySelectedKeys geçersiz")?;
            selected.insert(prov.into(), json!(selected_key));
            atomic_write(
                &paths.setting_json,
                &serde_json::to_string_pretty(&setting).map_err(|e| e.to_string())?,
            )?;
            Ok(())
        }
        other => Err(format!("ZCode yalnızca zcode_oauth ve z_api destekler (verilen: {other})")),
    }
}

pub fn disable_personal_rules_at(paths: &ZCodePaths) -> Result<(), String> {
    if !paths.provider_config_json.is_file() {
        return Ok(());
    }
    let mut pcfg = read_json_object(&paths.provider_config_json);
    if let Some(rules) = pcfg
        .get_mut("config")
        .and_then(|c| c.get_mut("providerConfigRules"))
        .and_then(|r| r.get_mut("providerRules"))
        .and_then(|a| a.as_array_mut())
    {
        for item in rules.iter_mut() {
            let is_personal = item
                .pointer("/config/group")
                .and_then(|v| v.as_str())
                == Some("standard-personal");
            if is_personal {
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("enabled".into(), json!(false));
                }
            }
        }
        atomic_write(
            &paths.provider_config_json,
            &serde_json::to_string_pretty(&pcfg).map_err(|e| e.to_string())?,
        )?;
    }
    Ok(())
}

pub fn disable_at(paths: &ZCodePaths) -> Result<(), String> {
    disable_personal_rules_at(paths)?;
    if paths.setting_json.is_file() {
        let mut setting = read_json_object(&paths.setting_json);
        if let Some(modes) = setting
            .get_mut("modelProviderFamilyModes")
            .and_then(|v| v.as_object_mut())
        {
            modes.insert("zai".into(), json!("oauth"));
        }
        atomic_write(
            &paths.setting_json,
            &serde_json::to_string_pretty(&setting).map_err(|e| e.to_string())?,
        )?;
    }
    Ok(())
}

fn build_personal_rule_config(api_key: &str, base_url: &str) -> Value {
    json!({
        "group": "standard-personal",
        "access": {
            "type": "api-key",
            "apiKey": api_key,
        },
        "api": {
            "type": "anthropic-messages",
            "baseUrl": base_url,
        },
        "personalModelIds": ["GLM-5.3", "GLM-5.2", "GLM-5-Turbo"],
        "modelOrder": ["GLM-5.3", "GLM-5.2", "GLM-5-Turbo"],
    })
}

pub fn launch_zcode_app() -> Result<(), String> {
    let exe = detect_zcode_exe()
        .ok_or_else(|| "ZCode masaüstü uygulaması (`ZCode.exe`) bulunamadı".to_string())?;
    Command::new(exe)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("ZCode başlatılamadı: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto;

    fn temp_home(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "harness-auth-zc-{tag}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(dir.join(".zcode").join("v2")).unwrap();
        dir
    }

    #[test]
    fn zcode_inspect_import_activate_and_disable() {
        let home = temp_home("flow");
        let paths = ZCodePaths::in_home(&home);
        atomic_write(
            &paths.provider_config_json,
            &serde_json::to_string_pretty(&json!({
                "schemaVersion": 1,
                "config": {
                    "providerConfigRules": {
                        "providerRules": [{
                            "providerId": "rule-1",
                            "providerName": "z ai test",
                            "enabled": true,
                            "config": {
                                "group": "standard-personal",
                                "access": { "type": "api-key", "apiKey": "id1.secret999" },
                                "api": { "type": "anthropic-messages", "baseUrl": "https://api.z.ai/api/anthropic" },
                                "personalModelIds": ["GLM-5.2"]
                            }
                        }]
                    }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        atomic_write(
            &paths.setting_json,
            &serde_json::to_string_pretty(&json!({
                "modelProviderFamilyModes": { "zai": "api-key" }
            }))
            .unwrap(),
        )
        .unwrap();

        let live = inspect_live_at(&paths, None);
        assert!(live.v2_exists);
        assert_eq!(live.zai_mode, "api-key");
        assert_eq!(live.active_personal_name.as_deref(), Some("z ai test"));

        let key = crypto::derive_vault_key("test");
        let mut vault = UnifiedVault::default();
        let imported = import_personal_rules_into_vault(&paths, &mut vault, &key).unwrap();
        assert_eq!(imported, 1);
        assert_eq!(vault.accounts.len(), 1);

        let api_acc = UnifiedAccount::from_z_api_key(
            "Yeni Key",
            "zai",
            "newid.newsec",
            None,
            &key,
        )
        .unwrap();
        activate_zcode_account_at(&paths, &api_acc, &key).unwrap();

        let live2 = inspect_live_at(&paths, None);
        assert_eq!(live2.active_personal_name.as_deref(), Some("Yeni Key"));
        assert_eq!(live2.zai_mode, "api-key");

        let oauth_acc = UnifiedAccount::from_zcode_credential(
            "OAuth Hesap",
            "zcode_oauth",
            &json!({ "provider": "zai", "apiKey": "oauth-key", "jwt": "jwt-123" }),
            None,
            &key,
        )
        .unwrap();
        activate_zcode_account_at(&paths, &oauth_acc, &key).unwrap();
        let live3 = inspect_live_at(&paths, None);
        assert_eq!(live3.zai_mode, "oauth");
        assert!(live3.active_personal_name.is_none());

        let _ = std::fs::remove_dir_all(&home);
    }
}
