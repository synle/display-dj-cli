# display-dj

Cross-platform CLI for monitor brightness, display scaling, system volume, and dark mode control.

## Architecture

Platform-abstracted Rust binary with shared interface (`main.rs`) and platform modules.

The `Platform` trait defines: `enumerate`, `reset_all_gamma`, `debug_info`.
The `DisplayControl` trait defines: `get_brightness`, `get_contrast`, `set_brightness`, `set_contrast`, `reset_gamma`.

All displays (built-in + external) are unified under the same interface with consistent IDs: `builtin` (or `0`), `1`, `2`, etc. Display lookup supports both ID and monitor name (case-insensitive).

Dark mode, volume, and scaling live directly in `main.rs` behind `#[cfg(target_os)]` blocks — they're OS-level features, not per-display (except scaling which is per-display).

### macOS (`src/macos.rs` + `main.rs`)

1. **Built-in display**: `DisplayServicesGetBrightness` / `DisplayServicesSetBrightness` from the private DisplayServices framework, loaded at runtime via `dlopen`/`dlsym`.
2. **External DDC/CI**: `ddc-macos` crate — uses `IOAVServiceWriteI2C`/`IOAVServiceReadI2C` (Apple Silicon) or `IOI2CSendRequest` (Intel).
3. **External gamma**: `CGSetDisplayTransferByFormula`. Resets on process exit.
4. **Dark mode**: `osascript` via System Events (`tell appearance preferences to set dark mode`).
5. **Volume**: `osascript` — `get volume settings` / `set volume output volume`.
6. **Scaling**: CoreGraphics native FFI — `CGDisplayCopyAllDisplayModes` to enumerate modes, `CGDisplaySetDisplayMode` to switch. Scale = `CGDisplayModeGetPixelWidth / CGDisplayModeGetWidth`. No external deps.

### Windows (`src/windows.rs` + `main.rs`)

1. **Built-in display**: WMI `WmiMonitorBrightness` / `WmiMonitorBrightnessMethods` via PowerShell.
2. **External DDC/CI**: `ddc-winapi` crate — uses Win32 Dxva2 (`GetMonitorBrightness`/`SetMonitorBrightness`).
3. **Display dedup**: On laptops, the built-in panel can appear in both WMI and `ddc_winapi::Monitor::enumerate()`. The enumerate code checks `MONITORINFOF_PRIMARY` via `GetMonitorInfoW` and skips the primary HMONITOR from DDC when a WMI builtin was already detected. Display names are enriched with PnP device IDs from `EnumDisplayDevicesW` (e.g. `Generic PnP Monitor (DEL40F4)`) to distinguish monitors with the same generic description.
4. **External gamma**: `SetDeviceGammaRamp` via GDI32.
5. **Dark mode**: Registry keys `AppsUseLightTheme` + `SystemUsesLightTheme` via `reg add` + `WM_SETTINGCHANGE` broadcast for title bar refresh.
6. **Volume**: PowerShell `AudioDeviceCmdlets` module. Requires one-time setup: `Install-Module -Name AudioDeviceCmdlets`.
7. **Scaling**: Registry DPI (`LogPixels` + `Win8DpiScaling`). Requires logout to apply.

### Linux (`src/linux.rs` + `main.rs`)

1. **Built-in display**: `/sys/class/backlight/*/brightness` sysfs (direct write) or `brightnessctl` fallback.
2. **External DDC/CI**: `ddcutil` CLI — reads/writes VCP codes over i2c-dev.
3. **External gamma (X11)**: `xrandr --output <name> --brightness <val>`. Persists after process exit.
4. **External gamma (Wayland)**: Tries in order: `wlr-randr` (wlroots compositors), `wl-gammarelay-rs` (via busctl), XWayland fallback via xrandr.
5. **Display server detection**: `XDG_SESSION_TYPE` env var, then `WAYLAND_DISPLAY` / `DISPLAY` fallback.
6. **Output name mapping**: `xrandr --listactivemonitors` (X11), `wlr-randr` / `swaymsg` (Wayland). Filters out eDP/LVDS (built-in).
7. **Dark mode**: Tries in order: `gsettings` (GNOME color-scheme + gtk-theme), `plasma-apply-colorscheme` (KDE), `xfconf-query` (XFCE).
8. **Volume**: `pactl` (PulseAudio/PipeWire) with `amixer` (ALSA) fallback.
9. **Scaling (X11)**: `xrandr --scale`. Uses inverse scale (100%/target) since xrandr scales the framebuffer.
10. **Scaling (Wayland)**: `wlr-randr --scale`. Direct scale factor.

### Known behaviors

- Some monitors (e.g., Acer XZ322QU V3) return DDC/CI checksum errors on reads and silently ignore writes. These need gamma fallback.
- DDC brightness 0 can cause monitors to enter standby/freeze. Clamped to minimum 1.
- Gamma on a monitor with low DDC backlight produces minimal visible change (the effects multiply).
- The `force` mode (DDC + gamma stacked) provides the most consistent results across mixed monitor setups.
- Scale is clamped to 75%-300% on all platforms to prevent unusable UI.
- macOS scaling switches display modes (resolution-based). Windows requires logout. Linux X11/Wayland applies instantly.

## CLI

```bash
# Brightness
display-dj set_all <level> [mode]
display-dj set_one <id|name> <level> [mode]
display-dj get_all
display-dj get_one <id|name>
display-dj list
display-dj reset

# Theme
display-dj dark
display-dj light
display-dj theme

# Volume
display-dj get_volume
display-dj set_volume <level>
display-dj mute
display-dj unmute

# Scaling
display-dj get_scale
display-dj set_scale_all <percent>
display-dj set_scale_one <id|name> <percent>

# Diagnostics
display-dj debug

# Server
display-dj serve [port]
```
