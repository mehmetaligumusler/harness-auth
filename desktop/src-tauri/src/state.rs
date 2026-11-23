//! Shared application state.

use std::sync::Mutex;

use crate::harness::{HarnessOAuthState, SessionVault};
use crate::oauth::OAuthState;
use crate::paths::Paths;
use crate::proxy::ManagedProxy;
use crate::settings::Settings;
use crate::vault::VaultData;

pub struct AppState {
    pub paths: Paths,
    /// Master secret (Windows Credential Manager-backed).
    pub secret: String,
    pub settings: Mutex<Settings>,
    pub vault: Mutex<VaultData>,
    pub sessions: Mutex<SessionVault>,
    pub proxy: Mutex<ManagedProxy>,
    pub oauth: Mutex<OAuthState>,
    pub harness_oauth: Mutex<HarnessOAuthState>,
}

impl AppState {
    pub fn vault_key(&self) -> [u8; 32] {
        crate::crypto::derive_vault_key(&self.secret)
    }
}
