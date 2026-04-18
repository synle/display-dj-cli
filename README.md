# display-dj-cli

[![Build Release Binaries](https://github.com/synle/display-dj-cli/actions/workflows/build.yml/badge.svg)](https://github.com/synle/display-dj-cli/actions/workflows/build.yml)

Cross-platform CLI for controlling monitor brightness, display scaling, system volume, dark mode, keep-awake, and desktop wallpaper.

Supports **macOS**, **Windows**, and **Linux** (X11 + Wayland).

## Install

### Download pre-built binary

Download from the [Actions](../../actions) artifacts tab, or from [Releases](../../releases).

| Platform | Binary |
|----------|--------|
| macOS (Apple Silicon) | `display-dj-macos-arm64` |
| macOS (Intel) | `display-dj-macos-x64` |
| Windows | `display-dj-windows-x64.exe` |
| Linux (x86_64) | `display-dj-linux-x64` |

```bash
# macOS / Linux: make executable and move to PATH
chmod +x display-dj-*
sudo mv display-dj-* /usr/local/bin/display-dj

# Windows: just move the .exe somewhere in your PATH
```

### Build from source

```bash
cargo build --release
# Binary: target/release/ddc-test (or ddc-test.exe on Windows)
```

## Usage

```bash
display-dj set_all <level> [mode]       # Set all displays (0-100)
display-dj set_one <id> <level> [mode]   # Set one display (by ID or name)
display-dj get_all                      # Get brightness for all (JSON)
display-dj get_one <id>                 # Get brightness for one (JSON)
display-dj list                         # List all displays (JSON)
display-dj reset                        # Reset gamma to defaults
display-dj set_contrast_all <level>     # Set contrast on all displays (0-100, DDC only)
display-dj set_contrast_one <id> <level> # Set contrast on one display (0-100, DDC only)
display-dj dark                         # Switch to dark mode
display-dj light                        # Switch to light mode
display-dj theme                        # Get current theme (JSON)
display-dj get_volume                   # Get volume (JSON)
display-dj set_volume <level>           # Set volume (0-100)
display-dj mute                         # Mute audio
display-dj unmute                       # Unmute audio
display-dj get_scale                    # Get display scaling (JSON)
display-dj set_scale_all <percent>      # Set all displays scaling (75-300)
display-dj set_scale_one <id> <percent> # Set one display scaling (75-300)
display-dj keep_awake_on                # Prevent system sleep (blocks until Ctrl+C)
display-dj keep_awake_off               # Stop preventing system sleep
display-dj get_keep_awake               # Get keep-awake status (JSON)
display-dj set_wallpaper <fit> <path>   # Set wallpaper on all monitors
display-dj set_wallpaper_one <index> <fit> <path>  # Set wallpaper on one monitor (0-based)
display-dj get_wallpaper                # Get current wallpaper (JSON)
display-dj get_wallpaper_supported      # Check wallpaper support (JSON)
display-dj wallpaper_slideshow_start <interval> <order> <fit> <folder>
                                        # Start slideshow (interval: minutes, order: forward/backward/random)
display-dj wallpaper_slideshow_stop     # Stop slideshow
display-dj wallpaper_slideshow_status   # Get slideshow status (JSON)
display-dj debug                        # Dump diagnostics for all displays (JSON)
display-dj serve [port]                 # Start HTTP server (default: 51337)
```

### Examples

```bash
display-dj list                         # see all displays
display-dj set_all 50                   # set everything to 50%
display-dj set_one builtin 80           # set built-in display
display-dj set_one 2 100 ddc            # set external #2 via DDC only
display-dj set_one "XZ322QU V3" 30      # set by monitor name
display-dj get_one builtin              # read brightness as JSON
display-dj reset                        # restore gamma
display-dj set_contrast_all 50          # set contrast on all DDC monitors
display-dj set_contrast_one 2 70        # set contrast on external #2
display-dj dark                         # switch to dark mode
display-dj theme                        # check current theme
display-dj get_volume                   # read volume as JSON
display-dj set_volume 50                # set volume to 50%
display-dj mute                         # mute
display-dj unmute                       # unmute
display-dj get_scale                    # see current scale per display
display-dj set_scale_all 150            # set all displays to 150%
display-dj set_scale_one builtin 200    # set built-in to 200% (Retina)
display-dj keep_awake_on                # prevent sleep (Ctrl+C to stop)
display-dj get_keep_awake               # check keep-awake status
display-dj keep_awake_off               # stop preventing sleep
display-dj set_wallpaper fill ~/pic.jpg # set wallpaper with fill mode
display-dj set_wallpaper_one 0 fill ~/pic.jpg  # set on monitor 0 only
display-dj get_wallpaper                # get current wallpaper path + fit
display-dj get_wallpaper_supported      # check if wallpaper is supported
display-dj wallpaper_slideshow_start 30 forward fill ~/Pictures  # slideshow every 30 min
display-dj wallpaper_slideshow_status   # check slideshow status
display-dj wallpaper_slideshow_stop     # stop slideshow
display-dj debug                        # dump full diagnostics (active tests + raw platform data)
display-dj serve                        # start HTTP server on port 51337
```

### Modes

| Mode | Behavior |
|------|----------|
| **force** (default) | DDC + gamma stacked on every external monitor. Most reliable. |
| **auto** | DDC where supported, gamma as fallback. |
| **ddc** | DDC/CI only. Monitors without DDC are skipped. |
| **gamma** | Gamma tables only (software dimming). |

### Display IDs

- `builtin` — built-in display (laptop/iMac)
- `1`, `2`, `3`... — external monitors in enumeration order
- Monitor names also work (case-insensitive): `"XZ322QU V3"`, `"VX2718-2KPC"`

## HTTP Server Mode

`display-dj serve` starts a lightweight HTTP server on `127.0.0.1` (localhost only, not exposed to the network). The server keeps the process alive, so gamma changes persist on macOS.

```bash
display-dj serve          # default port 51337
display-dj serve 8080     # custom port
```

All routes are `GET` with path-based parameters — no query strings, no POST bodies. Returns JSON with `Access-Control-Allow-Origin: *` for browser/Electron/Tauri compatibility.

### Routes

```bash
# Display info
curl localhost:51337/list
curl localhost:51337/get_all
curl localhost:51337/get_one/builtin
curl localhost:51337/get_one/2

# Set brightness (0-100, default mode: force)
curl localhost:51337/set_all/50
curl localhost:51337/set_all/50/ddc
curl localhost:51337/set_one/2/80
curl localhost:51337/set_one/2/80/force
curl localhost:51337/set_one/builtin/60

# Set contrast (0-100, DDC only)
curl localhost:51337/set_contrast_all/50
curl localhost:51337/set_contrast_one/2/70

# Theme
curl localhost:51337/dark
curl localhost:51337/light
curl localhost:51337/theme

# Volume
curl localhost:51337/get_volume
curl localhost:51337/set_volume/50
curl localhost:51337/mute
curl localhost:51337/unmute

# Scaling
curl localhost:51337/get_scale
curl localhost:51337/set_scale_all/150
curl localhost:51337/set_scale_one/builtin/200

# Keep-awake
curl localhost:51337/keep_awake
curl localhost:51337/keep_awake/enable
curl localhost:51337/keep_awake/disable

# Wallpaper
curl localhost:51337/set_wallpaper/fill/Users/syle/Pictures/bg.jpg
curl localhost:51337/set_wallpaper_one/0/fill/Users/syle/Pictures/bg.jpg
curl localhost:51337/get_wallpaper
curl localhost:51337/get_wallpaper_supported

# Wallpaper slideshow
curl localhost:51337/wallpaper_slideshow_start/30/forward/fill/Users/syle/Pictures
curl localhost:51337/wallpaper_slideshow_status
curl localhost:51337/wallpaper_slideshow_stop

# Utility
curl localhost:51337/reset
curl localhost:51337/health
curl localhost:51337/debug
```

### Response format

All responses are JSON. Success:

```json
{"status":"ok"}
[{"id":"builtin","name":"Built-in Display","status":"ok"},{"id":"1",...}]
```

Errors return HTTP 400:

```json
{"error":"display '99' not found"}
```

### Integration from any language

```javascript
// JavaScript/TypeScript
const res = await fetch('http://127.0.0.1:51337/set_all/50');
const data = await res.json();
```

```python
# Python
import requests
requests.get('http://127.0.0.1:51337/set_all/50').json()
```

```bash
# Shell
curl -s localhost:51337/get_all | jq '.[].brightness'
```

## Platform Details

### macOS

**No external dependencies.** The binary is fully self-contained.

| Feature | Implementation |
|---------|---------------|
| Built-in brightness | DisplayServices private framework (native FFI) |
| External DDC/CI | `ddc-macos` crate via IOKit I2C (Apple Silicon + Intel) |
| Gamma (software dimming) | `CGSetDisplayTransferByFormula` (CoreGraphics) |
| Dark/light mode | `osascript` (System Events) |
| Volume | `osascript` (`get volume settings` / `set volume output volume`) |
| Scaling | CoreGraphics native FFI (`CGDisplayCopyAllDisplayModes` / `CGDisplaySetDisplayMode`) |
| Keep-awake | `caffeinate -di` child process (pre-installed) |
| Wallpaper | `osascript` (System Events — set picture on every desktop) |

Works on macOS 11+ (Big Sur and later). Apple Silicon and Intel.

### Windows

**No external dependencies.** The binary is fully self-contained.

| Feature | Implementation |
|---------|---------------|
| Built-in brightness | WMI `WmiMonitorBrightnessMethods` via PowerShell |
| External DDC/CI | `ddc-winapi` crate via Dxva2.dll |
| Gamma (software dimming) | `SetDeviceGammaRamp` (GDI32) |
| Dark/light mode | Registry (`AppsUseLightTheme` + `SystemUsesLightTheme`) |
| Volume | PowerShell + COM `IAudioEndpointVolume` |
| Scaling | Registry DPI (`LogPixels`) — requires logout to apply |
| Keep-awake | `SetThreadExecutionState` Win32 API (no external deps) |
| Wallpaper | Registry (`WallpaperStyle` + `TileWallpaper`) + `SystemParametersInfoW` via PowerShell |

Works on Windows 10 and later.

### Linux

**Requires external CLI tools** for DDC and gamma (see setup below). Built-in backlight control reads `/sys/class/backlight` directly (no deps).

| Feature | Implementation | Requires |
|---------|---------------|----------|
| Built-in brightness | `/sys/class/backlight/` sysfs | None (or `brightnessctl` if no write permission) |
| External DDC/CI | `ddcutil` CLI | `ddcutil`, `i2c-tools`, i2c group membership |
| Gamma (X11) | `xrandr --brightness` | `xrandr` (usually pre-installed) |
| Gamma (Wayland wlroots) | `wlr-randr --brightness` | `wlr-randr` |
| Gamma (Wayland other) | `wl-gammarelay-rs` via D-Bus | `wl-gammarelay-rs` |
| Gamma (GNOME Wayland) | Falls back to XWayland xrandr | `xrandr` |
| Dark/light mode | `gsettings` (GNOME), `plasma-apply-colorscheme` (KDE), `xfconf-query` (XFCE) | Desktop-dependent |
| Volume | `pactl` (PulseAudio/PipeWire) with `amixer` (ALSA) fallback | Pre-installed on most desktops |
| Scaling (X11) | `xrandr --scale` | `xrandr` (usually pre-installed) |
| Scaling (Wayland) | `wlr-randr --scale` | `wlr-randr` |
| Keep-awake | `systemd-inhibit` child process | `systemd` (pre-installed on most distros) |
| Wallpaper | `gsettings` (GNOME), `xfconf-query` (XFCE), `feh` fallback | Desktop-dependent |

#### Ubuntu / Debian

```bash
sudo apt install ddcutil i2c-tools brightnessctl x11-xserver-utils wlr-randr
sudo modprobe i2c-dev
echo "i2c-dev" | sudo tee /etc/modules-load.d/i2c-dev.conf
sudo usermod -aG i2c $USER
# Log out and back in for group change to take effect
```

#### Fedora / RHEL / CentOS

```bash
sudo dnf install ddcutil i2c-tools brightnessctl xrandr wlr-randr
sudo modprobe i2c-dev
echo "i2c-dev" | sudo tee /etc/modules-load.d/i2c-dev.conf
sudo usermod -aG i2c $USER
```

#### Arch Linux / Manjaro

```bash
sudo pacman -S ddcutil i2c-tools brightnessctl xorg-xrandr wlr-randr
sudo modprobe i2c-dev
echo "i2c-dev" | sudo tee /etc/modules-load.d/i2c-dev.conf
sudo usermod -aG i2c $USER
```

> `wlr-randr` is only needed for Wayland (Sway, Hyprland). Safe to skip on X11-only setups.

#### Verify setup

```bash
# Check i2c devices are available
ls /dev/i2c-*

# Check ddcutil can see monitors
ddcutil detect

# Check your user is in the i2c group
groups | grep i2c
```

## Notes

- Gamma changes require the process to stay alive on macOS (resets on exit). On Linux (xrandr/wlr-randr), gamma persists after exit.
- DDC brightness is clamped to a minimum of 1 to prevent monitors from turning off or freezing at 0.
- Built-in display always uses the platform-native API regardless of mode.
- When two monitors have the same name, name lookup matches the first one. Use numeric IDs to target a specific one.
- `display-dj debug` runs active diagnostics: sets brightness to 25% and restores, toggles volume and theme, and reports success/failure for each operation alongside raw platform data (HMONITOR details, DDC VCP reads, PnP device IDs, etc.).

## How It Works

| Method | What it does | Tradeoffs |
|--------|-------------|-----------|
| **DDC/CI** | Sends I2C commands to the monitor to adjust the actual backlight | Best quality, but not all monitors support it |
| **Gamma** | Adjusts the GPU's color output curve to simulate dimming | Works everywhere, but reduces color range |
| **Force** | Applies both DDC + gamma together | Most consistent across mixed monitor setups |

## Contributing

See [DEV.md](DEV.md) for architecture, build instructions, platform implementation details, and where to edit for common tasks. See [CONTRIBUTING.md](CONTRIBUTING.md) for the Rust syntax guide and CLI API reference.

## Related

**[display-dj](https://github.com/synle/display-dj)** — A cross-platform desktop app (Tauri + React) that uses this CLI as its backend sidecar. Provides a system tray popup with brightness sliders, dark mode toggle, volume control, night mode scheduling, and global keyboard shortcuts. Available for macOS, Windows, and Linux.
