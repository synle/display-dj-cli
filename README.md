# display-dj

Cross-platform CLI for controlling monitor brightness, display scaling, system volume, and dark mode.

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
display-dj dark                         # switch to dark mode
display-dj theme                        # check current theme
display-dj get_volume                   # read volume as JSON
display-dj set_volume 50                # set volume to 50%
display-dj mute                         # mute
display-dj unmute                       # unmute
display-dj get_scale                    # see current scale per display
display-dj set_scale_all 150            # set all displays to 150%
display-dj set_scale_one builtin 200    # set built-in to 200% (Retina)
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

# Utility
curl localhost:51337/reset
curl localhost:51337/health
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

## How It Works

| Method | What it does | Tradeoffs |
|--------|-------------|-----------|
| **DDC/CI** | Sends I2C commands to the monitor to adjust the actual backlight | Best quality, but not all monitors support it |
| **Gamma** | Adjusts the GPU's color output curve to simulate dimming | Works everywhere, but reduces color range |
| **Force** | Applies both DDC + gamma together | Most consistent across mixed monitor setups |
