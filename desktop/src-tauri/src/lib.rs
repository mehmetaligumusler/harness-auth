//! HarnessAuth backend library.
//!
//! The crate is split into a library + thin `main.rs` so integration tests
//! (`tests/e2e.rs`) can drive the real sidecar child process through the same
//! modules the Tauri commands use.

pub mod claude;
pub mod commands;
pub mod credstore;
pub mod crypto;
pub mod embedded_engine;
pub mod harness;
pub mod http;
pub mod oauth;
pub mod panel;
pub mod paths;
pub mod proxy;
pub mod secret;
pub mod settings;
pub mod state;
pub mod tray;
pub mod unified_vault;
pub mod vault;
pub mod yamlpatch;
