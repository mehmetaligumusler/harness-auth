//! Master secret for the account vault and the proxy credential store.
//!
//! The random secret is generated once and stored in Windows Credential
//! Manager (DPAPI-backed per-user store) via the `keyring` crate. It is
//! injected into every proxy child as `ZCODE_PROXY_CREDENTIAL_SECRET`, so the
//! manager can read/write the proxy's encrypted credential store and the
//! store never depends on machine-derived keys.

const SERVICE: &str = "ZCodeManager";
const ACCOUNT: &str = "master-key";

pub fn load_or_create() -> Result<String, String> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| format!("keyring: {e}"))?;
    match entry.get_password() {
        Ok(secret) => {
            if secret.len() >= 32 {
                return Ok(secret);
            }
            // Unusable/legacy value — replace it.
            let fresh = crate::crypto::random_token();
            entry
                .set_password(&fresh)
                .map_err(|e| format!("keyring update: {e}"))?;
            Ok(fresh)
        }
        Err(keyring::Error::NoEntry) => {
            let fresh = crate::crypto::random_token();
            entry
                .set_password(&fresh)
                .map_err(|e| format!("keyring set: {e}"))?;
            Ok(fresh)
        }
        Err(e) => Err(format!("keyring get: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cross-process persistence check: run twice —
    ///   1st: `cargo test keyring_persistence`            (writes)
    ///   2nd: `ZCM_KR_VERIFY=<value> cargo test keyring_persistence` (reads)
    /// The second invocation proves the secret survives process death.
    #[test]
    fn keyring_persistence() {
        let entry = keyring::Entry::new("ZCodeManagerTests", "roundtrip").unwrap();
        if let Ok(expected) = std::env::var("ZCM_KR_VERIFY") {
            let got = entry.get_password().expect("secret must survive process restart");
            assert_eq!(got, expected, "keyring did not persist across processes");
        } else {
            let fresh = crate::crypto::random_token();
            entry.set_password(&fresh).expect("set_password");
            eprintln!("ZCM_KR_VERIFY={fresh}");
            assert_eq!(entry.get_password().unwrap(), fresh);
        }
    }
}
