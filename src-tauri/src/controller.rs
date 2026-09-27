use crate::state::PulsePattern;
use std::sync::Mutex;
use vigem_client::{Client, TargetId, Xbox360Wired, XGamepad, XButtons};

pub struct VirtualController {
    target: Mutex<Option<Xbox360Wired<Client>>>,
}

impl VirtualController {
    pub fn new() -> Self {
        let instance = Self {
            target: Mutex::new(None),
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

    pub fn pulse(&self, pattern: PulsePattern) -> anyhow::Result<()> {
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
                gamepad.thumb_rx = 2000;
                target.update(&gamepad)?;
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            PulsePattern::LeftStickNudge => {
                gamepad.thumb_lx = 2000;
                target.update(&gamepad)?;
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            PulsePattern::DpadTap => {
                gamepad.buttons = XButtons!(UP);
                target.update(&gamepad)?;
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            PulsePattern::TriggerTap => {
                gamepad.right_trigger = 30;
                target.update(&gamepad)?;
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            PulsePattern::Spin => {
                use rand::Rng;
                let dir: i32 = if rand::thread_rng().gen_bool(0.5) { 1 } else { -1 };
                let dev_lx = rand::thread_rng().gen_range(-1500..=1500);
                let dev_ly = rand::thread_rng().gen_range(-1000..=1000);
                let dev_rx = rand::thread_rng().gen_range(-1500..=1500);
                let dev_ry = rand::thread_rng().gen_range(-1000..=1000);

                gamepad.thumb_lx = ((18000 * dir) + dev_lx).clamp(-32000, 32000) as i16;
                gamepad.thumb_ly = dev_ly.clamp(-32000, 32000) as i16;
                gamepad.thumb_rx = ((18000 * dir) + dev_rx).clamp(-32000, 32000) as i16;
                gamepad.thumb_ry = dev_ry.clamp(-32000, 32000) as i16;

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
