# display-dj

Cross-platform CLI for monitor brightness, display scaling, system volume, dark mode, keep-awake, and desktop wallpaper control.

See [DEV.md](DEV.md) for the full developer guide (architecture diagrams, build instructions, request lifecycle, where to edit).

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
7. **Keep-awake**: `caffeinate -di` as a child process. `-d` prevents display sleep, `-i` prevents idle sleep. Kill the child to disable.
8. **Wallpaper**: `osascript` via System Events (`tell every desktop to set picture`). Fit mode is best-effort via System Events.

### Windows (`src/windows.rs` + `main.rs`)

1. **Built-in display**: WMI `WmiMonitorBrightness` / `WmiMonitorBrightnessMethods` via PowerShell.
2. **External DDC/CI**: `ddc-winapi` crate — uses Win32 Dxva2 (`GetMonitorBrightness`/`SetMonitorBrightness`).
3. **Display dedup**: On laptops, the built-in panel can appear in both WMI and `ddc_winapi::Monitor::enumerate()`. The enumerate code checks `MONITORINFOF_PRIMARY` via `GetMonitorInfoW` and skips the primary HMONITOR from DDC when a WMI builtin was already detected. Display names are enriched with PnP device IDs from `EnumDisplayDevicesW` (e.g. `Generic PnP Monitor (DEL40F4)`) to distinguish monitors with the same generic description.
4. **External gamma**: `SetDeviceGammaRamp` via GDI32.
5. **Dark mode**: Registry keys `AppsUseLightTheme` + `SystemUsesLightTheme` via `reg add` + `WM_SETTINGCHANGE` broadcast for title bar refresh.
6. **Volume**: PowerShell `AudioDeviceCmdlets` module. Requires one-time setup: `Install-Module -Name AudioDeviceCmdlets`.
7. **Scaling**: Registry DPI (`LogPixels` + `Win8DpiScaling`). Requires logout to apply.
8. **Keep-awake**: `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED)` via Win32 API. Reset with `SetThreadExecutionState(ES_CONTINUOUS)`.
9. **Wallpaper**: Registry keys (`WallpaperStyle` + `TileWallpaper`) for fit mode + `SystemParametersInfoW(SPI_SETDESKWALLPAPER)` via PowerShell P/Invoke. Fit mapping: fill→Style=10/Tile=0, fit→Style=6/Tile=0, stretch→Style=2/Tile=0, center→Style=0/Tile=0, tile→Style=0/Tile=1.

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
11. **Keep-awake**: `systemd-inhibit --what=idle --who=display-dj --why=KeepAwake sleep infinity` as a child process. Kill to disable.
12. **Wallpaper**: Tries in order: `gsettings` (GNOME — `picture-uri` + `picture-options`), `xfconf-query` (XFCE), `feh` fallback. Fit mapping: fill→`zoom`, fit→`scaled`, stretch→`stretched`, center→`centered`, tile→`wallpaper`.

### Windows display dedup (builtin duplicate elimination)

On Windows laptops, the built-in panel often appears in **both** the WMI brightness API and the `ddc_winapi::Monitor::enumerate()` DDC enumeration. Without dedup, the builtin shows up twice: once as a WMI-backed "Built-in Display" and once as a DDC-listed "Generic PnP Monitor" (typically with null VCP brightness since the laptop panel doesn't respond to DDC commands).

**The fix** (`windows.rs`, `enumerate()`):

1. First, check if WMI brightness is available (`BuiltinControl::wmi_get()`). If so, add the built-in display and set `has_builtin = true`.
2. When iterating DDC monitors, pair each with its HMONITOR handle (from `EnumDisplayMonitors`).
3. For each DDC monitor, call `get_hmonitor_details()` to get the `is_primary` flag (`MONITORINFOF_PRIMARY`).
4. **Skip any DDC monitor that is primary when `has_builtin` is true** — this is the duplicate.

**How to verify via `debug` output:**

- `platform.wmi_brightness` is non-null → WMI detected a built-in panel.
- `platform.ddc_monitor_count` may be higher than the final `displays` count — that's expected.
- `platform.hmonitors[N].is_primary == true` identifies which HMONITOR is the builtin.
- The DDC monitor at the same index as the primary HMONITOR typically has `vcp_brightness: null` (laptop panels don't speak DDC).
- The final `displays` array should have exactly one `"builtin"` entry and no duplicate.

**PnP device ID enrichment:** External monitors with the same generic description (e.g., "Generic PnP Monitor") are disambiguated by appending the PnP device identifier from `EnumDisplayDevicesW`. The device ID is extracted from the monitor's hardware path (e.g., `MONITOR\DEL40F4\{guid}\NNNN` → `DEL40F4`), giving names like `"Generic PnP Monitor (DEL40F4)"`.

### Known behaviors

- Some monitors (e.g., Acer XZ322QU V3) return DDC/CI checksum errors on reads and silently ignore writes. These need gamma fallback.
- DDC brightness 0 can cause monitors to enter standby/freeze. Clamped to minimum 1.
- Gamma on a monitor with low DDC backlight produces minimal visible change (the effects multiply).
- The `force` mode (DDC + gamma stacked) provides the most consistent results across mixed monitor setups.
- Scale is clamped to 75%-300% on all platforms to prevent unusable UI.
- macOS scaling switches display modes (resolution-based). Windows requires logout. Linux X11/Wayland applies instantly.
- Keep-awake uses OS-native subprocess/syscall (caffeinate, systemd-inhibit, SetThreadExecutionState) — no external deps. CLI `keep_awake_on` blocks until Ctrl+C. Server mode uses `/keep_awake/enable` and `/keep_awake/disable` for toggle control.
- Wallpaper slideshow uses a static `Mutex<Option<SlideshowState>>` + `Arc<AtomicBool>` cancel flag + background thread. Only one slideshow active at a time — starting a new one cancels the old. Manual wallpaper changes (`/set_wallpaper`, `/set_wallpaper_one`) auto-stop any running slideshow. Timer thread sleeps in 1-second increments to check the cancel flag frequently. For `forward`/`backward` order, the folder is rescanned each tick to pick up new/deleted files. For `random`, reshuffle happens after a full cycle.

## CLI

```bash
# Brightness
display-dj set_all <level> [mode]
display-dj set_one <id|name> <level> [mode]
display-dj get_all
display-dj get_one <id|name>
display-dj list
display-dj reset

# Contrast (DDC only — requires DDC-capable external monitor)
display-dj set_contrast_all <level>
display-dj set_contrast_one <id|name> <level>

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

# Keep-awake
display-dj keep_awake_on
display-dj keep_awake_off
display-dj get_keep_awake

# Wallpaper
display-dj set_wallpaper <fit> <path>
display-dj set_wallpaper_one <index> <fit> <path>
display-dj get_wallpaper
display-dj get_wallpaper_supported
display-dj wallpaper_slideshow_start <interval> <order> <fit> <folder>
display-dj wallpaper_slideshow_stop
display-dj wallpaper_slideshow_status

# Diagnostics
display-dj debug

# Server
display-dj serve [port]
```

### Server endpoints (selected)

```
GET  /health              → {"status":"ok","pid":1234,"uptime":42}
GET  /keep_awake          → {"enabled": true/false}
POST /keep_awake/enable   → {"status":"ok","enabled":true}
POST /keep_awake/disable  → {"status":"ok","enabled":false}
GET  /set_wallpaper/<fit>/<path> → {"ok":true}
GET  /set_wallpaper_one/<index>/<fit>/<path> → {"ok":true}
GET  /get_wallpaper       → {"path":"...","fit":"fill"}
GET  /get_wallpaper_supported → {"supported":true}
GET  /wallpaper_slideshow_start/<interval>/<order>/<fit>/<folder> → {"ok":true,"total_images":12,"current_image":"..."}
GET  /wallpaper_slideshow_stop → {"ok":true,"was_running":true}
GET  /wallpaper_slideshow_status → {"running":true,"folder":"...","interval_minutes":30,...}
```

## GitHub Raw File URLs

When fetching raw file content from GitHub repos, always use `raw.githubusercontent.com` (CORS-friendly):

https://raw.githubusercontent.com/{owner}/{repo}/HEAD/{path}

This format works for all use cases (browser fetch with CORS, curl/shell scripts, direct browser links).

Do NOT use:

- `https://github.com/{owner}/{repo}/blob/HEAD/{path}?raw=1` (no CORS headers, breaks browser fetch)
- `https://api.github.com/repos/{owner}/{repo}/contents/{path}` (returns JSON, not raw content)


## CI / Release Workflows

Release workflows use shared composite actions from `synle/gha-workflows/actions/release/`.

**release-official.yml** (workflow_dispatch only):
- `begin-release` resolves version from `Cargo.toml` (or manual input), cleans up any existing release, creates a draft placeholder tagged `v{version}`.
- Build matrix runs on 6 platforms: macOS ARM64/x64, Windows x64/ARM64, Linux x64/ARM64. Tests run on 3 primary platforms (macOS ARM64, Windows x64, Linux x64). Linux ARM64 cross-compiles with `gcc-aarch64-linux-gnu`.
- Release job uses low-level `_common/notes` and `_common/finalize` actions (instead of `end-release`) because it needs to download artifacts, then append a repo-specific downloads table between notes generation and finalize.
- On success: published release (not draft, not prerelease, marked latest). On failure: draft release with `[Error]` title suffix.

**release-beta.yml** (workflow_dispatch only):
- `begin-release` generates a `release-beta-{date}-{sha}` tag.
- Same 6-platform build matrix as official.
- `end-release` handles notes + finalize in one step (no downloads table needed for beta).
- On success: draft prerelease with `[Success]` title suffix. On failure: draft with `[Error]` title suffix.

**Interactive triggering:** Use `/release-official` or `/release-beta` skills to trigger workflows, view changelogs, and watch runs from Claude Code.

## Git / PR Merge Policy

- Always use **squash and merge** when merging PRs. Never use merge commits or rebase merges. This keeps the git history clean with one commit per PR.
- You may `git merge origin/main` or `git merge origin/master` locally to sync branches, but PR merges must always be squash merges.
