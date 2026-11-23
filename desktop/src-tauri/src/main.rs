// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use tauri::{Manager, RunEvent, WindowEvent};

use harness_auth::{
    commands, crypto, harness, oauth, paths, proxy, secret, settings, state::AppState, tray, vault,
};

fn main() {
    // Loopback HTTP from the manager must never be routed through a system
    // proxy (ureq honors these env vars).
    for v in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        std::env::remove_var(v);
    }

    let app_state = init_state();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::save_settings,
            commands::reset_panel_identity,
            commands::add_api_key_account,
            commands::update_account,
            commands::set_account_enabled,
            commands::deactivate_account,
            commands::delete_account,
            commands::activate_account,
            commands::test_account,
            commands::start_oauth,
            commands::cancel_oauth,
            commands::proxy_start,
            commands::proxy_stop,
            commands::proxy_restart,
            commands::proxy_kill,
            commands::proxy_status,
            commands::proxy_test,
            commands::fetch_quota,
            commands::fetch_logs,
            commands::read_error_logs,
            commands::list_models,
            commands::save_model_prefs,
            commands::test_model,
            commands::get_config,
            commands::save_config,
            commands::claude_detect,
            commands::claude_launch,
            commands::push_recent_folder,
            commands::get_versions,
            // Multi-harness session commands
            commands::update_session,
            commands::set_session_enabled,
            commands::delete_session,
            commands::cancel_harness_oauth,
            commands::claude_capture_oauth,
            commands::claude_start_oauth_login,
            commands::claude_add_api_session,
            commands::claude_edit_session,
            commands::claude_set_method,
            commands::claude_launch_folder,
            commands::opencode_capture_current,
            commands::opencode_add_api_session,
            commands::opencode_add_zcode_session,
            commands::opencode_start_oauth_login,
            commands::opencode_activate_session,
            commands::opencode_set_method,
            commands::opencode_edit_session,
            commands::opencode_disable,
            commands::opencode_launch_folder,
            commands::antigravity_capture_current,
            commands::antigravity_start_oauth_login,
            commands::antigravity_activate_session,
            commands::antigravity_disable,
            commands::antigravity_launch_folder,
        ])
        .setup(|app| {
            let handle = app.handle();

            if let Err(e) = tray::create(handle) {
                eprintln!("tray init failed: {e}");
            }

            // Adopt a proxy left behind by a previous session.
            {
                let state = handle.state::<AppState>();
                let settings = state.settings.lock().unwrap().clone();
                if let Some(adopted) = proxy::adopt_proxy(&settings) {
                    *state.proxy.lock().unwrap() = adopted;
                }
            }

            // Hidden start (autostart with --hidden, or startMinimized).
            let start_hidden = std::env::args().any(|a| a == "--hidden")
                || handle
                    .state::<AppState>()
                    .settings
                    .lock()
                    .unwrap()
                    .start_minimized;
            if start_hidden {
                if let Some(win) = handle.get_webview_window("main") {
                    let _ = win.hide();
                }
            }

            // Optional proxy autostart (needs an active account).
            {
                let state = handle.state::<AppState>();
                let autostart = state.settings.lock().unwrap().proxy_autostart;
                let has_account = state.vault.lock().unwrap().active().is_some();
                if autostart && has_account {
                    let handle = handle.clone();
                    tauri::async_runtime::spawn(async move {
                        if commands::proxy_start(handle).await.is_err() {
                            eprintln!("proxy autostart failed");
                        }
                    });
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let state = app.state::<AppState>();
                let close_to_tray = state.settings.lock().unwrap().close_to_tray;
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                    notify_hidden_once(app);
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("HarnessAuth başlatılamadı")
        .run(|app, event| {
            if let RunEvent::Exit = event {
                cleanup(app);
            }
        });
}

fn init_state() -> AppState {
    let paths = paths::Paths::new();
    if let Err(e) = paths.ensure_all() {
        eprintln!("data dirs unavailable: {e}");
    }
    let settings = settings::Settings::load(&paths);
    let secret = secret::load_or_create().unwrap_or_else(|e| {
        eprintln!("master secret unavailable ({e}); vault disabled until fixed");
        String::new()
    });
    let vault_key = crypto::derive_vault_key(&secret);
    let vault = if paths.vault_file().exists() {
        vault::VaultData::load(&paths.vault_file())
    } else {
        vault::migrate_legacy_accounts(&paths.vault_file(), &vault_key)
            .unwrap_or_else(|| vault::VaultData::load(&paths.vault_file()))
    };
    let sessions = harness::SessionVault::load(&paths.sessions_file());
    if let Err(e) = proxy::ensure_proxy_config(&paths) {
        eprintln!("proxy config init failed: {e}");
    }
    AppState {
        settings: Mutex::new(settings),
        secret,
        vault: Mutex::new(vault),
        sessions: Mutex::new(sessions),
        proxy: Mutex::default(),
        oauth: Mutex::default(),
        harness_oauth: Mutex::default(),
        paths,
    }
}

fn cleanup(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    oauth::cancel(app);
    commands::cancel_harness_oauth(app.clone());
    let keep = state.settings.lock().unwrap().keep_proxy_on_exit;
    if keep {
        return;
    }
    let settings = state.settings.lock().unwrap().clone();
    let managed = state.proxy.lock().unwrap().clone();
    if managed.panel_port != 0 {
        let _ = proxy::shutdown_proxy(&settings, &managed);
    }
}

static HIDE_NOTIFIED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn notify_hidden_once(app: &tauri::AppHandle) {
    if HIDE_NOTIFIED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    use tauri_plugin_notification::NotificationExt;
    let _ = app
        .notification()
        .builder()
        .title("HarnessAuth")
        .body("Uygulama sistem tepsisine küçültüldü. Arka plan servisi çalışmaya devam ediyor.")
        .show();
}
