//! System tray icon + menu.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

pub const TRAY_ID: &str = "harness-auth-tray";

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "HarnessAuth'u Göster", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle-proxy", "ZCode Servisini Başlat / Durdur", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Çıkış", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &toggle, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("HarnessAuth")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "toggle-proxy" => crate::commands::toggle_proxy(app),
            "quit" => {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.close();
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

/// Refresh the tray tooltip with the current proxy state.
pub fn update_tooltip(app: &AppHandle, running: bool) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let text = if running {
            "HarnessAuth — ZCode servisi çalışıyor"
        } else {
            "HarnessAuth — ZCode servisi durduruldu"
        };
        let _ = tray.set_tooltip(Some(text));
    }
}
