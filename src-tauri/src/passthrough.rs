use crate::controller::VirtualController;
use crate::state::AppState;
use rusty_xinput::XInputHandle;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use vigem_client::{XButtons, XGamepad};

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
                    let pad = xstate.raw.Gamepad;

                    let has_activity = pad.wButtons != 0
                        || pad.bLeftTrigger > 25
                        || pad.bRightTrigger > 25
                        || pad.sThumbLX.abs() > 3500
                        || pad.sThumbLY.abs() > 3500
                        || pad.sThumbRX.abs() > 3500
                        || pad.sThumbRY.abs() > 3500;

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
                }
            }

            if !forwarded {
                if last_activity_time.elapsed() < Duration::from_millis(200) {
                    // Quick decay forwarding of physical state within 200ms
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
