mod controller;
mod passthrough;
mod scheduler;
mod state;
mod tray;

use controller::VirtualController;
use state::{AppState, OperationMode, PulsePattern, StatusPayload};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;

#[tauri::command]
fn get_status(state: tauri::State<Arc<AppState>>) -> StatusPayload {
    state.get_status()
}

#[tauri::command]
fn set_mode(
    app: AppHandle,
    state: tauri::State<Arc<AppState>>,
    mode: OperationMode,
) -> StatusPayload {
    *state.mode.lock().unwrap() = mode;
    let status = state.get_status();
    let _ = app.emit("status-changed", &status);
    status
}

#[tauri::command]
fn set_passthrough(state: tauri::State<Arc<AppState>>, enabled: bool) {
    state.passthrough_enabled.store(enabled, Ordering::Relaxed);
}

#[tauri::command]
fn set_running(
    app: AppHandle,
    state: tauri::State<Arc<AppState>>,
    value: bool,
) -> StatusPayload {
    state.running.store(value, Ordering::Relaxed);
    tray::update_tray_state(&app, value);
    let status = state.get_status();
    let _ = app.emit("status-changed", &status);
    status
}

#[tauri::command]
fn set_interval(state: tauri::State<Arc<AppState>>, seconds: u64) {
    let clamped = seconds.clamp(10, 3600);
    state.interval_secs.store(clamped, Ordering::Relaxed);
}

#[tauri::command]
fn set_pattern(state: tauri::State<Arc<AppState>>, pattern: PulsePattern) {
    *state.pattern.lock().unwrap() = pattern;
}

#[tauri::command]
fn set_minimize_to_tray(state: tauri::State<Arc<AppState>>, enabled: bool) {
    state.minimize_to_tray.store(enabled, Ordering::Relaxed);
}

#[tauri::command]
fn test_pulse(
    controller: tauri::State<Arc<VirtualController>>,
    state: tauri::State<Arc<AppState>>,
) -> Result<String, String> {
    let pattern = *state.pattern.lock().unwrap();
    match controller.pulse(pattern) {
        Ok(_) => {
            state.pulse_count.fetch_add(1, Ordering::Relaxed);
            Ok("Pulse delivered successfully!".to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn reconnect_controller(
    controller: tauri::State<Arc<VirtualController>>,
    state: tauri::State<Arc<AppState>>,
) -> Result<bool, String> {
    match controller.connect() {
        Ok(_) => {
            state.driver_available.store(true, Ordering::Relaxed);
            Ok(true)
        }
        Err(e) => {
            state.driver_available.store(false, Ordering::Relaxed);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
fn nudge_drift(state: tauri::State<Arc<AppState>>) -> Result<(), String> {
    state.force_drift_shift.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
fn open_game_controllers() -> Result<(), String> {
    std::process::Command::new("joy.cpl")
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let controller = Arc::new(VirtualController::new());
    let state = Arc::new(AppState::default());

    // Update initial driver availability state
    state
        .driver_available
        .store(controller.is_connected(), Ordering::Relaxed);

    let controller_clone = controller.clone();
    let state_clone = state.clone();
    let state_for_shortcut = state.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_shortcuts(["Ctrl+Alt+A"])
                .unwrap()
                .with_handler(move |app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let current = state_for_shortcut.running.load(Ordering::Relaxed);
                        let new_val = !current;
                        state_for_shortcut.running.store(new_val, Ordering::Relaxed);
                        tray::update_tray_state(app, new_val);
                        let _ = app.emit("status-changed", state_for_shortcut.get_status());
                    }
                })
                .build(),
        )
        .manage(controller.clone())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            get_status,
            set_mode,
            set_passthrough,
            set_running,
            set_interval,
            set_pattern,
            set_minimize_to_tray,
            test_pulse,
            nudge_drift,
            reconnect_controller,
            open_game_controllers
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // Setup system tray
            let _ = tray::setup_tray(&app_handle, state_clone.clone());

            // Start controller passthrough background loop
            passthrough::start_passthrough_loop(
                controller_clone.clone(),
                state_clone.clone(),
            );

            // Start scheduler background loop
            scheduler::start_loop(
                app_handle.clone(),
                controller_clone.clone(),
                state_clone.clone(),
            );

            // Window close event handling
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                let state_for_close = state_clone.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        if state_for_close.minimize_to_tray.load(Ordering::Relaxed) {
                            api.prevent_close();
                            let _ = window_clone.hide();
                        }
                    }
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
