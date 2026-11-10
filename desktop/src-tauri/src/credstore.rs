//! Read/write the proxy's encrypted credential store
//! (`<store_dir>/credentials.json`, format `{"encrypted": base64(iv||ct)}`),
//! byte-compatible with `src/auth/store.ts`.

use std::path::Path;

use crate::crypto;

pub fn write_credential(store_dir: &Path, secret: &str, cred_json: &str) -> Result<(), String> {
    std::fs::create_dir_all(store_dir).map_err(|e| format!("mkdir failed: {e}"))?;
    let key = crypto::derive_key(secret);
    let sealed = crypto::seal(&key, cred_json)?;
    let file = store_dir.join("credentials.json");
    let tmp = store_dir.join("credentials.json.tmp-manager");
    std::fs::write(&tmp, serde_json::json!({ "encrypted": sealed }).to_string())
        .map_err(|e| format!("write failed: {e}"))?;
    std::fs::rename(&tmp, &file).map_err(|e| format!("rename failed: {e}"))
}

pub fn read_credential(store_dir: &Path, secret: &str) -> Result<Option<serde_json::Value>, String> {
    let file = store_dir.join("credentials.json");
    let text = match std::fs::read_to_string(&file) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    let parsed: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("store file corrupt: {e}"))?;
    let enc = parsed
        .get("encrypted")
        .and_then(|v| v.as_str())
        .ok_or("store file missing encrypted field")?;
    let key = crypto::derive_key(secret);
    let json = crypto::open(&key, enc)?;
    let cred = serde_json::from_str(&json).map_err(|e| format!("credential JSON invalid: {e}"))?;
    Ok(Some(cred))
}

pub fn clear_credential(store_dir: &Path) {
    let _ = std::fs::remove_file(store_dir.join("credentials.json"));
    let _ = std::fs::remove_file(store_dir.join("credentials.json.tmp-manager"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_roundtrip() {
        let dir = std::env::temp_dir().join(format!("zcm-store-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(read_credential(&dir, "s").unwrap().is_none());
        let cred = r#"{"apiKey":"abc","provider":"zai"}"#;
        write_credential(&dir, "s1", cred).unwrap();
        assert!(read_credential(&dir, "s2").is_err(), "wrong secret must fail");
        let back = read_credential(&dir, "s1").unwrap().unwrap();
        assert_eq!(back["apiKey"], "abc");
        clear_credential(&dir);
        assert!(read_credential(&dir, "s1").unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
