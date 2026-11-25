//! End-to-end lifecycle tests against the REAL compiled sidecar binary
//! (`harness-zcode-engine.exe`). No mocks for the proxy: these tests spawn it as a
//! child exactly like the app does and drive the panel control API.
//!
//! - `proxy_lifecycle_e2e`: fake credential — validates process spawn, panel
//!   status/logs, server start/stop, config hot-patch and shutdown. No Z.AI
//!   network access is asserted to succeed (the fake key is rejected
//!   upstream, which is the expected, asserted behavior).
//! - `real_account_e2e` (#[ignore]): uses the host ZCode desktop config
//!   (`auth login zai --import`) to run a REAL quota + 1-token generation
//!   request. Requires a signed-in ZCode installation and network; enable
//!   with `ZCM_E2E_REAL=1 cargo test --test e2e -- --ignored`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use harness_auth::credstore;
use harness_auth::http;
use harness_auth::panel::PanelClient;
use harness_auth::paths::Paths;
use harness_auth::proxy::{self, ManagedProxy};
use harness_auth::settings::Settings;
use harness_auth::yamlpatch;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn proxy_exe(paths: &Paths) -> PathBuf {
    harness_auth::embedded_engine::resolve_or_extract(paths)
        .expect("extract embedded zcode engine")
}

struct TempApp {
    dir: PathBuf,
    paths: Paths,
    settings: Settings,
    secret: String,
}

impl TempApp {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "zcm-e2e-{}-{}-{}",
            tag,
            std::process::id(),
            chrono_stamp()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // Paths::new honors HARNESS_AUTH_DATA_DIR; cargo runs each test
        // binary in one process, and this suite serializes, so setting it is
        // safe here.
        std::env::set_var("HARNESS_AUTH_DATA_DIR", &dir);
        let paths = Paths::new();
        paths.ensure_all().unwrap();
        proxy::ensure_proxy_config(&paths).unwrap();
        // Pick a free server port (default 8317 may be taken).
        let port = proxy::free_port();
        proxy::patch_proxy_config(
            &paths,
            &[yamlpatch::update_under("server", "port", port.to_string())],
        )
        .unwrap();
        let mut settings = Settings::default();
        settings.panel_port = 0; // force spawn to pick a free panel port
        let secret = harness_auth::crypto::random_token();
        Self {
            dir,
            paths,
            settings,
            secret,
        }
    }

    fn panel(&self, managed: &ManagedProxy) -> PanelClient {
        PanelClient::new(managed.panel_port, &self.settings.panel_token)
    }

    fn spawn(&self, cred_json: &str) -> ManagedProxy {
        proxy::spawn_proxy(&proxy_exe(&self.paths), &self.paths, &self.settings, &self.secret, cred_json)
            .expect("proxy spawn")
    }

    fn server_port(&self) -> u16 {
        proxy::config_port(&self.paths)
    }

    fn proxy_key(&self) -> String {
        proxy::read_proxy_config(&self.paths)
            .unwrap()
            .pointer("/auth/proxyApiKey")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string()
    }
}

impl Drop for TempApp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
        std::env::remove_var("HARNESS_AUTH_DATA_DIR");
    }
}

fn chrono_stamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Kills the spawned proxy when dropped — even on assertion failure — so a
/// failing test never leaks proxy processes.
struct ProxyGuard {
    app: TempApp,
    managed: ManagedProxy,
}

impl Drop for ProxyGuard {
    fn drop(&mut self) {
        let _ = proxy::shutdown_proxy(&self.app.settings, &self.managed);
        // Belt & braces: force-kill if graceful shutdown did not land.
        if let Some(pid) = self.managed.child_pid {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .status();
        }
    }
}

#[test]
fn proxy_lifecycle_e2e() {
    let app = TempApp::new("lifecycle");

    // Fake credential — enough for `serve` to consider us logged in.
    let fake_cred = r#"{"apiKey":"e2e-fake-key-000000000000","provider":"zai"}"#;
    credstore::write_credential(&app.paths.proxy_store_dir(), &app.secret, fake_cred).unwrap();

    // --- spawn (guard kills the child even if an assert fails) ----------
    let managed = app.spawn(fake_cred);
    assert!(managed.panel_port > 0);
    assert!(managed.child_pid.is_some());
    let guard = ProxyGuard { app, managed: managed.clone() };
    let app = &guard.app;

    let panel = app.panel(&managed);
    assert!(panel.healthz(), "panel healthz after spawn");

    let st = panel.status().expect("status");
    assert_eq!(st["loggedIn"], serde_json::json!(true));
    assert_eq!(st["provider"], serde_json::json!("zai"));
    assert_eq!(st["plan"], serde_json::json!("coding-plan"));
    assert!(st["proxyPort"].as_u64().unwrap() > 0);

    // --- server health through the proxy port + client key --------------
    let port = app.server_port();
    let key = app.proxy_key();
    let health = http::get(
        &format!("http://127.0.0.1:{port}/health"),
        &[("x-api-key", key.as_str())],
        8,
    )
    .unwrap();
    assert_eq!(health.status, 200, "body: {}", health.body);

    // Client auth is enforced when a key is configured.
    let noauth = http::get(&format!("http://127.0.0.1:{port}/v1/models"), &[], 8).unwrap();
    assert_eq!(noauth.status, 401, "proxy must reject keyless clients");

    // --- logs ------------------------------------------------------------
    let logs = panel.logs(0).expect("logs");
    assert_eq!(logs["event"], serde_json::json!("logs"));
    assert!(logs["nextSince"].as_i64().unwrap() >= 0);

    // --- quota with fake key: upstream rejects, the snapshot degrades ----
    // gracefully (ok:true + errors[], no fabricated numbers).
    let quota = panel.quota().expect("quota envelope");
    assert_eq!(quota["event"], serde_json::json!("quota"));
    let inner = &quota["quota"];
    let errors = inner["errors"].as_array().expect("errors array");
    assert!(!errors.is_empty(), "fake credential must surface quota errors");
    assert!(
        inner["balances"].as_array().map(|a| a.is_empty()).unwrap_or(true)
            && inner["codingPlan"].is_null(),
        "no fabricated quota data for a fake credential: {inner}"
    );

    // --- server stop/start (process stays alive) -------------------------
    panel.stop_server().expect("stopProxy");
    let st = panel.status().unwrap();
    assert_eq!(st["proxyPort"], serde_json::json!(0));
    assert!(panel.healthz(), "panel survives server stop");

    panel.start_server().expect("startProxy");
    let st = panel.status().unwrap();
    assert!(st["proxyPort"].as_u64().unwrap() > 0);

    // --- config hot-patch (plan switch on a live process) ----------------
    // The panel's `status.plan` is a startup snapshot (proxy's ControlState);
    // the file watcher applies edits to live traffic. Our status layer reads
    // the file, and the process must survive the patch without a restart.
    proxy::patch_proxy_config(
        &app.paths,
        &[yamlpatch::update("plan", "start-plan"), yamlpatch::update("planAutoSwitch", "true")],
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    let cfg = proxy::read_proxy_config(&app.paths).unwrap();
    assert_eq!(cfg["plan"], serde_json::json!("start-plan"));
    assert_eq!(cfg["planAutoSwitch"], serde_json::json!(true));
    assert!(panel.healthz(), "process survives a config hot-patch");
    let st = panel.status().unwrap();
    assert!(
        st["proxyPort"].as_u64().unwrap() > 0,
        "server still up after hot-patch"
    );

    // --- graceful shutdown ------------------------------------------------
    proxy::shutdown_proxy(&app.settings, &managed).expect("shutdown");
    assert!(!panel.healthz(), "panel gone after shutdown");
}

#[test]
fn port_conflict_is_detected() {
    // Occupy a port, then try to patch the config to it — save_config in the
    // app refuses; here we assert the primitive: binding it fails.
    let port = proxy::free_port();
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
    assert!(proxy::port_in_use(port));
    drop(listener);
    assert!(!proxy::port_in_use(port));
}

#[test]
#[ignore = "uses the real Z.AI account from the host ZCode config; run explicitly"]
fn real_account_e2e() {
    if std::env::var("ZCM_E2E_REAL").ok().as_deref() != Some("1") {
        eprintln!("set ZCM_E2E_REAL=1 to run the real-account E2E test");
        return;
    }

    let app = TempApp::new("real");

    // Import the host ZCode credential through the proxy's own CLI.
    let mut cmd = Command::new(proxy_exe(&app.paths));
    cmd.args(["--cli", "auth", "login", "zai", "--import"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    proxy::child_env(&mut cmd, &app.paths, &app.settings, &app.secret, proxy::free_port());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let status = cmd.status().expect("run auth import");
    assert!(status.success(), "auth login --import failed: {status}");

    let cred = credstore::read_credential(&app.paths.proxy_store_dir(), &app.secret)
        .expect("read store")
        .expect("credential present after import");
    assert_eq!(cred["provider"], "zai");
    eprintln!("imported credential apiKey hint: {}", mask(&cred["apiKey"].as_str().unwrap_or("?")));

    let cred_json = serde_json::to_string(&cred).unwrap();
    let managed = app.spawn(&cred_json);
    let guard = ProxyGuard { app, managed: managed.clone() };
    let app = &guard.app;
    let panel = app.panel(&managed);

    // REAL quota snapshot through the panel.
    let quota = panel.quota().expect("real quota");
    eprintln!(
        "quota snapshot: codingPlan limits = {}, balances = {}",
        quota["quota"]["codingPlan"]["limits"].as_array().map(|a| a.len()).unwrap_or(0),
        quota["quota"]["balances"].as_array().map(|a| a.len()).unwrap_or(0),
    );
    assert!(
        quota["quota"]["codingPlan"]["limits"].as_array().map(|a| !a.is_empty()).unwrap_or(false)
            || quota["quota"]["balances"].as_array().map(|a| !a.is_empty()).unwrap_or(false),
        "expected real coding-plan limits or start-plan balances"
    );

    // REAL 1-token generation through the proxy.
    let port = app.server_port();
    let key = app.proxy_key();
    let body = serde_json::json!({
        "model": "glm-5.3",
        "max_tokens": 8,
        "messages": [{ "role": "user", "content": "Sadece 'merhaba' de." }]
    });
    let res = http::post_json(
        &format!("http://127.0.0.1:{port}/v1/messages"),
        &[("x-api-key", key.as_str())],
        &body,
        120,
    )
    .expect("request through proxy");
    assert_eq!(res.status, 200, "body: {}", res.body);
    let parsed: serde_json::Value = serde_json::from_str(&res.body).unwrap();
    assert!(parsed["content"].is_array() || parsed["usage"].is_object(), "model responded");
    eprintln!(
        "real generation ok: usage = {}",
        parsed["usage"].to_string().chars().take(120).collect::<String>()
    );

    proxy::shutdown_proxy(&app.settings, &managed).expect("shutdown");
}

fn mask(s: &str) -> String {
    if s.len() <= 10 {
        "•".repeat(4)
    } else {
        format!("{}…", &s[..6])
    }
}

// Silence dead-code path warning for repo_root used by both tests.
#[allow(dead_code)]
fn _touch(_: &Path) {}
