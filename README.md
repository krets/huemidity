# HueMIDIty v0.1.0

A modern, lightweight desktop utility that binds incoming MIDI events (CC, Note On/Off) to Philips Hue smart lights, groups, and scenes. It runs as a background service in your system tray, featuring an interactive glassmorphic web dashboard for easy control and mapping.

Hosted on GitHub: [https://github.com/krets/huemidity](https://github.com/krets/huemidity)

---

## Features

- **Auto-Discovery:** Automatically finds your local Philips Hue Bridge IP and guides you through link-button pairing.
- **Glassmorphic Web Dashboard:** A sleek, dark-themed dashboard showing your lights and groups with drag-and-drop widget layout customization.
- **Real-Time MIDI Logger:** See incoming MIDI messages live, with a quick **Bind** button to map them instantly.
- **Granular Mapping Options:**
  - **Auto-On:** Turning a slider or knob automatically turns the light on if it was off.
  - **Invert Control:** Reverses the signal direction (e.g., slide up to dim, down to brighten).
  - **Momentary vs. Latch:** Supports toggle momentaries (triggering only on button press) and standard latch switches.
- **Interactive Gestures:**
  - Double-click any device card on the dashboard to toggle it on/off.
  - Hover over a card and use your mouse scroll wheel to adjust brightness.
- **Platform-Native Integration:**
  - Runs in the system tray on Windows and macOS.
  - Bypass default python taskbar grouping with a clean custom application icon.
  - **Launch on Startup:** Setting in the dashboard to automatically start HueMIDIty when you log into your computer.

---

## Getting Started

### Prerequisites

You will need **Python 3.9+** and a package helper like [astral uv](https://github.com/astral-sh/uv).

If you don't have `uv` installed, get it using your platform package manager:

- **Windows (Winget):**
  ```powershell
  winget install astral-sh.uv
  ```
- **macOS (Homebrew):**
  ```bash
  brew install uv
  ```

---

## Installation & Launch

### 1. Windows

1. Double-click the `install.bat` script to automatically initialize the virtual environment and install all dependencies.
2. Launch the utility by executing:
   ```powershell
   .venv\Scripts\python.exe main.py
   ```
3. A lightbulb icon will appear in your system tray. Double-click it or right-click and choose **Show/Hide Dashboard** to open the interface.

### 2. macOS

1. Double-click or run the `install.command` script in your terminal to set up the environment and compile a native AppleScript wrapper bundle:
   ```bash
   chmod +x install.command
   ./install.command
   ```
2. Once complete, double-click the generated `HueMIDIty.app` in the directory to launch the app.
3. A lightbulb emoji status item (`💡⌨️`) will appear in your status bar, allowing you to access the dashboard.

---

## Configuration File

Your settings, dashboard layouts, and MIDI bindings are safely stored inside your user-specific folders so they persist across updates:

- **Windows:** `%APPDATA%\HueMIDIty\config.json`
- **macOS:** `~/Library/Application Support/HueMIDIty/config.json`
- **Linux:** `~/.config/huemidity/config.json`

*Note: If you have a legacy local `config.json` in the application folder, HueMIDIty will automatically migrate it to the new user directory on first launch.*

---

## Contributing & Support

For issues, bug reports, or feature requests, visit the repository:
[https://github.com/krets/huemidity](https://github.com/krets/huemidity)
