# Developer Guide

## Architecture

Platform-abstracted Rust binary with a shared interface (`main.rs`) and per-OS platform modules. Only one platform module is compiled per build via `#[cfg(target_os)]`.

```
                    ┌─────────────────────────────┐
                    │          main.rs             │
                    │  CLI parsing, dispatch,      │
                    │  shared types & traits       │
                    │  (Platform, DisplayControl)  │
                    └──────┬──────┬──────┬─────────┘
                           │      │      │
              #[cfg(macos)]│      │      │#[cfg(linux)]
                           │      │      │
                    ┌──────┘      │      └──────┐
                    ▼      #[cfg(windows)]       ▼
              ┌──────────┐  ▼    ┌──────────┐  ┌──────────┐
              │ macos.rs │       │windows.rs│  │ linux.rs │
              └──────────┘       └──────────┘  └──────────┘
```

**Key traits:**

- `Platform` — static methods: `enumerate()`, `reset_all_gamma()`, `debug_info()`
- `DisplayControl` — per-display instance methods: `get_brightness`, `set_brightness`, `get_contrast`, `set_contrast`, `reset_gamma`

**Display ID scheme:** `builtin` (or `0`) for the built-in panel, `1`, `2`, etc. for externals. Lookup supports both ID and monitor name (case-insensitive).

**OS-level features** (dark mode, volume, scaling) live in `main.rs` behind `#[cfg(target_os)]` blocks — they're not per-display (except scaling which is per-display).

## Directory Structure

```
display-dj-cli/
├── Cargo.toml          # Package manifest — deps, per-platform deps, metadata
├── Cargo.lock          # Pinned dependency versions
├── src/
│   ├── main.rs         # Entry point, CLI parsing, dispatch, shared types & traits,
│   │                   #   dark mode, volume, scaling, HTTP server
│   ├── macos.rs        # macOS: DisplayServices FFI, ddc-macos, CoreGraphics gamma
│   ├── windows.rs      # Windows: WMI brightness, ddc-winapi, GDI32 gamma, PnP dedup
│   └── linux.rs        # Linux: sysfs/brightnessctl, ddcutil, xrandr/wlr-randr gamma
├── .github/
│   └── workflows/
│       ├── build.yml   # CI: builds 6 targets (macOS/Windows/Linux x arm64/x64)
│       └── release.yml # Release workflow
├── CLAUDE.md           # AI assistant context (architecture deep-dive)
├── CONTRIBUTING.md     # Contributor guide with Rust syntax reference
└── README.md           # User-facing docs
```

### File Responsibilities

| File | Role |
|---|---|
| `Cargo.toml` | Dependencies, project metadata, per-platform conditional deps |
| `src/main.rs` | Entry point. Shared types (`DisplayInfo`), traits (`Platform`, `DisplayControl`), CLI parsing, command dispatch, HTTP server, and cross-platform dark mode/volume/scaling/keep-awake |
| `src/macos.rs` | macOS `Platform` + `DisplayControl` impl. CoreGraphics FFI for gamma, DisplayServices private framework for built-in brightness, `ddc-macos` crate for DDC/CI |
| `src/windows.rs` | Windows `Platform` + `DisplayControl` impl. Win32 DDC/CI, WMI via PowerShell for built-in, gamma ramp via GDI32, HMONITOR dedup, PnP device ID enrichment |
| `src/linux.rs` | Linux `Platform` + `DisplayControl` impl. sysfs/`brightnessctl` for built-in, `ddcutil` for DDC/CI, `xrandr`/`wlr-randr`/`wl-gammarelay-rs` for gamma. Runtime display server detection |

### Dependency Graph

```
main.rs (entry point, shared types & traits)
  ├── macos.rs   ─── imports from main: DisplayInfo, DisplayControl, Platform, constants
  │                   external crates: ddc, ddc-macos, libc
  ├── windows.rs ─── imports from main: DisplayInfo, DisplayControl, Platform, constants
  │                   external crates: ddc, ddc-winapi, windows (Win32), winapi
  └── linux.rs   ─── imports from main: DisplayInfo, DisplayControl, Platform, constants
                      external crates: (none — uses CLI tools via std::process::Command)
```

## Request Lifecycle

Here's a trace of what happens when a user runs `display-dj set_one 2 50`:

1. **CLI args parsed** — `main()` collects `std::env::args()`, extracts `cmd = "set_one"`
2. **Platform dispatch** — `#[cfg(target_os)]` selects the concrete platform type, calls `dispatch::<MacPlatform>(cmd, &args)` (or WinPlatform/LinuxPlatform)
3. **Command match** — `dispatch()` matches `"set_one"`, parses `id = "2"`, `level = 50`, `mode = "force"` (default)
4. **Enumerate displays** — `P::enumerate()` returns `Vec<(DisplayInfo, Box<dyn DisplayControl>)>` — all detected displays with their control handles
5. **Display lookup** — `matches_display(&info, "2")` finds the target by ID or name
6. **Set brightness** — calls `ctrl.set_brightness(50, "force")` on the matched display's `DisplayControl` impl
7. **Platform-specific execution** — depending on mode:
   - `ddc`: sends DDC/CI I2C command (VCP code `0x10`, value `50`)
   - `gamma`: adjusts GPU color curve via OS API
   - `force`: does both DDC + gamma
8. **Status output** — prints result to stderr (`OK` / `FAILED`)
9. **Keep-alive** — if gamma was used, `maybe_keep_alive()` blocks forever (gamma resets on process exit on macOS/Windows)

## Building

```bash
# Debug build (fast compile, slow runtime)
cargo build

# Release build (slow compile, optimized binary)
cargo build --release

# Cross-compile for a specific target
cargo build --release --target aarch64-apple-darwin

# Run directly
cargo run -- list
cargo run -- set_all 50
cargo run -- debug
```

The binary name is `display-dj` (from `Cargo.toml` package name). Debug builds go to `target/debug/`, release to `target/release/`.

### Cross-compilation targets

| Target | Platform |
|--------|----------|
| `aarch64-apple-darwin` | macOS ARM (Apple Silicon) |
| `x86_64-apple-darwin` | macOS Intel |
| `x86_64-pc-windows-msvc` | Windows x64 |
| `aarch64-pc-windows-msvc` | Windows ARM |
| `x86_64-unknown-linux-gnu` | Linux x64 |
| `aarch64-unknown-linux-gnu` | Linux ARM (needs cross-linker) |

### Running tests

```bash
cargo test
```

## Platform Implementation Details

### macOS (`src/macos.rs`)

| Feature | Implementation |
|---------|---------------|
| Built-in brightness | `DisplayServicesGetBrightness` / `DisplayServicesSetBrightness` — private framework loaded via `dlopen`/`dlsym` at runtime |
| External DDC/CI | `ddc-macos` crate — `IOAVServiceWriteI2C`/`IOAVServiceReadI2C` (Apple Silicon) or `IOI2CSendRequest` (Intel) |
| Gamma | `CGSetDisplayTransferByFormula` — resets on process exit |
| Dark mode | `osascript` via System Events |
| Volume | `osascript` — `get volume settings` / `set volume output volume` |
| Scaling | CoreGraphics native FFI — `CGDisplayCopyAllDisplayModes` + `CGDisplaySetDisplayMode` |
| Keep-awake | `caffeinate -di` child process (pre-installed) |
| Wallpaper | `osascript` via System Events (`tell every desktop to set picture`) |

### Windows (`src/windows.rs`)

| Feature | Implementation |
|---------|---------------|
| Built-in brightness | WMI `WmiMonitorBrightness` / `WmiMonitorBrightnessMethods` via PowerShell |
| External DDC/CI | `ddc-winapi` crate — Win32 Dxva2 |
| Gamma | `SetDeviceGammaRamp` via GDI32 — resets on process exit |
| Dark mode | Registry keys `AppsUseLightTheme` + `SystemUsesLightTheme` + `WM_SETTINGCHANGE` broadcast |
| Volume | PowerShell `AudioDeviceCmdlets` module |
| Scaling | Registry DPI (`LogPixels` + `Win8DpiScaling`) — requires logout |
| Keep-awake | `SetThreadExecutionState` Win32 API (ES_CONTINUOUS + ES_SYSTEM_REQUIRED + ES_DISPLAY_REQUIRED) |
| Wallpaper | Registry (`WallpaperStyle` + `TileWallpaper`) + `SystemParametersInfoW` via PowerShell |

**Builtin dedup:** On laptops, the built-in panel appears in both WMI and DDC enumeration. The enumerate code checks `MONITORINFOF_PRIMARY` via `GetMonitorInfoW` and skips the primary HMONITOR from DDC when a WMI builtin was already detected. See CLAUDE.md for full details.

**PnP enrichment:** External monitors with the same generic name get disambiguated with PnP device IDs from `EnumDisplayDevicesW` (e.g., `"Generic PnP Monitor (DEL40F4)"`).

### Linux (`src/linux.rs`)

| Feature | Implementation |
|---------|---------------|
| Built-in brightness | `/sys/class/backlight/*/brightness` sysfs or `brightnessctl` fallback |
| External DDC/CI | `ddcutil` CLI — VCP codes over i2c-dev |
| Gamma (X11) | `xrandr --output <name> --brightness <val>` — persists after exit |
| Gamma (Wayland) | `wlr-randr` > `wl-gammarelay-rs` (busctl) > XWayland xrandr fallback |
| Display server detection | `XDG_SESSION_TYPE` env var, then `WAYLAND_DISPLAY` / `DISPLAY` fallback |
| Dark mode | `gsettings` (GNOME) > `plasma-apply-colorscheme` (KDE) > `xfconf-query` (XFCE) |
| Volume | `pactl` (PulseAudio/PipeWire) with `amixer` (ALSA) fallback |
| Scaling | `xrandr --scale` (X11, inverse) or `wlr-randr --scale` (Wayland, direct) |
| Keep-awake | `systemd-inhibit --what=idle --who=display-dj sleep infinity` child process |
| Wallpaper | `gsettings` (GNOME), `xfconf-query` (XFCE), `feh` fallback |

## Output Conventions

- **stdout** — JSON only (machine-readable, safe to pipe)
- **stderr** — human-readable status messages, errors, usage text

This means `display-dj get_all 2>/dev/null | jq` always works.

## Known Behaviors & Gotchas

- **DDC brightness 0 = standby** — setting DDC brightness to 0 can freeze monitors. Clamped to minimum 1 on all platforms.
- **Gamma persistence** — macOS/Windows reset gamma on process exit. Linux (xrandr/wlr-randr) persists. This is why `maybe_keep_alive()` blocks the process in `force`/`gamma` modes.
- **Parent-death detection** — In server mode (`cmd_serve`), a background thread reads from stdin. When the parent process exits (closing the pipe), stdin returns EOF and the server shuts down via `process::exit(0)`. This prevents orphaned sidecar processes when the parent crashes or is force-quit.
- **DDC checksum errors** — some monitors (e.g., Acer XZ322QU V3) return checksum errors on reads and silently ignore writes. The `force` mode stacks DDC + gamma as a reliable fallback.
- **Gamma + low DDC backlight** — minimal visible change because the effects multiply.
- **Scale clamping** — 75%-300% on all platforms to prevent unusable UI.
- **macOS scaling** — switches display modes (resolution-based). Windows requires logout. Linux applies instantly.

## CI / CD

GitHub Actions builds 6 binaries on every push to `main` and on PRs:

- macOS ARM64 + x64
- Windows x64 + ARM64
- Linux x64 + ARM64

Tests run on native runners (macOS ARM, Windows x64, Linux x64). Cross-compiled targets (macOS x64, Windows ARM, Linux ARM) skip tests. See `.github/workflows/build.yml`.

## Where to Edit

| Task | Where |
|------|-------|
| Add a new CLI command | `main.rs` — add to `dispatch()` match, create `cmd_*` function |
| Add a new HTTP route | `main.rs` — find the `serve` command handler, add route matching |
| Fix macOS display detection | `src/macos.rs` — `MacPlatform::enumerate()` |
| Fix Windows display dedup | `src/windows.rs` — `WinPlatform::enumerate()`, `get_hmonitor_details()` |
| Fix Linux display detection | `src/linux.rs` — `LinuxPlatform::enumerate()`, display server detection |
| Add a new brightness mode | Each platform file's `set_brightness()` impl + update `main.rs` dispatch |
| Change dark mode behavior | `main.rs` — `cmd_theme()` function, behind `#[cfg(target_os)]` blocks |
| Change volume behavior | `main.rs` — `cmd_get_volume()` / `cmd_set_volume()`, behind `#[cfg(target_os)]` blocks |
| Change scaling behavior | `main.rs` — `cmd_get_scale()` / `cmd_set_scale_*()`, behind `#[cfg(target_os)]` blocks |
| Change keep-awake behavior | `main.rs` — `enable_keep_awake()` / `disable_keep_awake()` / `is_keep_awake_active()`, behind `#[cfg(target_os)]` blocks |
| Change wallpaper behavior | `main.rs` — `set_wallpaper()` / `get_wallpaper()` / `is_wallpaper_supported()`, behind `#[cfg(target_os)]` blocks |
| Change slideshow behavior | `main.rs` — `slideshow_start()` / `slideshow_stop()` / `slideshow_status()` / `slideshow_cancel()`, `SLIDESHOW` static Mutex |
| Add a new shared type | `main.rs` — add struct with `#[derive(Serialize, Clone)]` |
| Add a platform dependency | `Cargo.toml` — under `[target.'cfg(target_os = "...")'.dependencies]` |
