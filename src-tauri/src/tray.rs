use crate::state::AppState;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

pub fn update_tray_state(app: &AppHandle, running: bool) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let tooltip = if running {
            "NoAFK - Active (Sending pulses)"
        } else {
            "NoAFK - Paused"
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

pub fn setup_tray(app: &AppHandle, state: Arc<AppState>) -> Result<(), Box<dyn std::error::Error>> {
    let toggle_item = MenuItem::with_id(app, "toggle", "Toggle Start/Stop", true, None::<&str>)?;
    let open_item = MenuItem::with_id(app, "open", "Show Window", true, None::<&str>)?;
    let joy_item = MenuItem::with_id(app, "joy", "Open Game Controllers (joy.cpl)", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&toggle_item, &open_item, &joy_item, &quit_item])?;

    let state_clone = state.clone();

    let icon = match app.default_window_icon() {
        Some(i) => i.clone(),
        None => {
            return Err("Default window icon missing".into());
        }
    };

    TrayIconBuilder::with_id("main-tray")
        .tooltip("NoAFK - Paused")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app_handle, event| {
            match event.id().as_ref() {
                "toggle" => {
                    let new_val = !state_clone.running.load(Ordering::Relaxed);
                    state_clone.running.store(new_val, Ordering::Relaxed);
                    update_tray_state(app_handle, new_val);
                    let _ = app_handle.emit("status-changed", state_clone.get_status());
                }
                "open" => {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
                "joy" => {
                    let _ = std::process::Command::new("joy.cpl").spawn();
                }
                "quit" => {
                    app_handle.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app_handle = tray.app_handle();
                if let Some(window) = app_handle.get_webview_window("main") {
                    let is_visible = window.is_visible().unwrap_or(false);
                    if is_visible {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
