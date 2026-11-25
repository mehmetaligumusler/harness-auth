// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use tauri::{Manager, RunEvent, WindowEvent};

use harness_auth::{
    commands, crypto, harness::openai_bridge::OpenAiBridgeState, oauth, paths, proxy, secret,
    settings, state::AppState, tray, unified_vault::UnifiedVault,
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
            commands::get_versions,
            commands::push_recent_folder,
            // Centralized Vault
            commands::add_z_api_account,
            commands::add_custom_provider,
            commands::update_unified_account,
            commands::set_account_enabled,
            commands::delete_account,
            commands::get_account_secret,
            commands::test_account,
            commands::import_system_accounts,
            // OAuth Capture / Browser Flows
            commands::start_oauth,
            commands::cancel_oauth,
            commands::start_claude_oauth,
            commands::capture_claude_oauth,
            commands::start_agy_oauth,
            commands::capture_agy_oauth,
            commands::cancel_harness_oauth,
            // Harnesses (Claude Code / OpenCode / ZCode / Antigravity)
            commands::activate_for_harness,
            commands::disable_harness,
            commands::launch_harness,
            commands::opencode_set_live_model,
            commands::opencode_upsert_live_provider,
            commands::opencode_remove_live_provider,
            commands::opencode_toggle_account,
            commands::opencode_set_default_account,
            // ZCode Proxy Service
            commands::activate_account,
            commands::proxy_start,
            commands::proxy_stop,
            commands::proxy_restart,
            commands::proxy_kill,
            commands::proxy_status,
            commands::proxy_test,
            commands::fetch_quota,
            commands::fetch_logs,
            commands::read_error_logs,
            commands::get_config,
            commands::save_config,
            commands::test_model,
        ])
        .setup(|app| {
            let handle = app.handle();

            if let Err(e) = tray::create(handle) {
                eprintln!("tray init failed: {e}");
            }

            {
                let state = handle.state::<AppState>();
                let settings = state.settings.lock().unwrap().clone();
                if let Some(adopted) = proxy::adopt_proxy(&settings) {
                    *state.proxy.lock().unwrap() = adopted;
                }
            }

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

            {
                let state = handle.state::<AppState>();
                let autostart = state.settings.lock().unwrap().proxy_autostart;
                let has_account = state
                    .vault
                    .lock()
                    .unwrap()
                    .first_enabled_zcode_service()
                    .is_some();
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
    let mut settings = settings::Settings::load(&paths);
    let secret = secret::load_or_create().unwrap_or_else(|e| {
        eprintln!("master secret unavailable ({e}); vault disabled until fixed");
        String::new()
    });
    let vault_key = crypto::derive_vault_key(&secret);
    let (vault, settings_changed) =
        UnifiedVault::load_or_migrate(&paths, &vault_key, &mut settings);
    if settings_changed {
        let _ = settings.save(&paths);
    }
    if let Err(e) = proxy::ensure_proxy_config(&paths) {
        eprintln!("proxy config init failed: {e}");
    }
    AppState {
        settings: Mutex::new(settings),
        secret,
        vault: Mutex::new(vault),
        proxy: Mutex::default(),
        oauth: Mutex::default(),
        harness_oauth: Mutex::default(),
        openai_bridge: OpenAiBridgeState::default(),
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
