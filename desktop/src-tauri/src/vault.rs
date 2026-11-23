//! Multi-account vault.
//!
//! The proxy natively holds exactly one credential (`credentials.json`). The
//! manager keeps every account here — each record carries the full proxy
//! `Credential` JSON as an AES-256-GCM blob — and materializes the *active*
//! account into the proxy's store on activation. Secrets never reach the
//! frontend; only masked hints do.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::crypto;

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRecord {
    pub id: String,
    pub name: String,
    /// "oauth" | "api-key"
    pub kind: String,
    /// "zai" | "bigmodel"
    pub provider: String,
    pub key_hint: String,
    pub user_hint: Option<String>,
    /// Whether the credential carries a start-plan JWT.
    pub has_jwt: bool,
    /// Whether the credential carries the Z.AI API-key secret half.
    pub has_secret: bool,
    /// AES-256-GCM blob of the full proxy Credential JSON.
    pub encrypted: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub default_plan: Option<String>,
    pub default_model: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub base_url: Option<String>,
}

/// What the frontend sees — everything except `encrypted`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub provider: String,
    pub key_hint: String,
    pub user_hint: Option<String>,
    pub has_jwt: bool,
    pub has_secret: bool,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub default_plan: Option<String>,
    pub default_model: Option<String>,
    pub enabled: bool,
    pub base_url: Option<String>,
    pub active: bool,
}

impl AccountRecord {
    pub fn summary(&self, active: bool) -> AccountSummary {
        AccountSummary {
            id: self.id.clone(),
            name: self.name.clone(),
            kind: self.kind.clone(),
            provider: self.provider.clone(),
            key_hint: self.key_hint.clone(),
            user_hint: self.user_hint.clone(),
            has_jwt: self.has_jwt,
            has_secret: self.has_secret,
            created_at: self.created_at,
            last_used_at: self.last_used_at,
            default_plan: self.default_plan.clone(),
            default_model: self.default_model.clone(),
            enabled: self.enabled,
            base_url: self.base_url.clone(),
            active,
        }
    }

    pub fn decrypt_credential(&self, vault_key: &[u8; 32]) -> Result<Value, String> {
        let json = crypto::open(vault_key, &self.encrypted)?;
        serde_json::from_str(&json).map_err(|e| format!("credential JSON invalid: {e}"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct VaultData {
    pub version: u32,
    pub active_id: Option<String>,
    pub accounts: Vec<AccountRecord>,
}

impl VaultData {
    pub fn load(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => VaultData::default(),
        }
    }

    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("mkdir failed: {e}"))?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| format!("write failed: {e}"))
    }

    pub fn active(&self) -> Option<&AccountRecord> {
        self.active_id
            .as_ref()
            .and_then(|id| self.accounts.iter().find(|a| &a.id == id && a.enabled))
    }

    pub fn summaries(&self) -> Vec<AccountSummary> {
        self.accounts
            .iter()
            .map(|a| a.summary(self.is_active(&a.id)))
            .collect()
    }

    pub fn is_active(&self, id: &str) -> bool {
        self.active_id.as_deref() == Some(id)
    }
}

/// One-time silent migration from legacy `%APPDATA%\zcode-manager\accounts.json`
/// into the new encrypted vault using the legacy master secret.
pub fn migrate_legacy_accounts(
    target_vault_path: &std::path::Path,
    new_vault_key: &[u8; 32],
) -> Option<VaultData> {
    if target_vault_path.exists() {
        return None;
    }
    let legacy_dir = crate::paths::Paths::legacy_zcode_dir()?;
    let legacy_vault_file = legacy_dir.join("accounts.json");
    if !legacy_vault_file.is_file() {
        return None;
    }
    let legacy_secret = crate::secret::read_legacy_zcode_secret()?;
    let legacy_key = crypto::derive_vault_key(&legacy_secret);
    let legacy = VaultData::load(&legacy_vault_file);
    if legacy.accounts.is_empty() {
        return None;
    }
    let mut migrated = VaultData {
        version: legacy.version.max(1),
        active_id: legacy.active_id.clone(),
        accounts: Vec::with_capacity(legacy.accounts.len()),
    };
    for mut rec in legacy.accounts {
        let cred = match rec.decrypt_credential(&legacy_key) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let raw_json = serde_json::to_string(&cred).ok()?;
        rec.encrypted = crypto::seal(new_vault_key, &raw_json).ok()?;
        rec.enabled = true;
        migrated.accounts.push(rec);
    }
    if migrated.accounts.is_empty() {
        return None;
    }
    let _ = migrated.save(target_vault_path);
    Some(migrated)
}

fn mask_key(api_key: &str) -> String {
    let k = api_key.trim();
    if k.len() <= 8 {
        return "•".repeat(k.len().max(4));
    }
    format!("{}…{}", &k[..6], &k[k.len() - 4..])
}

fn mask_user(user_id: &str) -> String {
    let u = user_id.trim();
    if u.len() <= 8 {
        return u.to_string();
    }
    format!("{}…", &u[..8])
}

/// Build a vault record from a full proxy Credential JSON object.
pub fn record_from_credential(
    name: &str,
    kind: &str,
    cred: &Value,
    vault_key: &[u8; 32],
) -> Result<AccountRecord, String> {
    let api_key = cred
        .get("apiKey")
        .and_then(|v| v.as_str())
        .ok_or("credential missing apiKey")?;
    let provider = cred
        .get("provider")
        .and_then(|v| v.as_str())
        .ok_or("credential missing provider")?
        .to_string();
    if provider != "zai" && provider != "bigmodel" {
        return Err(format!("unknown provider: {provider}"));
    }

    let json = serde_json::to_string(cred).map_err(|e| e.to_string())?;
    let encrypted = crypto::seal(vault_key, &json)?;

    Ok(AccountRecord {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
        kind: kind.to_string(),
        provider,
        key_hint: mask_key(api_key),
        user_hint: cred
            .get("userId")
            .and_then(|v| v.as_str())
            .map(mask_user),
        has_jwt: cred
            .get("jwt")
            .and_then(|v| v.as_str())
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        has_secret: cred
            .get("secret")
            .and_then(|v| v.as_str())
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        encrypted,
        created_at: chrono::Utc::now().timestamp_millis(),
        last_used_at: None,
        default_plan: None,
        default_model: None,
        enabled: true,
        base_url: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> [u8; 32] {
        crypto::derive_vault_key("test-master")
    }

    #[test]
    fn record_roundtrip_and_masking() {
        let cred: Value = serde_json::json!({
            "apiKey": "0123456789abcdef",
            "secret": "s",
            "provider": "zai",
            "userId": "user-123456789",
            "jwt": "eyJ..."
        });
        let rec = record_from_credential("Test", "oauth", &cred, &key()).unwrap();
        assert!(rec.has_jwt && rec.has_secret);
        assert!(rec.enabled);
        assert_eq!(rec.key_hint, "012345…cdef");
        assert_eq!(rec.user_hint.as_deref(), Some("user-123…"));
        let back = rec.decrypt_credential(&key()).unwrap();
        assert_eq!(back, cred);
    }

    #[test]
    fn missing_provider_rejected() {
        let cred: Value = serde_json::json!({"apiKey": "x"});
        assert!(record_from_credential("T", "api-key", &cred, &key()).is_err());
    }

    #[test]
    fn vault_active_lookup_and_disabled() {
        let mut v = VaultData::default();
        let mut rec = record_from_credential(
            "A",
            "api-key",
            &serde_json::json!({"apiKey": "k1234567890", "provider": "zai"}),
            &key(),
        )
        .unwrap();
        let id = rec.id.clone();
        rec.enabled = false;
        v.accounts.push(rec);
        v.active_id = Some(id.clone());
        assert!(v.active().is_none(), "disabled account must not be returned as active");
        v.accounts[0].enabled = true;
        assert_eq!(v.active().unwrap().id, id);
        assert!(v.is_active(&id));
    }
}
