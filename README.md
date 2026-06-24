# HueMIDIty v0.1.1

Binds incoming MIDI events (CC, Note On/Off) to Philips Hue lights, groups, and scenes. It runs in the system tray and opens a configuration dashboard.

## Installation

### 1. macOS (via Homebrew Tap)

The recommended way to install on macOS is using Homebrew:

```bash
brew tap krets/huemidity
brew trust krets/huemidity  # Required in Homebrew 6.0+ for custom taps
brew install huemidity
```

This will install the command-line utility `huemidity` and package a native `HueMIDIty.app` launcher.

* **To run the CLI directly:**
  ```bash
  huemidity
  ```
* **To add a double-clickable launcher in `/Applications`:**
  ```bash
  ln -s "$(brew --prefix)/opt/huemidity/HueMIDIty.app" /Applications/
  ```

### 2. All Platforms (via `uv`)

If you have [uv](https://github.com/astral-sh/uv) installed, you can install and run it directly as a global tool:

```bash
uv tool install git+https://github.com/krets/huemidity.git
huemidity
```

This installs HueMIDIty in an isolated environment and exposes the `huemidity` command on your PATH.

### 3. macOS Double-Clickable Bundle (Source Installation)

If you want to run or build from source on macOS:

```bash
git clone https://github.com/krets/huemidity.git
cd huemidity
./install.command
open HueMIDIty.app
```
*(The executable permission for `install.command` is pre-set via git.)*
