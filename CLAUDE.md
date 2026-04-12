# display-dj

Cross-platform CLI for monitor brightness, volume, and dark mode control.

## Architecture

Platform-abstracted Rust binary with shared interface (`main.rs`) and platform modules.

The `Platform` trait defines: `enumerate`, `reset_all_gamma`.
The `DisplayControl` trait defines: `get_brightness`, `get_contrast`, `set_brightness`, `set_contrast`, `reset_gamma`.

All displays (built-in + external) are unified under the same interface with consistent IDs: `builtin` (or `0`), `1`, `2`, etc. Display lookup supports both ID and monitor name (case-insensitive).

Dark mode and volume control live directly in `main.rs` behind `#[cfg(target_os)]` blocks — they're OS-level features, not per-display.

### macOS (`src/macos.rs` + `main.rs`)

1. **Built-in display**: `DisplayServicesGetBrightness` / `DisplayServicesSetBrightness` from the private DisplayServices framework, loaded at runtime via `dlopen`/`dlsym`.
2. **External DDC/CI**: `ddc-macos` crate — uses `IOAVServiceWriteI2C`/`IOAVServiceReadI2C` (Apple Silicon) or `IOI2CSendRequest` (Intel).
3. **External gamma**: `CGSetDisplayTransferByFormula`. Resets on process exit.
4. **Dark mode**: `osascript` via System Events (`tell appearance preferences to set dark mode`).
5. **Volume**: `osascript` — `get volume settings` / `set volume output volume`.

### Windows (`src/windows.rs` + `main.rs`)

1. **Built-in display**: WMI `WmiMonitorBrightness` / `WmiMonitorBrightnessMethods` via PowerShell.
2. **External DDC/CI**: `ddc-winapi` crate — uses Win32 Dxva2 (`GetMonitorBrightness`/`SetMonitorBrightness`).
3. **External gamma**: `SetDeviceGammaRamp` via GDI32.
4. **Dark mode**: Registry keys `AppsUseLightTheme` + `SystemUsesLightTheme` via `reg add`.
5. **Volume**: PowerShell + COM `IAudioEndpointVolume` (default audio endpoint).

### Linux (`src/linux.rs` + `main.rs`)

1. **Built-in display**: `/sys/class/backlight/*/brightness` sysfs (direct write) or `brightnessctl` fallback.
2. **External DDC/CI**: `ddcutil` CLI — reads/writes VCP codes over i2c-dev.
3. **External gamma (X11)**: `xrandr --output <name> --brightness <val>`. Persists after process exit.
4. **External gamma (Wayland)**: Tries in order: `wlr-randr` (wlroots compositors), `wl-gammarelay-rs` (via busctl), XWayland fallback via xrandr.
5. **Display server detection**: `XDG_SESSION_TYPE` env var, then `WAYLAND_DISPLAY` / `DISPLAY` fallback.
6. **Output name mapping**: `xrandr --listactivemonitors` (X11), `wlr-randr` / `swaymsg` (Wayland). Filters out eDP/LVDS (built-in).
7. **Dark mode**: Tries in order: `gsettings` (GNOME color-scheme + gtk-theme), `plasma-apply-colorscheme` (KDE), `xfconf-query` (XFCE).
8. **Volume**: `pactl` (PulseAudio/PipeWire) with `amixer` (ALSA) fallback.

### Known monitor behaviors

- Some monitors (e.g., Acer XZ322QU V3) return DDC/CI checksum errors on reads and silently ignore writes. These need gamma fallback.
- DDC brightness 0 can cause monitors to enter standby/freeze. Clamped to minimum 1.
- Gamma on a monitor with low DDC backlight produces minimal visible change (the effects multiply).
- The `force` mode (DDC + gamma stacked) provides the most consistent results across mixed monitor setups.

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

# Server
display-dj serve [port]
```
