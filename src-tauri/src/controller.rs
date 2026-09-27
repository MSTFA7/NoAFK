use crate::state::PulsePattern;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use vigem_client::{Client, TargetId, Xbox360Wired, XGamepad, XButtons};

pub struct VirtualController {
    target: Mutex<Option<Xbox360Wired<Client>>>,
    pub is_pulsing: AtomicBool,
}

impl VirtualController {
    pub fn new() -> Self {
        let instance = Self {
            target: Mutex::new(None),
            is_pulsing: AtomicBool::new(false),
        };
        let _ = instance.connect();
        instance
    }

    pub fn is_connected(&self) -> bool {
        self.target.lock().unwrap().is_some()
    }

    pub fn connect(&self) -> anyhow::Result<()> {
        let mut target_lock = self.target.lock().unwrap();
        if target_lock.is_some() {
            return Ok(());
        }

        let client = Client::connect()?;
        let mut target = Xbox360Wired::new(client, TargetId::XBOX360_WIRED);
        target.plugin()?;
        target.wait_ready()?;

        *target_lock = Some(target);
        Ok(())
    }

    pub fn unplug(&self) {
        let mut target_lock = self.target.lock().unwrap();
        if let Some(mut target) = target_lock.take() {
            let _ = target.unplug();
        }
    }

    pub fn update_raw(&self, gamepad: &XGamepad) -> anyhow::Result<()> {
        let mut target_lock = self.target.lock().unwrap();
        if let Some(target) = target_lock.as_mut() {
            target.update(gamepad)?;
            Ok(())
        } else {
            anyhow::bail!("Virtual controller not connected");
        }
    }

    pub fn pulse(&self, pattern: PulsePattern) -> anyhow::Result<()> {
        self.is_pulsing.store(true, Ordering::SeqCst);
        let res = self.pulse_internal(pattern);
        self.is_pulsing.store(false, Ordering::SeqCst);
        res
    }

    fn pulse_internal(&self, pattern: PulsePattern) -> anyhow::Result<()> {
        let mut target_lock = self.target.lock().unwrap();
        let target = match target_lock.as_mut() {
            Some(t) => t,
            None => {
                anyhow::bail!("ViGEmBus driver not connected. Please install the driver and click Reconnect.");
            }
        };

        let mut gamepad = XGamepad::default();
        match pattern {
            PulsePattern::RightStickNudge => {
                // Break past XInput deadzone (8689) and GTA deadzones (~10000)
                gamepad.thumb_rx = 24000;
                target.update(&gamepad)?;
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            PulsePattern::LeftStickNudge => {
                gamepad.thumb_lx = 24000;
                target.update(&gamepad)?;
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            PulsePattern::DpadTap => {
                // D-pad UP opens phone in GTA V
                gamepad.buttons = XButtons!(UP);
                target.update(&gamepad)?;
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            PulsePattern::TriggerTap => {
                // Trigger threshold in XInput is 30, use 220 (85%) to register past deadzones
                gamepad.right_trigger = 220;
                target.update(&gamepad)?;
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
            PulsePattern::Spin => {
                use rand::Rng;
                let dir: i32 = if rand::thread_rng().gen_bool(0.5) { 1 } else { -1 };
                let dev_lx = rand::thread_rng().gen_range(-2000..=2000);
                let dev_ly = rand::thread_rng().gen_range(-2000..=2000);
                let dev_rx = rand::thread_rng().gen_range(-2000..=2000);

                // Both walk and turn camera with natural variation
                gamepad.thumb_lx = ((24000 * dir) + dev_lx).clamp(-32000, 32000) as i16;
                gamepad.thumb_ly = (16000 + dev_ly).clamp(-32000, 32000) as i16;
                gamepad.thumb_rx = ((26000 * dir) + dev_rx).clamp(-32000, 32000) as i16;

                target.update(&gamepad)?;
                std::thread::sleep(std::time::Duration::from_millis(1000));
            }
        }

        target.update(&XGamepad::default())?;
        Ok(())
    }
}

impl Drop for VirtualController {
    fn drop(&mut self) {
        self.unplug();
    }
}
