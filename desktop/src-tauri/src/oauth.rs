//! OAuth login flow via the proxy CLI child (`zcode-proxy --cli auth login`).
//!
//! The CLI prints the authorize URL on stdout and opens the default browser
//! itself; it then polls the provider server-side and writes the resolved
//! credential into the store (`ZCODE_PROXY_STORE_DIR`). We watch the child,
//! surface the URL to the UI, and on success import the credential into the
//! account vault — restoring the previously-active account's store file so a
//! running proxy process keeps its in-memory credential consistent with disk.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const LOGIN_TIMEOUT: Duration = Duration::from_secs(320);

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OAuthState {
    pub running: bool,
    pub provider: String,
    pub authorize_url: Option<String>,
    pub started_at: Option<i64>,
    #[serde(skip)]
    pub child_pid: Option<u32>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthDone {
    pub ok: bool,
    pub provider: String,
    pub account_id: Option<String>,
    pub error: Option<String>,
}

pub fn start(app: &AppHandle, provider: &str, name_hint: Option<String>) -> Result<(), String> {
    if provider != "zai" && provider != "bigmodel" {
        return Err(format!("bilinmeyen sağlayıcı: {provider}"));
    }
    {
        let state = app.state::<AppState>();
        let mut flow = state.oauth.lock().unwrap();
        if flow.running {
            return Err("Zaten devam eden bir giriş akışı var".into());
        }
        flow.running = true;
        flow.provider = provider.to_string();
        flow.authorize_url = None;
        flow.started_at = Some(chrono::Utc::now().timestamp_millis());
    }

    let app = app.clone();
    let provider = provider.to_string();
    let name_hint = name_hint.unwrap_or_default();
    std::thread::spawn(move || run_flow(app, provider, name_hint));
    Ok(())
}

pub fn cancel(app: &AppHandle) {
    let pid = {
        let state = app.state::<AppState>();
        let mut flow = state.oauth.lock().unwrap();
        let pid = flow.child_pid.take();
        flow.running = false;
        pid
    };
    if let Some(pid) = pid {
        crate::proxy::kill_pid(pid);
    }
}

fn run_flow(app: AppHandle, provider: String, name_hint: String) {
    let result = run_flow_inner(&app, &provider, &name_hint);
    let done = match result {
        Ok(account_id) => OAuthDone {
            ok: true,
            provider,
            account_id,
            error: None,
        },
        Err(e) => OAuthDone {
            ok: false,
            provider,
            account_id: None,
            error: Some(e),
        },
    };
    {
        let state = app.state::<AppState>();
        state.oauth.lock().unwrap().running = false;
    }
    let _ = app.emit("oauth://done", &done);
}

fn run_flow_inner(app: &AppHandle, provider: &str, name_hint: &str) -> Result<Option<String>, String> {
    let (exe, paths, settings, secret) = {
        let state = app.state::<AppState>();
        let settings = state.settings.lock().unwrap().clone();
        let paths = state.paths.clone();
        let secret = state.secret.clone();
        (crate::paths::resolve_proxy_exe(), paths, settings, secret)
    };

    crate::proxy::ensure_proxy_config(&paths)?;

    // Snapshot the currently-active store so we can restore it afterwards.
    let snapshot: Option<String> = crate::credstore::read_credential(&paths.proxy_store_dir(), &secret)
        .ok()
        .flatten()
        .and_then(|v| serde_json::to_string(&v).ok());

    let mut cmd = Command::new(&exe);
    cmd.args(["--cli", "auth", "login", provider])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    crate::proxy::child_env(&mut cmd, &paths, &settings, &secret, settings.panel_port);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("giriş süreci başlatılamadı: {e}"))?;
    let pid = child.id();
    {
        let state = app.state::<AppState>();
        state.oauth.lock().unwrap().child_pid = Some(pid);
    }

    // Read stdout for the authorize URL (child prints "  https://...").
    let mut authorize_url: Option<String> = None;
    let mut tail: Vec<String> = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = line.trim().to_string();
                    if !trimmed.is_empty() {
                        tail.push(trimmed.clone());
                        if tail.len() > 40 {
                            tail.remove(0);
                        }
                    }
                    if authorize_url.is_none()
                        && trimmed.starts_with("http")
                        && trimmed.contains("://")
                    {
                        authorize_url = Some(trimmed);
                        {
                            let state = app.state::<AppState>();
                            let mut flow = state.oauth.lock().unwrap();
                            flow.authorize_url = authorize_url.clone();
                        }
                        let _ = app.emit("oauth://url", authorize_url.clone().unwrap_or_default());
                    }
                }
                Err(_) => break,
            }
        }
    }

    // Bounded wait for the child to exit.
    let deadline = Instant::now() + LOGIN_TIMEOUT;
    let code = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {
                if Instant::now() > deadline {
                    crate::proxy::kill_pid(pid);
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(e) => return Err(format!("giriş süreci izlenemedi: {e}")),
        }
    };

    let restore = |app: &AppHandle| {
        let state = app.state::<AppState>();
        match &snapshot {
            Some(json) => {
                let _ = crate::credstore::write_credential(
                    &state.paths.proxy_store_dir(),
                    &state.secret,
                    json,
                );
            }
            None => crate::credstore::clear_credential(&state.paths.proxy_store_dir()),
        }
    };

    if code != Some(0) {
        restore(app);
        let reason = if code.is_none() {
            "Zaman aşımı (5 dk)".to_string()
        } else {
            tail.last().cloned().unwrap_or_else(|| "giriş tamamlanamadı".into())
        };
        return Err(format!("Giriş başarısız: {reason}"));
    }

    // Success — import the fresh credential into the vault.
    let imported = crate::credstore::read_credential(&paths.proxy_store_dir(), &secret)
        .map_err(|e| format!("giriş sonrası kimlik okunamadı: {e}"))?;

    let Some(cred) = imported else {
        restore(app);
        return Err("Giriş tamamlandı ama kimlik bulunamadı".into());
    };

    let state = app.state::<AppState>();
    let vault_key = crate::crypto::derive_vault_key(&state.secret);
    let mut vault = state.vault.lock().unwrap();
    let default_name = format!(
        "{} OAuth {}",
        if provider == "zai" { "Z.AI" } else { "BigModel" },
        vault.accounts.len() + 1
    );
    let name = if name_hint.trim().is_empty() {
        default_name
    } else {
        name_hint.trim().to_string()
    };
    let record =
        crate::vault::record_from_credential(&name, "oauth", &cred, &vault_key)?;
    let account_id = record.id.clone();
    let was_empty = vault.accounts.is_empty();
    vault.accounts.push(record);
    if was_empty {
        vault.active_id = Some(account_id.clone());
    }
    vault
        .save(&state.paths.vault_file())
        .map_err(|e| format!("kasa kaydedilemedi: {e}"))?;
    drop(vault);

    // First account ever: keep the store as-is (it IS the active account).
    if !was_empty {
        restore(app);
    }

    Ok(Some(account_id))
}
