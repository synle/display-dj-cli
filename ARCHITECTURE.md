# display-dj-cli — Architecture

## High-Level Overview

`display-dj` is a single-binary, cross-platform Rust CLI for controlling display brightness, contrast, gamma, scaling, system volume, dark/light theme, wallpaper, and keep-awake. It targets macOS, Windows, and Linux from one source tree, selecting the correct platform backend at compile time via `#[cfg(target_os = ...)]`.

Runtime model:

- Built as a release binary with `cargo build --release` (no runtime, no installer).
- One process per invocation. Stateless except for the optional `serve` mode, which exposes the same commands over HTTP (default port `51337`) for long-lived integrations (Stream Deck, hotkey daemons, the companion Tauri UI).
- Human-readable output goes to **stderr**; structured **JSON** goes to **stdout** so the CLI composes cleanly in scripts.

Main flow:

1. `main()` parses `argv` and dispatches to a per-command handler.
2. The handler calls into the platform module through two traits — `Platform` (enumeration / global ops) and `DisplayControl` (per-display read/write).
3. Each platform module talks to the OS via native APIs (CoreGraphics + DisplayServices on macOS, Win32 + WMI + PowerShell on Windows, sysfs + xrandr/wlr-randr + pactl/amixer on Linux). External monitors everywhere go through DDC/CI (`ddc` crate).
4. The handler prints JSON to stdout or returns an HTTP response from `serve`.

Shared primitives live at the crate root (`src/main.rs`): `DisplayInfo`, `DisplayControl` trait, `Platform` trait, VCP register constants (`VCP_BRIGHTNESS = 0x10`, `VCP_CONTRAST = 0x12`), and the `BUILTIN_ID = "builtin"` sentinel.

## Key Directories

| Path | Purpose |
|------|---------|
| `src/` | All Rust sources. Single crate, single binary (`display-dj`). |
| `src/main.rs` | Entry point, CLI parser, command dispatcher, HTTP server, shared traits and types, plus cross-cutting features (theme, volume, scale, wallpaper, keep-awake) implemented inline with `#[cfg(...)]` per OS. |
| `src/macos.rs` | macOS backend — CoreGraphics FFI for gamma, DisplayServices (private framework, `dlopen`/`dlsym`) for built-in brightness, `ddc-macos` for external monitors. |
| `src/windows.rs` | Windows backend — Win32 GDI `SetDeviceGammaRamp` for gamma, WMI via PowerShell for built-in brightness, `ddc-winapi` for external monitors. |
| `src/linux.rs` | Linux backend — `/sys/class/backlight` (or `brightnessctl`) for built-in, `ddc` (i2c-dev) for external, `xrandr`/`wlr-randr` for gamma based on detected display server (X11 / Wayland / unknown). |
| `tests/` | Integration tests that exec the built binary and assert stdout/stderr/exit-code contracts. Coverage is enforced in CI. |
| `tests/cli.rs` | The integration test suite. |
| `.github/workflows/` | CI/CD: cross-platform build matrix, coverage gate, official + beta release pipelines, artifact cleanup. |

## Important Files

- **`Cargo.toml`** — Crate manifest. Pins shared deps (`ddc`, `serde`, `serde_json`) and target-gated deps:
  - `cfg(target_os = "macos")` → `ddc-macos`, `libc`
  - `cfg(target_os = "windows")` → `ddc-winapi`, `windows` (with `Win32_Devices_Display`, `Win32_Graphics_Gdi`, `Win32_Foundation` features), `winapi` (with `wingdi`, `windef`)
  - Linux has no extra target deps — it shells out to system tools instead.
- **`src/main.rs`** — Single ~3k-line entry module. Declares the platform module for the current target with `mod macos | mod windows | mod linux`, defines the `Platform` and `DisplayControl` traits, dispatches all subcommands (`set_all`, `set_one`, `get_all`, `get_one`, `list`, `reset`, `dark`, `light`, `theme`, `get_volume`, `set_volume`, `mute`, `unmute`, `set_contrast_*`, `get_scale`, `set_scale_*`, `keep_awake_*`, `set_wallpaper*`, `get_wallpaper*`, `wallpaper_slideshow_*`, `debug`, `serve`), and implements the HTTP server used by `serve`.
- **`src/macos.rs` / `src/windows.rs` / `src/linux.rs`** — Each implements `Platform::enumerate()`, `Platform::reset_all_gamma()`, `Platform::debug_info()`, and a `DisplayControl` impl for both the built-in panel and each external (DDC/CI) monitor.
- **`tests/cli.rs`** — Integration tests; the only test target. Drives coverage numbers reported by the `rust_coverage` CI job.
- **`.github/workflows/build.yml`** — Build matrix (macOS arm64/x64, Windows x64/arm64, Linux x64/arm64) plus the `rust_coverage` job that runs `cargo llvm-cov`, posts a Markdown summary, and fails the build if lines / regions / functions drop below `MIN_LINES=60`, `MIN_REGIONS=60`, `MIN_FUNCTIONS=60`. Note: only Linux-compiled code is measured; `#[cfg(target_os = "macos"|"windows")]` blocks are excluded by definition.
- **`.github/workflows/release-official.yml` / `release-beta.yml`** — Tag-driven release pipelines that build the same matrix and publish binaries as GitHub Releases.
- **`.github/workflows/cleanup-artifacts.yml`** — Periodic artifact retention/cleanup.
- **`CLAUDE.md`, `CONTRIBUTING.md`, `DEV.md`, `README.md`** — Contributor- and user-facing docs (out of scope here).

## Build & Release Flow

- **Build:** `cargo build --release --target <triple>`. The build matrix in `build.yml` cross-compiles for six targets; Linux arm64 installs `gcc-aarch64-linux-gnu` and writes a `~/.cargo/config.toml` linker override. Tests run on the three "native" targets (`aarch64-apple-darwin`, `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`) — cross-compiled targets skip tests.
- **Coverage gate:** `rust_coverage` runs `cargo llvm-cov`, emits `rust-coverage.json` + `rust-coverage.lcov`, uploads them as the `rust-coverage` artifact (14-day retention), posts a per-metric table to `$GITHUB_STEP_SUMMARY`, and fails on regression.
- **Artifacts:** Per-target binaries upload to `actions/upload-artifact@v7` with 90-day retention. Names follow `display-dj-<os>-<arch>[.exe]`.
- **Releases:** `release-official.yml` and `release-beta.yml` are dispatched against `v*` tags. Beta releases keep a separate channel; official releases are cut from `main` and are the supported channel.
