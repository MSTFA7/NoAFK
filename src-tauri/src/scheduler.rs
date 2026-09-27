use crate::controller::VirtualController;
use crate::state::AppState;
use rand::Rng;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize)]
pub struct PulseTickPayload {
    pub remaining_secs: u64,
    pub total_secs: u64,
    pub is_running: bool,
}

#[derive(Clone, Serialize)]
pub struct PulseFiredPayload {
    pub pattern: String,
    pub pulse_count: u64,
    pub timestamp: u64,
    pub success: bool,
    pub error: Option<String>,
}

pub fn start_loop(
    app_handle: AppHandle,
    controller: Arc<VirtualController>,
    state: Arc<AppState>,
) {
    std::thread::spawn(move || {
        loop {
            if !state.running.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(250));
                continue;
            }

            let base = state.interval_secs.load(Ordering::Relaxed).max(5);
            let max_jitter = (base / 10).clamp(1, 15);
            let jitter = rand::thread_rng().gen_range(0..=max_jitter);
            let total_wait = base + jitter;

            // Countdown loop
            let mut remaining = total_wait;
            while remaining > 0 {
                if !state.running.load(Ordering::Relaxed) {
                    break;
                }

                // If passthrough is active and user touched physical controller recently, postpone countdown
                if state.passthrough_enabled.load(Ordering::Relaxed) {
                    let last_input = state.last_physical_input.load(Ordering::Relaxed);
                    if last_input > 0 {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        if now.saturating_sub(last_input) < 15 {
                            remaining = total_wait;
                            let _ = app_handle.emit(
                                "pulse-tick",
                                PulseTickPayload {
                                    remaining_secs: remaining,
                                    total_secs: total_wait,
                                    is_running: true,
                                },
                            );
                            std::thread::sleep(Duration::from_secs(1));
                            continue;
                        }
                    }
                }

                let _ = app_handle.emit(
                    "pulse-tick",
                    PulseTickPayload {
                        remaining_secs: remaining,
                        total_secs: total_wait,
                        is_running: true,
                    },
                );

                std::thread::sleep(Duration::from_secs(1));
                remaining = remaining.saturating_sub(1);
            }

            // Check if still running after countdown
            if state.running.load(Ordering::Relaxed) {
                if state.passthrough_enabled.load(Ordering::Relaxed) {
                    let last_input = state.last_physical_input.load(Ordering::Relaxed);
                    if last_input > 0 {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        if now.saturating_sub(last_input) < 15 {
                            continue;
                        }
                    }
                }

                let pattern = *state.pattern.lock().unwrap();
                let pulse_res = controller.pulse(pattern);

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let (success, err_msg) = match pulse_res {
                    Ok(_) => {
                        state.pulse_count.fetch_add(1, Ordering::Relaxed);
                        *state.last_pulse_timestamp.lock().unwrap() = Some(now);
                        (true, None)
                    }
                    Err(e) => (false, Some(e.to_string())),
                };

                let pattern_str = match pattern {
                    crate::state::PulsePattern::RightStickNudge => "Right Stick Nudge",
                    crate::state::PulsePattern::LeftStickNudge => "Left Stick Nudge",
                    crate::state::PulsePattern::DpadTap => "D-Pad Tap",
                    crate::state::PulsePattern::TriggerTap => "Right Trigger Tap",
                    crate::state::PulsePattern::Spin => "Spin",
                };

                let _ = app_handle.emit(
                    "pulse-fired",
                    PulseFiredPayload {
                        pattern: pattern_str.to_string(),
                        pulse_count: state.pulse_count.load(Ordering::Relaxed),
                        timestamp: now,
                        success,
                        error: err_msg,
                    },
                );
            }
        }
    });
}
