//! ZCode Manager backend library.
//!
//! The crate is split into a library + thin `main.rs` so integration tests
//! (`tests/e2e.rs`) can drive the real proxy child process through the same
//! modules the Tauri commands use.

pub mod claude;
pub mod commands;
pub mod credstore;
pub mod crypto;
pub mod http;
pub mod oauth;
pub mod panel;
pub mod paths;
pub mod proxy;
pub mod secret;
pub mod settings;
pub mod state;
pub mod tray;
pub mod vault;
pub mod yamlpatch;
