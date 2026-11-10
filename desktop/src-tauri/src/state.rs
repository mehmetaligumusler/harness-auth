//! Shared application state.

use std::sync::Mutex;

use crate::paths::Paths;
use crate::proxy::ManagedProxy;
use crate::settings::Settings;
use crate::oauth::OAuthState;
use crate::vault::VaultData;

pub struct AppState {
    pub paths: Paths,
    /// Master secret (Windows Credential Manager-backed).
    pub secret: String,
    pub settings: Mutex<Settings>,
    pub vault: Mutex<VaultData>,
    pub proxy: Mutex<ManagedProxy>,
    pub oauth: Mutex<OAuthState>,
}

impl AppState {
    pub fn vault_key(&self) -> [u8; 32] {
        crate::crypto::derive_vault_key(&self.secret)
    }
}
