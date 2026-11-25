//! Shared application state.

use std::sync::Mutex;

use crate::harness::openai_bridge::OpenAiBridgeState;
use crate::harness::HarnessOAuthState;
use crate::oauth::OAuthState;
use crate::paths::Paths;
use crate::proxy::ManagedProxy;
use crate::settings::Settings;
use crate::unified_vault::UnifiedVault;

pub struct AppState {
    pub paths: Paths,
    /// Master secret (Windows Credential Manager-backed).
    pub secret: String,
    pub settings: Mutex<Settings>,
    pub vault: Mutex<UnifiedVault>,
    pub proxy: Mutex<ManagedProxy>,
    pub oauth: Mutex<OAuthState>,
    pub harness_oauth: Mutex<HarnessOAuthState>,
    pub openai_bridge: OpenAiBridgeState,
}

impl AppState {
    pub fn vault_key(&self) -> [u8; 32] {
        crate::crypto::derive_vault_key(&self.secret)
    }
}
