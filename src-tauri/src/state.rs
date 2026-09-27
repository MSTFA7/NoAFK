use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Mutex,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PulsePattern {
    RightStickNudge,
    LeftStickNudge,
    DpadTap,
    TriggerTap,
    Spin,
}

impl Default for PulsePattern {
    fn default() -> Self {
        Self::RightStickNudge
    }
}

pub struct AppState {
    pub running: AtomicBool,
    pub interval_secs: AtomicU64,
    pub pattern: Mutex<PulsePattern>,
    pub pulse_count: AtomicU64,
    pub last_pulse_timestamp: Mutex<Option<u64>>,
    pub driver_available: AtomicBool,
    pub minimize_to_tray: AtomicBool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            running: AtomicBool::new(false),
            interval_secs: AtomicU64::new(90),
            pattern: Mutex::new(PulsePattern::default()),
            pulse_count: AtomicU64::new(0),
            last_pulse_timestamp: Mutex::new(None),
            driver_available: AtomicBool::new(false),
            minimize_to_tray: AtomicBool::new(true),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusPayload {
    pub running: bool,
    pub interval_secs: u64,
    pub pattern: PulsePattern,
    pub pulse_count: u64,
    pub last_pulse_timestamp: Option<u64>,
    pub driver_available: bool,
    pub minimize_to_tray: bool,
}

impl AppState {
    pub fn get_status(&self) -> StatusPayload {
        StatusPayload {
            running: self.running.load(Ordering::Relaxed),
            interval_secs: self.interval_secs.load(Ordering::Relaxed),
            pattern: *self.pattern.lock().unwrap(),
            pulse_count: self.pulse_count.load(Ordering::Relaxed),
            last_pulse_timestamp: *self.last_pulse_timestamp.lock().unwrap(),
            driver_available: self.driver_available.load(Ordering::Relaxed),
            minimize_to_tray: self.minimize_to_tray.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let state = AppState::default();
        let status = state.get_status();
        assert!(!status.running);
        assert_eq!(status.interval_secs, 90);
        assert_eq!(status.pattern, PulsePattern::RightStickNudge);
        assert_eq!(status.pulse_count, 0);
        assert_eq!(status.last_pulse_timestamp, None);
        assert!(status.minimize_to_tray);
    }

    #[test]
    fn test_pattern_serde() {
        let patterns = [
            (PulsePattern::RightStickNudge, "\"right_stick_nudge\""),
            (PulsePattern::LeftStickNudge, "\"left_stick_nudge\""),
            (PulsePattern::DpadTap, "\"dpad_tap\""),
            (PulsePattern::TriggerTap, "\"trigger_tap\""),
            (PulsePattern::Spin, "\"spin\""),
        ];

        for (pattern, expected_json) in patterns {
            let serialized = serde_json::to_string(&pattern).unwrap();
            assert_eq!(serialized, expected_json);
            let deserialized: PulsePattern = serde_json::from_str(&serialized).unwrap();
            assert_eq!(deserialized, pattern);
        }
    }

    #[test]
    fn test_status_payload_serde() {
        let payload = StatusPayload {
            running: true,
            interval_secs: 120,
            pattern: PulsePattern::Spin,
            pulse_count: 42,
            last_pulse_timestamp: Some(1700000000),
            driver_available: true,
            minimize_to_tray: true,
        };

        let json = serde_json::to_string(&payload).unwrap();
        let decoded: StatusPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.running, true);
        assert_eq!(decoded.interval_secs, 120);
        assert_eq!(decoded.pattern, PulsePattern::Spin);
        assert_eq!(decoded.pulse_count, 42);
        assert_eq!(decoded.last_pulse_timestamp, Some(1700000000));
        assert_eq!(decoded.driver_available, true);
        assert_eq!(decoded.minimize_to_tray, true);
    }
}
