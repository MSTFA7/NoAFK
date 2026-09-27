import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isEnabled, enable, disable } from "@tauri-apps/plugin-autostart";
import { openUrl } from "@tauri-apps/plugin-opener";

interface StatusPayload {
  running: boolean;
  interval_secs: number;
  pattern: "right_stick_nudge" | "left_stick_nudge" | "dpad_tap" | "trigger_tap" | "spin";
  pulse_count: number;
  last_pulse_timestamp: number | null;
  driver_available: boolean;
  minimize_to_tray: boolean;
}

interface PulseTickPayload {
  remaining_secs: number;
  total_secs: number;
  is_running: boolean;
}

interface PulseFiredPayload {
  pattern: string;
  pulse_count: number;
  timestamp: number;
  success: boolean;
  error?: string;
}

// Current window instance for custom titlebar
const appWindow = getCurrentWindow();

// UI Elements - Titlebar
const titlebarMinimize = document.getElementById("titlebar-minimize") as HTMLButtonElement;
const titlebarClose = document.getElementById("titlebar-close") as HTMLButtonElement;

// UI Elements - Status & Controls
const vigemStatusActive = document.getElementById("vigem-status-active") as HTMLElement;
const vigemStatusMissing = document.getElementById("vigem-status-missing") as HTMLElement;
const btnReconnect = document.getElementById("btn-reconnect") as HTMLButtonElement;
const btnOpenJoycpl = document.getElementById("btn-open-joycpl") as HTMLButtonElement;

const powerStatusLabel = document.getElementById("power-status-label") as HTMLElement;
const powerStatusSub = document.getElementById("power-status-sub") as HTMLElement;
const powerToggleBtn = document.getElementById("power-toggle-btn") as HTMLButtonElement;

const btnTestPulse = document.getElementById("btn-test-pulse") as HTMLButtonElement;
const countdownValue = document.getElementById("countdown-value") as HTMLElement;
const pulseCountText = document.getElementById("pulse-count-text") as HTMLElement;
const lastPulseText = document.getElementById("last-pulse-text") as HTMLElement;

const patternSelect = document.getElementById("pattern-select") as HTMLSelectElement;
const intervalSlider = document.getElementById("interval-slider") as HTMLInputElement;
const intervalDisplay = document.getElementById("interval-display") as HTMLElement;
const autostartToggle = document.getElementById("autostart-toggle") as HTMLInputElement;
const minimizeTrayToggle = document.getElementById("minimize-tray-toggle") as HTMLInputElement;
const footerNote = document.getElementById("footer-note") as HTMLElement;

// Modal Elements
const modalOverlay = document.getElementById("modal-overlay") as HTMLElement;
const modalTitle = document.getElementById("modal-title") as HTMLElement;
const modalMessage = document.getElementById("modal-message") as HTMLElement;
const modalCloseBtn = document.getElementById("modal-close-btn") as HTMLButtonElement;

// Controller Visualizer Container
const controllerVisualizer = document.getElementById("controller-visualizer") as HTMLElement;

let currentRunning = false;

// Custom Titlebar Listeners
titlebarMinimize.addEventListener("click", async () => {
  try {
    await appWindow.minimize();
  } catch (err) {
    console.error("Failed to minimize window:", err);
  }
});

titlebarClose.addEventListener("click", async () => {
  try {
    await appWindow.close();
  } catch (err) {
    console.error("Failed to close window:", err);
  }
});

function showModal(title: string, message: string) {
  modalTitle.textContent = title;
  modalMessage.textContent = message;
  modalOverlay.classList.remove("hidden");
  modalOverlay.setAttribute("aria-hidden", "false");
}

function hideModal() {
  modalOverlay.classList.add("hidden");
  modalOverlay.setAttribute("aria-hidden", "true");
}

modalCloseBtn.addEventListener("click", hideModal);
modalOverlay.addEventListener("click", (e) => {
  if (e.target === modalOverlay) {
    hideModal();
  }
});

function formatDuration(seconds: number): string {
  if (seconds < 60) {
    return `${seconds}s`;
  }
  const mins = Math.floor(seconds / 60);
  const remSecs = seconds % 60;
  return remSecs > 0 ? `${mins}m ${remSecs}s` : `${mins}m`;
}

// Truncated to h:mm a (e.g. "3:45 PM", without seconds)
function formatTimestamp(timestamp: number | null): string {
  if (!timestamp) return "Never";
  const date = new Date(timestamp * 1000);
  return date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

function formatCountdown(remaining: number): string {
  if (remaining < 60) {
    return `~${remaining}s`;
  }
  const mins = Math.floor(remaining / 60);
  const secs = remaining % 60;
  return secs > 0 ? `~${mins}m ${secs}s` : `~${mins}m`;
}

function updatePowerState(running: boolean) {
  currentRunning = running;
  if (running) {
    powerStatusLabel.textContent = "ACTIVE";
    powerStatusLabel.className = "power-state-title active";
    powerStatusSub.textContent = "Anti-AFK pulses running";
    powerToggleBtn.className = "power-button active";
  } else {
    powerStatusLabel.textContent = "PAUSED";
    powerStatusLabel.className = "power-state-title paused";
    powerStatusSub.textContent = "Click button to start pulses";
    powerToggleBtn.className = "power-button";
    countdownValue.textContent = "--";
  }
}

function updateDriverState(available: boolean) {
  if (available) {
    vigemStatusActive.classList.remove("hidden");
    vigemStatusMissing.classList.add("hidden");
  } else {
    vigemStatusActive.classList.add("hidden");
    vigemStatusMissing.classList.remove("hidden");
  }
}

function triggerPulseAnimation(pattern: string) {
  controllerVisualizer.classList.remove(
    "pulse-nudge-left",
    "pulse-nudge-right",
    "pulse-dpad",
    "pulse-trigger",
    "pulse-spin"
  );

  const normalized = pattern.toLowerCase();
  let animClass = "pulse-nudge-right";
  let duration = 350;

  if (normalized.includes("spin")) {
    animClass = "pulse-spin";
    duration = 1000;
  } else if (normalized.includes("left")) {
    animClass = "pulse-nudge-left";
  } else if (normalized.includes("dpad") || normalized.includes("d-pad")) {
    animClass = "pulse-dpad";
  } else if (normalized.includes("trigger")) {
    animClass = "pulse-trigger";
  }

  void controllerVisualizer.offsetWidth;
  controllerVisualizer.classList.add(animClass);
  setTimeout(() => {
    controllerVisualizer.classList.remove(animClass);
  }, duration);
}

function updateFooterNote(minimizeToTray: boolean) {
  footerNote.textContent = minimizeToTray
    ? "Closing window minimizes to tray"
    : "Closing window terminates application";
}

function applyStatus(status: StatusPayload) {
  updatePowerState(status.running);
  updateDriverState(status.driver_available);
  pulseCountText.textContent = status.pulse_count.toString();
  lastPulseText.textContent = formatTimestamp(status.last_pulse_timestamp);

  intervalSlider.value = status.interval_secs.toString();
  intervalDisplay.textContent = formatDuration(status.interval_secs);

  patternSelect.value = status.pattern;
  minimizeTrayToggle.checked = status.minimize_to_tray;
  updateFooterNote(status.minimize_to_tray);
}

async function initApp() {
  try {
    const initialStatus = await invoke<StatusPayload>("get_status");
    applyStatus(initialStatus);

    try {
      const enabled = await isEnabled();
      autostartToggle.checked = enabled;
    } catch (e) {
      console.warn("Autostart check error:", e);
    }

    await listen<StatusPayload>("status-changed", (event) => {
      applyStatus(event.payload);
    });

    await listen<PulseTickPayload>("pulse-tick", (event) => {
      if (!currentRunning) return;
      countdownValue.textContent = formatCountdown(event.payload.remaining_secs);
    });

    await listen<PulseFiredPayload>("pulse-fired", (event) => {
      pulseCountText.textContent = event.payload.pulse_count.toString();
      lastPulseText.textContent = formatTimestamp(event.payload.timestamp);
      triggerPulseAnimation(event.payload.pattern);

      if (!event.payload.success && event.payload.error) {
        updateDriverState(false);
      }
    });
  } catch (err) {
    console.error("Initialization error:", err);
  }
}

// Power Toggle Click
powerToggleBtn.addEventListener("click", async () => {
  const nextVal = !currentRunning;
  try {
    const updated = await invoke<StatusPayload>("set_running", { value: nextVal });
    applyStatus(updated);
  } catch (err) {
    console.error("Failed to toggle running state:", err);
  }
});

// Pattern Change
patternSelect.addEventListener("change", async () => {
  const selected = patternSelect.value;
  try {
    await invoke("set_pattern", { pattern: selected });
  } catch (err) {
    console.error("Failed to set pattern:", err);
  }
});

// Interval Slider
intervalSlider.addEventListener("input", () => {
  const val = Number(intervalSlider.value);
  intervalDisplay.textContent = formatDuration(val);
});

intervalSlider.addEventListener("change", async () => {
  const val = Number(intervalSlider.value);
  try {
    await invoke("set_interval", { seconds: val });
  } catch (err) {
    console.error("Failed to set interval:", err);
  }
});

// Test Pulse Button
btnTestPulse.addEventListener("click", async () => {
  try {
    btnTestPulse.disabled = true;
    btnTestPulse.textContent = "Pulsing...";
    await invoke<string>("test_pulse");
    triggerPulseAnimation(patternSelect.value);

    const status = await invoke<StatusPayload>("get_status");
    applyStatus(status);
  } catch (err) {
    showModal("Pulse Error", `Failed to send pulse: ${err}\nPlease ensure ViGEmBus is installed.`);
    updateDriverState(false);
  } finally {
    btnTestPulse.disabled = false;
    btnTestPulse.textContent = "Test Pulse";
  }
});

// Reconnect Driver Button
btnReconnect.addEventListener("click", async () => {
  try {
    btnReconnect.textContent = "Checking...";
    showModal("Driver Connection", "Checking ViGEmBus driver connection...");

    await invoke("reconnect_controller");
    const status = await invoke<StatusPayload>("get_status");
    applyStatus(status);

    if (status.driver_available) {
      showModal("Driver Status", "ViGEmBus driver connected successfully.");
    } else {
      showModal(
        "Driver Status",
        "ViGEmBus driver was not detected. Please ensure the driver installer has finished and try again."
      );
    }
  } catch (err) {
    showModal("Driver Error", `Connection attempt failed: ${err}`);
  } finally {
    btnReconnect.textContent = "Retry";
  }
});

// Download Driver Link
vigemStatusMissing.addEventListener("click", async (e) => {
  e.preventDefault();
  try {
    await openUrl("https://github.com/ViGEm/ViGEmBus/releases");
  } catch (err) {
    console.error("Failed to open URL:", err);
  }
});

// Open joy.cpl
btnOpenJoycpl.addEventListener("click", async () => {
  try {
    await invoke("open_game_controllers");
  } catch (err) {
    console.error("Failed to open joy.cpl:", err);
  }
});

// Autostart toggle
autostartToggle.addEventListener("change", async () => {
  try {
    if (autostartToggle.checked) {
      await enable();
    } else {
      await disable();
    }
  } catch (err) {
    console.error("Failed to update autostart setting:", err);
    autostartToggle.checked = !autostartToggle.checked;
  }
});

// Minimize to Tray toggle
minimizeTrayToggle.addEventListener("change", async () => {
  const enabled = minimizeTrayToggle.checked;
  try {
    await invoke("set_minimize_to_tray", { enabled });
    updateFooterNote(enabled);
  } catch (err) {
    console.error("Failed to set minimize to tray:", err);
    minimizeTrayToggle.checked = !enabled;
  }
});

// Bootstrap
window.addEventListener("DOMContentLoaded", () => {
  initApp();
});
