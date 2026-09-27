# NoAFK

A lightweight Windows utility that prevents idle timeouts by simulating subtle controller input through a virtual Xbox 360 controller.

## Features

- **Virtual Controller Input**: Sends brief, non-intrusive stick or button inputs.
- **Input Patterns**: Right stick nudge, left stick nudge, D-pad tap, trigger tap, and dual-stick spin.
- **Randomized Timing**: Adds timing variations to avoid mechanical repetition.
- **Global Shortcut**: Press `Ctrl + Alt + A` to toggle active state from anywhere.
- **System Tray Support**: Runs in the background with optional launch on Windows startup.

## Requirements

- Windows 10 or 11 (64-bit)
- [ViGEmBus Driver](https://github.com/ViGEm/ViGEmBus/releases) (required for virtual controller emulation)

## Installation

1. Install the [ViGEmBus Driver](https://github.com/ViGEm/ViGEmBus/releases) if not already installed.
2. Download and run `NoAFK_0.1.0_x64-setup.exe` from the Releases section.

A standalone portable executable (`noafk.exe`) is also provided.

## Build from Source

Requires Node.js (v18+) and Rust.

```bash
npm install
npm run tauri build
```

The installer is output to `src-tauri/target/release/bundle/nsis/`.

## License

MIT
