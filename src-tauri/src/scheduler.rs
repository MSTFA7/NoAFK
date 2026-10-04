use crate::controller::VirtualController;
use crate::state::{AppState, OperationMode};
use rand::Rng;
use serde::Serialize;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use vigem_client::XGamepad;

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

#[derive(Clone, Serialize)]
pub struct DriftTickPayload {
    pub rx: i16,
    pub ry: i16,
    pub lx: i16,
    pub ly: i16,
    pub is_user_active: bool,
    pub next_change_ms: u64,
}

struct DriftState {
    rx: f32,
    ry: f32,
    lx: f32,
    ly: f32,
    target_rx: f32,
    target_ry: f32,
    target_lx: f32,
    target_ly: f32,
    next_change: Instant,
    last_ui_emit: Instant,
}

impl DriftState {
    fn new() -> Self {
        Self {
            rx: 0.0,
            ry: 0.0,
            lx: 0.0,
            ly: 0.0,
            target_rx: 0.0,
            target_ry: 0.0,
            target_lx: 0.0,
            target_ly: 0.0,
            next_change: Instant::now(),
            last_ui_emit: Instant::now(),
        }
    }

    fn step(
        &mut self,
        app_handle: &AppHandle,
        controller: &VirtualController,
        state: &AppState,
    ) {
        let mut rng = rand::thread_rng();

        // Check if user clicked "Nudge Drift" to force an immediate new heading
        if state.force_drift_shift.swap(false, Ordering::SeqCst) {
            self.next_change = Instant::now();
        }

        // 1. Pick a new random target when interval expires
        if Instant::now() >= self.next_change {
            // Wander duration between 1.8s and 4.2s for natural pacing
            let duration_ms = rng.gen_range(1800..=4200);
            self.next_change = Instant::now() + Duration::from_millis(duration_ms);

            // 20% chance of a gentle resting pause near center
            if rng.gen_bool(0.20) {
                self.target_rx = rng.gen_range(-1500.0..=1500.0);
                self.target_ry = rng.gen_range(-1500.0..=1500.0);
                self.target_lx = 0.0;
                self.target_ly = 0.0;
            } else {
                // Organic 2D deflection (15,000 to 24,000, well past GTA V deadzone ~10,000)
                let angle_r: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
                let mag_r: f32 = rng.gen_range(16000.0..=24000.0);
                self.target_rx = angle_r.cos() * mag_r;
                self.target_ry = angle_r.sin() * mag_r;

                // 35% chance left stick takes a slight wandering step
                if rng.gen_bool(0.35) {
                    let angle_l: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
                    let mag_l: f32 = rng.gen_range(12000.0..=18000.0);
                    self.target_lx = angle_l.cos() * mag_l;
                    self.target_ly = angle_l.sin() * mag_l;
                } else {
                    self.target_lx = 0.0;
                    self.target_ly = 0.0;
                }
            }
        }

        // 2. Smooth exponential easing toward target (organic humanized movement)
        self.rx += (self.target_rx - self.rx) * 0.07;
        self.ry += (self.target_ry - self.ry) * 0.07;
        self.lx += (self.target_lx - self.lx) * 0.07;
        self.ly += (self.target_ly - self.ly) * 0.07;

        // 3. Humanized analog micro-noise (+-150)
        let noise_rx = rng.gen_range(-150.0..=150.0);
        let noise_ry = rng.gen_range(-150.0..=150.0);

        let final_rx = (self.rx + noise_rx).clamp(-32000.0, 32000.0) as i16;
        let final_ry = (self.ry + noise_ry).clamp(-32000.0, 32000.0) as i16;
        let final_lx = self.lx.clamp(-32000.0, 32000.0) as i16;
        let final_ly = self.ly.clamp(-32000.0, 32000.0) as i16;

        // Check if user is touching physical controller
        let last_input = state.last_physical_input.load(Ordering::Relaxed);
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let is_user_active = state.passthrough_enabled.load(Ordering::Relaxed)
            && last_input > 0
            && now_secs.saturating_sub(last_input) < 5;

        if !is_user_active {
            let mut pad = XGamepad::default();
            pad.thumb_rx = final_rx;
            pad.thumb_ry = final_ry;
            pad.thumb_lx = final_lx;
            pad.thumb_ly = final_ly;
            let _ = controller.update_raw(&pad);
        }

        // Emit UI event at ~15Hz for smooth visualizer display
        if self.last_ui_emit.elapsed() >= Duration::from_millis(66) {
            self.last_ui_emit = Instant::now();
            let next_ms = if self.next_change > Instant::now() {
                (self.next_change - Instant::now()).as_millis() as u64
            } else {
                0
            };
            let _ = app_handle.emit(
                "drift-tick",
                DriftTickPayload {
                    rx: if is_user_active { 0 } else { final_rx },
                    ry: if is_user_active { 0 } else { final_ry },
                    lx: if is_user_active { 0 } else { final_lx },
                    ly: if is_user_active { 0 } else { final_ly },
                    is_user_active,
                    next_change_ms: next_ms,
                },
            );
        }
    }

    fn reset(&mut self, controller: &VirtualController) {
        self.rx = 0.0;
        self.ry = 0.0;
        self.lx = 0.0;
        self.ly = 0.0;
        self.target_rx = 0.0;
        self.target_ry = 0.0;
        self.target_lx = 0.0;
        self.target_ly = 0.0;
        let _ = controller.update_raw(&XGamepad::default());
    }
}

pub fn start_loop(
    app_handle: AppHandle,
    controller: Arc<VirtualController>,
    state: Arc<AppState>,
) {
    std::thread::spawn(move || {
        let mut drift_state = DriftState::new();
        let mut was_running = false;

        loop {
            let is_running = state.running.load(Ordering::Relaxed);
            let mode = *state.mode.lock().unwrap();

            if !is_running {
                if was_running {
                    drift_state.reset(&controller);
                    was_running = false;
                }
                std::thread::sleep(Duration::from_millis(150));
                continue;
            }

            was_running = true;

            match mode {
                OperationMode::Drift => {
                    drift_state.step(&app_handle, &controller, &state);
                    std::thread::sleep(Duration::from_millis(20)); // ~50Hz smooth physics
                }
                OperationMode::Pulse => {
                    // Periodic pulse mode
                    let base = state.interval_secs.load(Ordering::Relaxed).max(5);
                    let max_jitter = (base / 10).clamp(1, 15);
                    let jitter = rand::thread_rng().gen_range(0..=max_jitter);
                    let total_wait = base + jitter;

                    let mut remaining = total_wait;
                    while remaining > 0 {
                        if !state.running.load(Ordering::Relaxed) {
                            break;
                        }
                        if *state.mode.lock().unwrap() != OperationMode::Pulse {
                            break;
                        }

                        // If passthrough is active and user touched physical controller, pause
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

                    // Firing pulse
                    if state.running.load(Ordering::Relaxed) && *state.mode.lock().unwrap() == OperationMode::Pulse {
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
            }
        }
    });
}
