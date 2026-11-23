//! Proxy child-process management.
//!
//! The manager spawns the compiled `harness-zcode-engine.exe --cli serve` as a child
//! with an isolated, manager-owned environment:
//!
//! - `ZCODE_PROXY_CONFIG`    -> `%APPDATA%\harness-auth\proxy\config.yaml`
//! - `ZCODE_PROXY_STORE_DIR` -> `%APPDATA%\harness-auth\proxy\store`
//! - `ZCODE_PROXY_CREDENTIAL_SECRET` -> the manager master secret
//! - `ZCODE_PANEL_ENABLED/TOKEN/PORT/HOST` -> loopback control panel
//! - `ZCODE_ERROR_LOG`, `ZCODE_LOG_FORMAT=compact`
//!
//! Lifecycle model: the *process* is the unit the manager owns; the *server*
//! inside it can be started/stopped via panel commands (`startProxy` /
//! `stopProxy`) without killing the process, which keeps quota/logs available.

use std::io::Read;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::panel::PanelClient;
use crate::paths::Paths;
use crate::settings::Settings;
use crate::yamlpatch::{self, YamlUpdate};

pub const DEFAULT_PROXY_PORT: u16 = 8317;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// First-run proxy config template. Mirrors the upstream defaults from
/// `src/config/template.ts` with manager-safe values (loopback bind +
/// generated client key).
pub(crate) fn template(port: u16, host: &str, api_key: &str) -> String {
    format!(
        r#"# HarnessAuth tarafindan yonetilen ZCode arka plan servis yapilandirmasi.
# Proxy bu dosyayi okur; identity.deviceMid gibi alanlari kendisi yazar.
server:
  port: {port}
  host: "{host}"

auth:
  # Istemcilerin (Claude Code vb.) proxy'ye erismek icin kullanacagi anahtar.
  proxyApiKey: "{api_key}"

provider: zai
plan: coding-plan

# Otomatik plan gecisi (planPriority sirasina gore).
planAutoSwitch: false

providers:
  zai:
    anthropicBase: "https://api.z.ai/api/anthropic"
    openaiBase: "https://api.z.ai/api/coding/paas/v4"
  bigmodel:
    anthropicBase: "https://open.bigmodel.cn/api/anthropic"
    openaiBase: "https://open.bigmodel.cn/api/coding/paas/v4"

defaultModel: glm-5.3

models:
  - glm-5.3
  - glm-5.3-flash

identity:
  appVersion: "3.15.1"
  sourceTitle: "cli"
  refererOrigin: "https://zcode.z.ai"
  deviceMid: ""

clientIdentity:
  mode: enforce
  ttlSeconds: 900
  maxSessions: 1024

endpointRouting:
  enabled: true
  origin: "https://zcode.z.ai"

clientSigning:
  enabled: true

highspeed:
  enabled: true
  origin: "https://zcode.z.ai"
  models: ["glm-5.3", "glm-5.2", "glm-5.1"]

logging:
  level: info
"#
    )
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedProxy {
    pub child_pid: Option<u32>,
    pub panel_port: u16,
    pub adopted: bool,
    pub started_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyStatus {
    pub process_alive: bool,
    pub adopted: bool,
    pub panel_port: u16,
    pub server_running: bool,
    pub server_port: Option<u16>,
    pub provider: Option<String>,
    pub plan: Option<String>,
    pub logged_in: bool,
    pub configured_port: u16,
    pub endpoint: String,
}

pub fn ensure_proxy_config(paths: &Paths) -> Result<(), String> {
    let cfg = paths.proxy_config();
    if cfg.exists() {
        return Ok(());
    }
    if let Some(dir) = cfg.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("mkdir failed: {e}"))?;
    }
    let key = crate::crypto::random_token();
    std::fs::write(&cfg, template(DEFAULT_PROXY_PORT, "127.0.0.1", &key))
        .map_err(|e| format!("config yazılamadı: {e}"))
}

pub fn read_proxy_config(paths: &Paths) -> Result<serde_json::Value, String> {
    let text = std::fs::read_to_string(paths.proxy_config())
        .map_err(|e| format!("config okunamadı: {e}"))?;
    let yaml: serde_yaml::Value =
        serde_yaml::from_str(&text).map_err(|e| format!("config çözümlenemedi: {e}"))?;
    serde_json::to_value(&yaml).map_err(|e| e.to_string())
}

pub fn patch_proxy_config(paths: &Paths, updates: &[YamlUpdate]) -> Result<(), String> {
    let cfg = paths.proxy_config();
    let text = std::fs::read_to_string(&cfg).map_err(|e| format!("config okunamadı: {e}"))?;
    let patched = yamlpatch::patch_yaml(&text, updates);
    // Validate before writing so a bad patch can never corrupt the file.
    serde_yaml::from_str::<serde_yaml::Value>(&patched)
        .map_err(|e| format!("yama sonrası config geçersiz: {e}"))?;
    let tmp = cfg.with_extension("yaml.tmp-manager");
    std::fs::write(&tmp, &patched).map_err(|e| format!("config yazılamadı: {e}"))?;
    std::fs::rename(&tmp, &cfg).map_err(|e| format!("config taşınamadı: {e}"))
}

pub fn config_port(paths: &Paths) -> u16 {
    read_proxy_config(paths)
        .ok()
        .and_then(|v| {
            v.pointer("/server/port")
                .and_then(|p| p.as_u64())
                .map(|p| p as u16)
        })
        .unwrap_or(DEFAULT_PROXY_PORT)
}

pub fn port_in_use(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_err()
}

pub fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .unwrap_or(0)
}

pub fn tail_file(path: &Path, max_lines: usize) -> Vec<String> {
    let mut text = String::new();
    if let Ok(mut f) = std::fs::File::open(path) {
        let _ = f.read_to_string(&mut text);
    }
    let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
    if lines.len() > max_lines {
        lines.drain(..lines.len() - max_lines);
    }
    lines
}

/// Env shared by every proxy child (serve + auth login).
pub fn child_env(
    cmd: &mut Command,
    paths: &Paths,
    settings: &Settings,
    secret: &str,
    panel_port: u16,
) {
    cmd.env("ZCODE_PROXY_CONFIG", paths.proxy_config())
        .env("ZCODE_PROXY_STORE_DIR", paths.proxy_store_dir())
        .env("ZCODE_PROXY_CREDENTIAL_SECRET", secret)
        .env("ZCODE_ERROR_LOG", paths.proxy_errors_log())
        .env("ZCODE_LOG_FORMAT", "compact")
        .env("ZCODE_PANEL_ENABLED", "1")
        .env("ZCODE_PANEL_TOKEN", &settings.panel_token)
        .env("ZCODE_PANEL_PORT", panel_port.to_string())
        .env("ZCODE_PANEL_HOST", "127.0.0.1");
}

/// Spawn the proxy process and wait until the panel + server are up.
#[allow(clippy::too_many_arguments)]
pub fn spawn_proxy(
    exe: &Path,
    paths: &Paths,
    settings: &Settings,
    secret: &str,
    cred_json: &str,
) -> Result<ManagedProxy, String> {
    ensure_proxy_config(paths)?;

    crate::credstore::write_credential(&paths.proxy_store_dir(), secret, cred_json)?;

    // Panel port: prefer the persisted one; if something else squats on it,
    // fall back to a free port and remember it.
    let mut panel_port = settings.panel_port;
    if panel_port == 0 || port_in_use(panel_port) && !PanelClient::new(panel_port, &settings.panel_token).healthz() {
        panel_port = free_port();
    }

    let stdout_log = paths.proxy_stdout_log();
    if let Some(dir) = stdout_log.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let log_file = std::fs::File::create(&stdout_log)
        .map_err(|e| format!("stdout log açılamadı: {e}"))?;
    let log_err = log_file
        .try_clone()
        .map_err(|e| format!("stdout log açılamadı: {e}"))?;

    let mut cmd = Command::new(exe);
    cmd.arg("--cli").arg("serve")
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(log_err));
    child_env(&mut cmd, paths, settings, secret, panel_port);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("proxy süreci başlatılamadı ({}): {e}", exe.display()))?;

    let pid = child.id();
    // The child is independent: drop the handle so we never wait on it.
    drop(child);

    let panel = PanelClient::new(panel_port, &settings.panel_token);
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        if panel.healthz() {
            // Give the server a moment to report its port.
            for _ in 0..15 {
                if let Ok(st) = panel.status() {
                    let port = st.get("proxyPort").and_then(|v| v.as_u64()).unwrap_or(0);
                    if port > 0 {
                        return Ok(ManagedProxy {
                            child_pid: Some(pid),
                            panel_port,
                            adopted: false,
                            started_at: Some(chrono::Utc::now().timestamp_millis()),
                        });
                    }
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            // Panel up but server did not bind — surface the proxy's own log.
            kill_pid(pid);
            let tail = tail_file(&stdout_log, 25).join("\n");
            return Err(format!(
                "Proxy paneli ayakta ama sunucu başlamadı (port kapalı olabilir):\n{tail}"
            ));
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    kill_pid(pid);
    let tail = tail_file(&stdout_log, 25).join("\n");
    Err(format!(
        "Proxy paneline 20 saniye içinde ulaşılamadı:\n{tail}"
    ))
}

/// Adopt a proxy process that survived a previous app session.
pub fn adopt_proxy(settings: &Settings) -> Option<ManagedProxy> {
    let panel = PanelClient::new(settings.panel_port, &settings.panel_token);
    if !panel.healthz() {
        return None;
    }
    // healthz is tokenless; require a valid token before adopting.
    panel.status().ok()?;
    Some(ManagedProxy {
        child_pid: None,
        panel_port: settings.panel_port,
        adopted: true,
        started_at: None,
    })
}

pub fn status(paths: &Paths, settings: &Settings, managed: &ManagedProxy) -> ProxyStatus {
    let configured_port = config_port(paths);
    let endpoint = format!("http://127.0.0.1:{configured_port}");
    // The panel's `status` snapshots provider/plan at process start; the file
    // watcher applies config edits to live traffic within ~300 ms. The config
    // file is therefore the source of truth for these two fields.
    let file_provider_plan = read_proxy_config(paths).ok().and_then(|c| {
        let p = c.get("provider").and_then(|v| v.as_str()).map(String::from);
        let pl = c.get("plan").and_then(|v| v.as_str()).map(String::from);
        (p.is_some() || pl.is_some()).then_some((p, pl))
    });
    let mut out = ProxyStatus {
        process_alive: false,
        adopted: managed.adopted,
        panel_port: managed.panel_port,
        server_running: false,
        server_port: None,
        provider: None,
        plan: None,
        logged_in: false,
        configured_port,
        endpoint,
    };
    if managed.panel_port == 0 {
        // Also probe the persisted port in case we crashed before saving.
        let panel = PanelClient::new(settings.panel_port, &settings.panel_token);
        if panel.healthz() && panel.status().is_ok() {
            out.process_alive = true;
            out.panel_port = settings.panel_port;
        }
        return out;
    }
    let panel = PanelClient::new(managed.panel_port, &settings.panel_token);
    if !panel.healthz() {
        return out;
    }
    out.process_alive = true;
    if let Ok(st) = panel.status() {
        let port = st.get("proxyPort").and_then(|v| v.as_u64()).unwrap_or(0);
        out.server_running = port > 0;
        out.server_port = if port > 0 { Some(port as u16) } else { None };
        out.logged_in = st.get("loggedIn").and_then(|v| v.as_bool()).unwrap_or(false);
    }
    if let Some((p, pl)) = file_provider_plan {
        out.provider = p.or(out.provider.take());
        out.plan = pl.or(out.plan.take());
    }
    out
}

/// Graceful stop: panel `shutdown`, then taskkill as a fallback.
pub fn shutdown_proxy(settings: &Settings, managed: &ManagedProxy) -> Result<(), String> {
    let panel = PanelClient::new(managed.panel_port, &settings.panel_token);
    let _ = panel.shutdown();
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if !panel.healthz() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    if let Some(pid) = managed.child_pid {
        kill_pid(pid);
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if !panel.healthz() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        return Err("proxy süreci kapanmadı".into());
    }
    // Adopted process: token-authenticated shutdown already sent above.
    Ok(())
}

pub fn kill_pid(pid: u32) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_is_valid_yaml_with_safe_defaults() {
        let t = template(8317, "127.0.0.1", "abc123");
        let v: serde_yaml::Value = serde_yaml::from_str(&t).unwrap();
        assert_eq!(v["server"]["port"].as_u64(), Some(8317));
        assert_eq!(v["server"]["host"].as_str(), Some("127.0.0.1"));
        assert_eq!(v["auth"]["proxyApiKey"].as_str(), Some("abc123"));
        assert_eq!(v["provider"].as_str(), Some("zai"));
        assert_eq!(
            v["providers"]["zai"]["anthropicBase"].as_str(),
            Some("https://api.z.ai/api/anthropic")
        );
        assert_eq!(v["highspeed"]["enabled"].as_bool(), Some(true));
    }

    #[test]
    fn free_port_is_bindable() {
        let p = free_port();
        assert!(p > 0);
        assert!(!port_in_use(p));
    }
}
