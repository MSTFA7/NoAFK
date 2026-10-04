use crate::controller::VirtualController;
use crate::state::AppState;
use rusty_xinput::XInputHandle;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use vigem_client::{XButtons, XGamepad};

#[derive(Default)]
struct SlotTracker {
    last_packet: u32,
    rest_lx: i16,
    rest_ly: i16,
    rest_rx: i16,
    rest_ry: i16,
    calibrated: bool,
}

pub fn start_passthrough_loop(
    controller: Arc<VirtualController>,
    state: Arc<AppState>,
) {
    std::thread::spawn(move || {
        let xinput = match XInputHandle::load_default() {
            Ok(handle) => handle,
            Err(e) => {
                eprintln!("[NoAFK] XInput failed to load: {:?}", e);
                return;
            }
        };

        let mut trackers: [SlotTracker; 4] = Default::default();
        let mut active_physical_slot: Option<u32> = None;
        let mut last_activity_time = Instant::now() - Duration::from_secs(10);
        let mut has_reset_to_neutral = true;

        loop {
            std::thread::sleep(Duration::from_millis(8)); // ~120Hz polling

            // If passthrough is disabled in state, idle
            if !state.passthrough_enabled.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }

            // Only forward if virtual controller is connected
            if !controller.is_connected() {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }

            // Get kernel-reported user index for virtual controller directly from ViGEmBus
            let virtual_slot = controller.get_user_index();

            // Candidate physical slots: all slots EXCEPT the virtual controller slot.
            // If virtual_slot not yet reported, safely check slots 1, 2, 3 (never slot 0).
            let candidate_slots: Vec<u32> = if let Some(v_slot) = virtual_slot {
                (0..4).filter(|&s| s != v_slot).collect()
            } else {
                vec![1, 2, 3]
            };

            let mut forwarded = false;

            for &slot in &candidate_slots {
                if let Ok(xstate) = xinput.get_state(slot) {
                    let raw = xstate.raw;
                    let pad = raw.Gamepad;
                    let tracker = &mut trackers[slot as usize];

                    if !tracker.calibrated {
                        tracker.last_packet = raw.dwPacketNumber;
                        tracker.rest_lx = pad.sThumbLX;
                        tracker.rest_ly = pad.sThumbLY;
                        tracker.rest_rx = pad.sThumbRX;
                        tracker.rest_ry = pad.sThumbRY;
                        tracker.calibrated = true;
                    }

                    // A change in packet number indicates the controller sent a new hardware report
                    let packet_changed = raw.dwPacketNumber != tracker.last_packet;

                    // Movement relative to calibrated rest baseline
                    let delta_lx = (pad.sThumbLX as i32 - tracker.rest_lx as i32).abs();
                    let delta_ly = (pad.sThumbLY as i32 - tracker.rest_ly as i32).abs();
                    let delta_rx = (pad.sThumbRX as i32 - tracker.rest_rx as i32).abs();
                    let delta_ry = (pad.sThumbRY as i32 - tracker.rest_ry as i32).abs();

                    let stick_deflected = delta_lx > 6000 || delta_ly > 6000 || delta_rx > 6000 || delta_ry > 6000;
                    let trigger_pulled = pad.bLeftTrigger > 35 || pad.bRightTrigger > 35;
                    let button_pressed = pad.wButtons != 0;

                    let has_activity = (packet_changed && stick_deflected) || trigger_pulled || button_pressed;

                    if packet_changed {
                        tracker.last_packet = raw.dwPacketNumber;
                    }

                    if has_activity {
                        active_physical_slot = Some(slot);
                        last_activity_time = Instant::now();
                        has_reset_to_neutral = false;

                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        state.last_physical_input.store(now, Ordering::Relaxed);

                        let forward_pad = XGamepad {
                            buttons: XButtons(pad.wButtons),
                            left_trigger: pad.bLeftTrigger,
                            right_trigger: pad.bRightTrigger,
                            thumb_lx: pad.sThumbLX,
                            thumb_ly: pad.sThumbLY,
                            thumb_rx: pad.sThumbRX,
                            thumb_ry: pad.sThumbRY,
                        };

                        let _ = controller.update_raw(&forward_pad);
                        forwarded = true;
                        break;
                    }
                } else {
                    trackers[slot as usize] = SlotTracker::default();
                }
            }

            if !forwarded {
                if last_activity_time.elapsed() < Duration::from_millis(250) {
                    // Quick decay forwarding of physical state within 250ms
                    if let Some(slot) = active_physical_slot {
                        if let Ok(xstate) = xinput.get_state(slot) {
                            let pad = xstate.raw.Gamepad;
                            let forward_pad = XGamepad {
                                buttons: XButtons(pad.wButtons),
                                left_trigger: pad.bLeftTrigger,
                                right_trigger: pad.bRightTrigger,
                                thumb_lx: pad.sThumbLX,
                                thumb_ly: pad.sThumbLY,
                                thumb_rx: pad.sThumbRX,
                                thumb_ry: pad.sThumbRY,
                            };
                            let _ = controller.update_raw(&forward_pad);
                        }
                    }
                } else if !has_reset_to_neutral {
                    // Reset to neutral ONCE when user releases physical controller
                    if !controller.is_pulsing.load(Ordering::SeqCst) {
                        let _ = controller.update_raw(&XGamepad::default());
                        has_reset_to_neutral = true;
                    }
                }
            }
        }
    });
}
