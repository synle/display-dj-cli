# Contributing

## Project Structure

```
ddc-test/
├── Cargo.toml          # Package manifest (like package.json)
├── src/
│   ├── main.rs         # Entry point — CLI parsing, dispatch, shared types & traits
│   ├── macos.rs        # macOS impl — CoreGraphics FFI, DisplayServices private API, ddc-macos
│   ├── windows.rs      # Windows impl — Win32 DDC/CI, WMI brightness via PowerShell, gamma ramp
│   └── linux.rs        # Linux impl — sysfs/brightnessctl, ddcutil, X11/Wayland gamma (xrandr/wlr-randr)
└── target/             # Build output (like node_modules + dist combined, gitignored)
```

### File Responsibilities

| File | Role | Node Analogy |
|---|---|---|
| `Cargo.toml` | Dependencies, project metadata, per-platform deps | `package.json` |
| `src/main.rs` | Entry point. Defines shared types (`DisplayInfo`), traits (`Platform`, `DisplayControl`), CLI parsing, command dispatch, and cross-platform dark mode toggle. Only this file is compiled on all platforms. | `index.js` + `types.ts` |
| `src/macos.rs` | macOS `Platform` and `DisplayControl` implementations. Uses CoreGraphics FFI for gamma, DisplayServices private framework for built-in brightness, and `ddc-macos` crate for external monitors via DDC/CI. | `platforms/macos.js` |
| `src/windows.rs` | Windows `Platform` and `DisplayControl` implementations. Uses Win32 API for DDC/CI, WMI via PowerShell for built-in laptop brightness, gamma ramp for software dimming. | `platforms/windows.js` |
| `src/linux.rs` | Linux `Platform` and `DisplayControl` implementations. Uses sysfs/`brightnessctl` for built-in, `ddcutil` for DDC/CI, `xrandr` (X11) or `wlr-randr`/`wl-gammarelay-rs` (Wayland) for gamma. Detects display server at runtime. | `platforms/linux.js` |

### Dependency Graph

```
main.rs (entry point, shared types & traits)
  ├── macos.rs   ─── imports from main: DisplayInfo, DisplayControl, Platform, constants
  │                   external crates: ddc, ddc-macos, libc
  ├── windows.rs ─── imports from main: DisplayInfo, DisplayControl, Platform, constants
  │                   external crates: ddc, ddc-winapi, windows (Win32)
  └── linux.rs   ─── imports from main: DisplayInfo, DisplayControl, Platform, constants
                      external crates: (none — uses CLI tools via std::process::Command)
```

Only one platform module is compiled per build (selected by `#[cfg(target_os = "...")]`). The platform modules implement the `Platform` and `DisplayControl` traits defined in `main.rs`, and `main.rs` calls them generically via `dispatch::<MacPlatform>(...)`.

---

## CLI API Reference

All commands write **JSON to stdout** (for piping/parsing) and **human-readable messages to stderr** (for the terminal). This separation means you can safely do `display-dj get_all 2>/dev/null | jq`.

### Commands

#### `list`

List all detected displays without reading live brightness values.

```
display-dj list
```

- **stdout**: JSON array of `DisplayInfo`
- **stderr**: (none)
- **exit code**: 0

```json
[
  {
    "id": "builtin",
    "name": "Built-in Display",
    "display_type": "builtin",
    "brightness": 20,
    "contrast": null,
    "ddc_supported": false
  },
  {
    "id": "1",
    "name": "XZ322QU V3",
    "display_type": "external",
    "brightness": null,
    "contrast": null,
    "ddc_supported": false
  },
  {
    "id": "2",
    "name": "VX2718-2KPC",
    "display_type": "external",
    "brightness": 20,
    "contrast": 70,
    "ddc_supported": true
  }
]
```

#### `get_all`

Get live brightness/contrast readings for all displays. Same JSON shape as `list`, but `brightness` and `contrast` are re-read from the hardware.

```
display-dj get_all
```

- **stdout**: JSON array of `DisplayInfo` (with fresh brightness/contrast reads)
- **stderr**: (none)
- **exit code**: 0

```json
[
  {
    "id": "builtin",
    "name": "Built-in Display",
    "display_type": "builtin",
    "brightness": 20,
    "contrast": null,
    "ddc_supported": false
  },
  {
    "id": "1",
    "name": "XZ322QU V3",
    "display_type": "external",
    "brightness": null,
    "contrast": null,
    "ddc_supported": false
  },
  {
    "id": "2",
    "name": "VX2718-2KPC",
    "display_type": "external",
    "brightness": 20,
    "contrast": 70,
    "ddc_supported": true
  }
]
```

#### `get_one <id|name>`

Get live brightness/contrast for a single display.

```
display-dj get_one builtin
display-dj get_one 2
display-dj get_one "VX2718-2KPC"
```

- **stdout**: Single `DisplayInfo` JSON object (not an array)
- **stderr**: Error message if display not found
- **exit code**: 0 on success, 1 if not found

```json
{
  "id": "2",
  "name": "VX2718-2KPC",
  "display_type": "external",
  "brightness": 20,
  "contrast": 70,
  "ddc_supported": true
}
```

#### `set_all <level> [mode]`

Set brightness on all displays. Level is 0-100. Mode defaults to `force`.

```
display-dj set_all 50
display-dj set_all 30 ddc
```

- **stdout**: (none)
- **stderr**: Status per display (`OK` / `FAILED`)
- **exit code**: 0
- **Note**: Process stays alive in `force` and `gamma` modes (gamma resets on exit)

```
Setting all 3 display(s) to 50% [mode=force]

  builtin (Built-in Display): OK
  1 (XZ322QU V3): OK
  2 (VX2718-2KPC): OK

Press Ctrl+C to exit (gamma will reset).
```

#### `set_one <id|name> <level> [mode]`

Set brightness on a single display. Level is 0-100. Mode defaults to `force`.

```
display-dj set_one builtin 80
display-dj set_one 2 30 ddc
display-dj set_one "VX2718-2KPC" 50
```

- **stdout**: (none)
- **stderr**: Status message (`OK` / `FAILED`), or error if not found
- **exit code**: 0 on success, 1 if not found
- **Note**: Process stays alive in `force` and `gamma` modes

#### `reset`

Reset gamma tables to system defaults on all displays.

```
display-dj reset
```

- **stdout**: (none)
- **stderr**: `Gamma reset to defaults.`
- **exit code**: 0

#### `dark`

Switch the system to dark mode.

```
display-dj dark
```

- **stdout**: (none)
- **stderr**: `Switched to dark mode.` or `Failed to switch to dark mode.`
- **exit code**: 0 on success, 1 on failure

#### `light`

Switch the system to light mode.

```
display-dj light
```

- **stdout**: (none)
- **stderr**: `Switched to light mode.` or `Failed to switch to light mode.`
- **exit code**: 0 on success, 1 on failure

#### `theme`

Get the current system theme (dark or light).

```
display-dj theme
```

- **stdout**: JSON `ThemeInfo` object
- **stderr**: Error message if theme can't be detected
- **exit code**: 0 on success, 1 if detection fails

```json
{
  "theme": "dark"
}
```

#### `debug`

Dump full diagnostics for all displays, volume, and theme. Runs active tests: sets brightness to 25% on each display (DDC, gamma, force modes), toggles volume (25/100/mute/unmute), toggles theme (dark/light), and restores everything to original state. Also includes raw platform data (HMONITOR details, DDC VCP reads, PnP device IDs, etc.).

```
display-dj debug
```

- **stdout**: JSON object with `displays`, `scale`, `platform` (raw data), and `tests` (active results)
- **stderr**: Progress messages (`Testing display 1...`, etc.)
- **exit code**: 0

The `tests.displays[]` section shows per-display results:

```json
{
  "id": "1",
  "name": "VX2718-2KPC",
  "ddc_supported": true,
  "initial_brightness": 100,
  "set_brightness_25_ddc": true,
  "get_after_ddc": 25,
  "set_brightness_25_gamma": true,
  "get_after_gamma": 25,
  "set_brightness_25_force": true,
  "get_after_force": 25,
  "restore_brightness": true,
  "get_after_restore": 100,
  "set_contrast_50": true,
  "get_after_contrast_set": 50
}
```

Available via HTTP at `GET /debug`.

#### `set_contrast_all <level>`

Set contrast on all displays. Level is 0-100. Only works on DDC-capable external monitors — built-in displays and non-DDC monitors will report `FAILED`.

```
display-dj set_contrast_all 50
```

- **stdout**: (none)
- **stderr**: Status per display (`OK` / `FAILED`)
- **exit code**: 0

#### `set_contrast_one <id|name> <level>`

Set contrast on a single display. Level is 0-100.

```
display-dj set_contrast_one 2 70
display-dj set_contrast_one "VX2718-2KPC" 50
```

- **stdout**: (none)
- **stderr**: Status message (`OK` / `FAILED`), or error if not found
- **exit code**: 0 on success, 1 if not found

Available via HTTP at `GET /set_contrast_all/<level>` and `GET /set_contrast_one/<id>/<level>`.

### JSON Schemas

#### `DisplayInfo`

Returned by `list`, `get_all`, and `get_one`.

| Field | Type | Description |
|---|---|---|
| `id` | `string` | `"builtin"`, `"1"`, `"2"`, ... |
| `name` | `string` | Human-readable display name |
| `display_type` | `string` | `"builtin"` or `"external"` |
| `brightness` | `number \| null` | Current brightness 0-100, or `null` if unreadable |
| `contrast` | `number \| null` | Current contrast 0-100, or `null` if unsupported |
| `ddc_supported` | `boolean` | Whether DDC/CI communication works with this display |

#### `ThemeInfo`

Returned by `theme`.

| Field | Type | Description |
|---|---|---|
| `theme` | `string` | `"dark"` or `"light"` |

### Display Lookup

Commands that take `<id|name>` accept:

- **By ID**: exact match -- `"builtin"`, `"1"`, `"2"`, etc.
- **By name**: case-insensitive -- `"Dell U2723QE"` matches `"dell u2723qe"`

ID is checked first. If two monitors share a name, the first match wins; use numeric IDs to disambiguate.

### Modes

| Mode | Behavior | Process exits? |
|---|---|---|
| `force` (default) | DDC + gamma stacked on every external | No (stays alive -- gamma resets on exit) |
| `auto` | DDC where supported, gamma as fallback | Depends on whether gamma was used |
| `ddc` | DDC/CI only, monitors without DDC skipped | Yes |
| `gamma` | Gamma tables only (software dimming) | No (stays alive) |

Built-in displays always use the platform-native brightness API regardless of mode.

### Exit Codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Error (missing args, display not found, operation failed, unsupported platform) |

### Output Channels

| Channel | Content | Purpose |
|---|---|---|
| **stdout** | JSON only (`list`, `get_all`, `get_one`, `theme`) | Machine-readable, safe to pipe |
| **stderr** | Status messages, errors, usage text | Human-readable terminal output |

---

## Integrating Into a Tauri (Rust + JS) App

This section shows how to replace platform-specific Rust display code with `display-dj` in a Tauri v2 app. This is the approach used by [Display DJ v2](../display-dj2).

### 1. Bundle the binary as a Tauri sidecar

Add to `src-tauri/tauri.conf.json`:

```json
{
  "bundle": {
    "externalBin": ["binaries/display-dj"]
  }
}
```

Place the platform binaries in `src-tauri/binaries/` with Tauri's naming convention:

```
src-tauri/binaries/
  display-dj-aarch64-apple-darwin      # macOS ARM
  display-dj-x86_64-apple-darwin       # macOS Intel
  display-dj-x86_64-pc-windows-msvc.exe  # Windows x64
  display-dj-aarch64-pc-windows-msvc.exe # Windows ARM
  display-dj-x86_64-unknown-linux-gnu  # Linux x64
  display-dj-aarch64-unknown-linux-gnu # Linux ARM
```

Download these from the [GitHub Releases](../../releases) or build locally with `cargo build --release --target <target>`.

### 2. Rust backend — call the sidecar

Replace your platform-specific `display.rs` with simple sidecar calls:

```rust
use tauri::Manager;
use tauri_plugin_shell::ShellExt;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub display_type: String,
    pub brightness: Option<u32>,
    pub contrast: Option<u32>,
    pub ddc_supported: bool,
}

#[tauri::command]
pub async fn get_monitors(app: tauri::AppHandle) -> Result<Vec<Monitor>, String> {
    let output = app.shell()
        .sidecar("display-dj").unwrap()
        .args(["get_all"])
        .output().await
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_brightness(
    app: tauri::AppHandle,
    monitor_id: String,
    value: u32,
) -> Result<(), String> {
    let output = app.shell()
        .sidecar("display-dj").unwrap()
        .args(["set_one", &monitor_id, &value.to_string()])
        .output().await
        .map_err(|e| e.to_string())?;
    if output.status.success() { Ok(()) }
    else { Err(String::from_utf8_lossy(&output.stderr).to_string()) }
}

#[tauri::command]
pub async fn set_all_brightness(app: tauri::AppHandle, value: u32) -> Result<(), String> {
    let output = app.shell()
        .sidecar("display-dj").unwrap()
        .args(["set_all", &value.to_string()])
        .output().await
        .map_err(|e| e.to_string())?;
    if output.status.success() { Ok(()) }
    else { Err(String::from_utf8_lossy(&output.stderr).to_string()) }
}

#[tauri::command]
pub async fn get_theme(app: tauri::AppHandle) -> Result<String, String> {
    let output = app.shell()
        .sidecar("display-dj").unwrap()
        .args(["theme"])
        .output().await
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[tauri::command]
pub async fn set_dark_mode(app: tauri::AppHandle, dark: bool) -> Result<(), String> {
    let cmd = if dark { "dark" } else { "light" };
    let output = app.shell()
        .sidecar("display-dj").unwrap()
        .args([cmd])
        .output().await
        .map_err(|e| e.to_string())?;
    if output.status.success() { Ok(()) }
    else { Err(String::from_utf8_lossy(&output.stderr).to_string()) }
}
```

### 3. Frontend — call via invoke (same as before)

The TypeScript side doesn't change. It calls `invoke()` as usual:

```typescript
import { invoke } from '@tauri-apps/api/core';

// List monitors
const monitors = await invoke<Monitor[]>('get_monitors');

// Set brightness
await invoke('set_brightness', { monitorId: '1', value: 50 });

// Set all
await invoke('set_all_brightness', { value: 80 });

// Dark mode
await invoke('set_dark_mode', { dark: true });
const theme = await invoke<string>('get_theme');
```

### 4. Alternative: HTTP server mode

If you need gamma persistence on macOS (the sidecar process exits after each call, resetting gamma), start the server as a long-lived sidecar:

```rust
// In your Tauri setup, spawn the server once:
use tauri_plugin_shell::ShellExt;

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // Start display-dj HTTP server as background sidecar.
    // Tauri's shell plugin pipes stdin automatically. When this app exits
    // (even via crash/force-quit), stdin closes and the sidecar shuts down.
    let (mut _rx, _child) = app.shell()
        .sidecar("display-dj").unwrap()
        .args(["serve"])
        .spawn()
        .expect("failed to start display-dj server");
    Ok(())
}
```

> **Note:** The sidecar monitors stdin for EOF. When the parent process exits, stdin closes and the server shuts down automatically — no explicit kill needed for crash recovery.

Then call via HTTP from the frontend directly (no Rust invoke needed):

```typescript
const BASE = 'http://127.0.0.1:51337';

const monitors = await fetch(`${BASE}/list`).then(r => r.json());
await fetch(`${BASE}/set_one/1/50`);
await fetch(`${BASE}/set_all/80`);
await fetch(`${BASE}/dark`);
```

### Sidecar vs HTTP — which to use?

| | Sidecar (spawn per command) | HTTP server |
|---|---|---|
| Gamma persistence (macOS) | No — process exits, gamma resets | Yes — server stays alive |
| DDC-only mode | Works fine | Also works |
| Simplicity | Simpler Rust code | Simpler frontend code |
| Startup latency | ~50ms per call | ~1ms per request |
| Best for | DDC monitors, dark mode, reads | Mixed DDC + gamma setups |

### What this replaces

Using `display-dj` as a sidecar eliminates all platform-specific code from your Tauri app:

| Before (in Rust) | After (via display-dj) |
|---|---|
| `ddc-macos` crate + IOKit FFI | `display-dj get_all` / `set_one` |
| `CoreGraphics` gamma FFI | `display-dj set_one <id> <level> force` |
| `DisplayServices` dlopen/dlsym | `display-dj set_one builtin <level>` |
| Win32 `Dxva2` + `SetDeviceGammaRamp` | Same CLI, different binary |
| WMI PowerShell brightness | Same CLI |
| `ddcutil` / `xrandr` / `brightnessctl` | Same CLI |
| `osascript` dark mode toggle | `display-dj dark` / `light` |
| Platform `#[cfg]` blocks | Zero — one binary per platform |

---

## Integrating display-dj Into Other Apps

There are two integration approaches:

1. **HTTP server** (recommended) — start `display-dj serve` as a background process, then call it via HTTP from any language. The server keeps gamma alive on macOS.
2. **CLI subprocess** — invoke `display-dj` commands directly. Simpler for one-shot operations, but gamma resets when the process exits on macOS.

### Option 1: HTTP Server (recommended for apps)

Start the server once (as a background process or sidecar), then make HTTP requests:

```bash
# Start in background
display-dj serve &

# Or on a custom port
display-dj serve 8080 &
```

Default port: `51337`. Binds to `127.0.0.1` only (localhost, not exposed to network). All responses include `Access-Control-Allow-Origin: *` for browser/Electron/Tauri CORS.

#### HTTP Routes

All routes are `GET` with path-based parameters. No query strings, no POST bodies, no headers required.

| Route | Description | Response |
|-------|-------------|----------|
| `/list` | List all displays | `[{id, name, display_type, brightness, contrast, ddc_supported}]` |
| `/get_all` | Get live brightness for all | Same as list with fresh readings |
| `/get_one/<id>` | Get one display | `{id, name, ...}` |
| `/set_all/<level>` | Set all to 0-100 (mode: force) | `[{id, name, status}]` |
| `/set_all/<level>/<mode>` | Set all with explicit mode | `[{id, name, status}]` |
| `/set_one/<id>/<level>` | Set one display (mode: force) | `{id, name, status}` |
| `/set_one/<id>/<level>/<mode>` | Set one with explicit mode | `{id, name, status}` |
| `/dark` | Switch to dark mode | `{status}` |
| `/light` | Switch to light mode | `{status}` |
| `/theme` | Get current theme | `{theme}` |
| `/reset` | Reset gamma to defaults | `{status}` |
| `/health` | Health check | `{status: "ok", pid, uptime}` |
| `/set_contrast_all/<level>` | Set contrast on all (DDC only) | `[{id, name, status}]` |
| `/set_contrast_one/<id>/<level>` | Set contrast on one (DDC only) | `{id, name, status}` |
| `/debug` | Full diagnostics with active tests | `{displays, scale, platform, tests}` |

`<id>` accepts numeric IDs (`0`, `1`, `2`), `builtin`, or monitor names (URL-encoded).

#### Node.js / Tauri (HTTP)

```typescript
const BASE = 'http://127.0.0.1:51337';

// List displays
const displays = await fetch(`${BASE}/list`).then(r => r.json());

// Set brightness
await fetch(`${BASE}/set_all/50`);
await fetch(`${BASE}/set_one/2/80`);
await fetch(`${BASE}/set_one/builtin/60`);

// Get brightness for one display
const info = await fetch(`${BASE}/get_one/builtin`).then(r => r.json());
console.log(info.brightness); // 60

// Dark mode
await fetch(`${BASE}/dark`);
const theme = await fetch(`${BASE}/theme`).then(r => r.json());
console.log(theme.theme); // "dark"

// Health check (useful for checking if server is running)
const ok = await fetch(`${BASE}/health`).then(r => r.ok).catch(() => false);
```

#### Tauri sidecar setup

To bundle `display-dj` as a Tauri sidecar that starts automatically:

```typescript
// Start server as sidecar on app launch
import { Command } from '@tauri-apps/plugin-shell';

const server = Command.sidecar('display-dj', ['serve']).spawn();

// Then use fetch() for all operations
const displays = await fetch('http://127.0.0.1:51337/list').then(r => r.json());
```

#### Python (HTTP)

```python
import requests

BASE = 'http://127.0.0.1:51337'

displays = requests.get(f'{BASE}/list').json()
requests.get(f'{BASE}/set_all/50')
requests.get(f'{BASE}/set_one/2/80')
theme = requests.get(f'{BASE}/theme').json()
```

#### Go (HTTP)

```go
resp, _ := http.Get("http://127.0.0.1:51337/list")
defer resp.Body.Close()
var displays []map[string]interface{}
json.NewDecoder(resp.Body).Decode(&displays)

http.Get("http://127.0.0.1:51337/set_all/50")
```

#### Why HTTP over CLI subprocess?

| | HTTP server | CLI subprocess |
|---|---|---|
| Gamma persistence (macOS) | Server stays alive, gamma holds | Process exits, gamma resets |
| Latency | ~1ms per request | ~50-200ms (process spawn overhead) |
| Concurrency | Handles multiple callers | One command at a time |
| Complexity | Start once, fetch forever | Spawn per command |
| Startup cost | One-time on launch | Per invocation |

### Option 2: CLI Subprocess (simpler for scripts)

For one-shot commands or DDC-only mode (where gamma persistence doesn't matter):

#### Shell / Scripts

```bash
displays=$(display-dj list)
display-dj get_all | jq '.[].brightness'
display-dj set_one 1 50 && echo "done"
```

#### Node.js / Tauri (subprocess)

```typescript
import { Command } from '@tauri-apps/plugin-shell';

// List displays
const output = await Command.create('display-dj', ['list']).execute();
const displays = JSON.parse(output.stdout);

// Set brightness
await Command.create('display-dj', ['set_one', '1', '50']).execute();

// Set all displays
await Command.create('display-dj', ['set_all', '80']).execute();

// Get brightness for one display
const info = JSON.parse(
  (await Command.create('display-dj', ['get_one', 'builtin']).execute()).stdout
);
console.log(info.brightness); // 80

// Dark mode
await Command.create('display-dj', ['dark']).execute();

// Get theme
const theme = JSON.parse(
  (await Command.create('display-dj', ['theme']).execute()).stdout
);
console.log(theme.theme); // "dark" or "light"
```

### Node.js (plain child_process)

```javascript
const { execSync } = require('child_process');

const displays = JSON.parse(execSync('display-dj list').toString());
execSync('display-dj set_all 50');
```

### Python

```python
import subprocess, json

displays = json.loads(subprocess.check_output(["display-dj", "list"]))
subprocess.run(["display-dj", "set_one", "1", "80"])
```

### Go

```go
out, _ := exec.Command("display-dj", "list").Output()
var displays []map[string]interface{}
json.Unmarshal(out, &displays)
```

### Alternative Node.js integration approaches

| Approach | How | Best for |
|----------|-----|----------|
| **child_process** | Shell out to `display-dj` binary | Simplest, recommended |
| **ffi-napi / koffi** | Call C functions directly (like Rust's `extern "C"`) | DisplayServices, CoreGraphics, Win32 |
| **node-gyp / N-API** | Write a C++ addon | Maximum performance, direct API access |
| **edge.js** | Call .NET from Node | Windows WMI specifically |

For most apps, shelling out to the binary is the right choice. It's simple, the JSON interface is stable, and the binary is only ~556KB.

---

## Troubleshooting & Platform Gotchas

### Windows: duplicate built-in display

**Problem:** On laptops, the built-in panel appears in both WMI (`WmiMonitorBrightness`) and the DDC enumeration (`ddc_winapi::Monitor::enumerate()`). Without handling this, `list` shows two entries for the same display — one as `"builtin"` and one as an external `"Generic PnP Monitor"`.

**Root cause:** `ddc_winapi::Monitor::enumerate()` uses `EnumDisplayMonitors` internally, which returns all HMONITOR handles including the primary (built-in) monitor. The DDC APIs then try to open a physical monitor handle for each, including the laptop panel — even though the panel doesn't speak DDC/CI.

**Fix:** In `windows.rs` `WinPlatform::enumerate()`, after adding the WMI-backed builtin, the DDC loop checks each monitor's `is_primary` flag (from `GetMonitorInfoW` → `MONITORINFOF_PRIMARY`). If a WMI builtin was already added **and** the DDC monitor maps to the primary HMONITOR, it's skipped.

**How to verify:** Run `display-dj debug` and check:
- `platform.wmi_brightness` — non-null means WMI detected a builtin
- `platform.hmonitors[N].is_primary` — identifies which HMONITOR is the laptop panel
- `platform.ddc_monitors[N].vcp_brightness` — typically `null` for the laptop panel's DDC entry
- `displays` array — should have exactly one `"builtin"` entry, no duplicate

**Example debug output (healthy laptop with one external):**
```json
{
  "displays": [
    {"id": "builtin", "display_type": "builtin", "ddc_supported": false},
    {"id": "1", "display_type": "external", "name": "Generic PnP Monitor (APT1222)"}
  ],
  "platform": {
    "wmi_brightness": 72,
    "ddc_monitor_count": 2,
    "hmonitors": [
      {"index": 0, "is_primary": true, "monitor_device_id": "MONITOR\\SDC416B\\..."},
      {"index": 1, "is_primary": false, "monitor_device_id": "MONITOR\\APT1222\\..."}
    ]
  }
}
```

Here `ddc_monitor_count` is 2 (DDC sees both monitors), but the final `displays` has only 2 entries because the primary DDC entry was deduped against the WMI builtin.

### Windows: PnP device ID enrichment

Multiple monitors may report the same generic description ("Generic PnP Monitor"). To distinguish them, `get_hmonitor_details()` calls `EnumDisplayDevicesW` to get each monitor's PnP device ID (e.g., `DEL40F4` for Dell, `APT1222` for a specific model). This is appended to the name: `"Generic PnP Monitor (DEL40F4)"`.

### DDC/CI quirks

- **Checksum errors on reads:** Some monitors (e.g., Acer XZ322QU V3) return DDC/CI checksum errors. The macOS module retries reads up to 3 times and writes up to 5 times with 50ms delays. The `force` mode stacks DDC + gamma as a reliable fallback.
- **Brightness 0 = standby:** Setting DDC brightness to 0 can cause monitors to enter standby or freeze. All platforms clamp the minimum DDC value to 1.
- **VCP null ≠ broken:** `vcp_brightness: null` in debug output means the DDC read failed — common for laptop panels (which use backlight APIs, not DDC) and monitors with flaky I2C buses. Gamma fallback still works.

### Gamma behavior differences

| Platform | Gamma persistence | Reset mechanism |
|----------|-------------------|-----------------|
| macOS (CoreGraphics) | Resets when process exits | `CGDisplayRestoreColorSyncSettings()` |
| Windows (GDI32) | Resets when process exits | `SetDeviceGammaRamp` with identity ramp |
| Linux X11 (xrandr) | **Persists** after exit | `xrandr --output <name> --brightness 1.0` |
| Linux Wayland (wlr-randr) | **Persists** after exit | `wlr-randr --output <name> --brightness 1.0` |

This is why macOS and Windows use `maybe_keep_alive()` — the process must stay alive to hold the gamma state. On Linux, gamma changes stick and the process can exit.

### Interpreting `debug` output

The `display-dj debug` command runs active tests and dumps raw platform data. Key sections:

| Section | What it contains |
|---------|-----------------|
| `displays` | Final enumerated displays (after dedup) with initial brightness/contrast reads |
| `platform` | Raw platform data: HMONITOR details (Windows), CoreGraphics displays (macOS), sysfs backlight + ddcutil output (Linux) |
| `tests.displays` | Per-display results: sets brightness to 25% via each mode (DDC, gamma, force), reads back, restores original |
| `tests.volume` | Volume get/set/mute/unmute cycle with restore |
| `tests.theme` | Dark/light toggle cycle with restore |
| `scale` | Current per-display scale factors |

**All tests are non-destructive** — they save initial state, run tests, and restore. If a test field is `null` or `false`, that feature is unavailable on that display/platform.

---

## Rust Syntax Guide for Node/JS Developers

This guide explains the Rust patterns used in this codebase using Node.js equivalents.

### Conditional Compilation (`#[cfg(...)]`)

```rust
#[cfg(target_os = "macos")]
mod macos;
```

`#[cfg(...)]` is a **compile-time conditional** -- code is included or excluded at build time. There's no direct Node equivalent since Node doesn't have compile-time elimination. The closest analogy:

```js
if (process.platform === 'darwin') require('./macos');
```

`mod macos;` imports the file `src/macos.rs` (or `src/macos/mod.rs`). It's like `const macos = require('./macos')`, but it also **declares** the module as part of this crate.

### Imports (`use`)

```rust
use serde::Serialize;
use std::thread;
use std::time::Duration;
```

```js
const { Serialize } = require('serde');  // external package
const { setTimeout } = require('timers');
```

`use` pulls names into scope. `std::` is the standard library (like Node built-ins). `serde::` is an external crate (like an npm package -- declared in `Cargo.toml`, which is Rust's `package.json`).

### Visibility (`pub`)

```rust
pub const VCP_BRIGHTNESS: u8 = 0x10;
pub const VCP_CONTRAST: u8 = 0x12;
pub const BUILTIN_ID: &str = "builtin";
```

```js
// In Node, everything you module.exports is public
export const VCP_BRIGHTNESS = 0x10;
export const VCP_CONTRAST = 0x12;
export const BUILTIN_ID = "builtin";
```

`pub` makes items visible outside the current module. Without `pub`, items are private by default -- the opposite of JS/Node where everything is accessible unless you don't export it.

`&str` on a constant means it's a **string slice** pointing to data baked into the binary -- like a string literal in JS, but the type explicitly says "borrowed, read-only."

### Derive Macros (`#[derive(...)]`)

```rust
#[derive(Serialize, Clone)]
pub struct DisplayInfo {
    pub id: String,
    pub name: String,
    pub display_type: String,
    pub brightness: Option<u32>,
    pub contrast: Option<u32>,
    pub ddc_supported: bool,
}
```

`#[derive(Serialize, Clone)]` auto-generates trait implementations at compile time:

- `Serialize` (from serde) -- makes the struct JSON-serializable. Like having `JSON.stringify()` just work on your object, but you have to opt in.
- `Clone` -- lets you copy the struct with `.clone()`. In JS, all objects can be spread/copied freely; in Rust, you must explicitly opt in.

Each `pub` field is individually exported. In JS, all object properties are always accessible -- Rust lets you make some fields private even on a public struct.

The TypeScript equivalent:

```ts
interface DisplayInfo {
    id: string;
    name: string;
    displayType: string;
    brightness: number | null;
    contrast: number | null;
    ddcSupported: boolean;
}
```

### Traits (Interfaces)

```rust
pub trait DisplayControl {
    fn get_brightness(&mut self) -> Option<u32>;
    fn get_contrast(&mut self) -> Option<u32>;
    fn set_brightness(&mut self, value: u16, mode: &str) -> bool;
    fn set_contrast(&mut self, value: u16) -> bool;
    fn reset_gamma(&self);
}

pub trait Platform {
    fn enumerate() -> Vec<(DisplayInfo, Box<dyn DisplayControl>)>;
    fn reset_all_gamma();
    fn debug_info() -> serde_json::Value;
}
```

A `trait` is like a TypeScript `interface`, but for implementations (not just shape):

```ts
interface DisplayControl {
    getBrightness(): number | null;
    getContrast(): number | null;
    setBrightness(value: number, mode: string): boolean;
    setContrast(value: number): boolean;
    resetGamma(): void;
}

interface Platform {
    enumerate(): [DisplayInfo, DisplayControl][];
    resetAllGamma(): void;
    debugInfo(): object;
}
```

Key types:

- `Vec<T>` -- like `Array<T>` in JS
- `(A, B)` -- a **tuple**, like a fixed-length typed array `[DisplayInfo, DisplayControl]`
- `Box<dyn DisplayControl>` -- a heap-allocated pointer to *any* object implementing `DisplayControl`. `dyn` = "I don't know the concrete type at compile time" (dynamic dispatch/polymorphism). `Box` = "it's on the heap" (like all JS objects).

No `self` parameter on `Platform` methods means they're **static methods** -- called on the type, not an instance (like `Array.isArray()` not `arr.push()`).

### `&self` and `&mut self`

- `&mut self` -- "I need a mutable reference to myself." This method can modify the object's fields. Like a normal JS method that does `this.x = ...`.
- `&self` -- "I only need a read-only reference." Won't modify anything. JS has no equivalent; all methods can mutate `this`.

### `eprintln!` vs `println!`

```rust
eprintln!("Setting all displays to {}%", level);  // stderr
println!("{}", serde_json::to_string_pretty(&results).unwrap());  // stdout
```

```js
console.error(`Setting all displays to ${level}%`);  // stderr
console.log(JSON.stringify(results, null, 2));        // stdout
```

`println!` writes to stdout, `eprintln!` writes to stderr. The `!` suffix means it's a **macro**, not a regular function. `{}` is the format placeholder like `${}` in template literals.

This codebase uses `eprintln!` for human-readable status messages and `println!` for machine-readable JSON output -- a common CLI pattern.

### `match` Expression (Switch Statement)

```rust
match cmd {
    "set_all" => {
        // ...
    }
    "set_one" => {
        // ...
    }
    "list" => cmd_list::<P>(),
    "reset" => {
        P::reset_all_gamma();
        eprintln!("Gamma reset to defaults.");
    }
    _ => {
        usage();
        if cmd != "help" && cmd != "--help" && cmd != "-h" {
            std::process::exit(1);
        }
    }
}
```

```js
switch (cmd) {
    case "set_all":
        // ...
        break;
    case "set_one":
        // ...
        break;
    case "list":
        cmdList(displays);
        break;
    case "reset":
        Platform.resetAllGamma();
        console.error("Gamma reset to defaults.");
        break;
    default:
        usage();
        if (cmd !== "help" && cmd !== "--help" && cmd !== "-h") {
            process.exit(1);
        }
}
```

`match` is like `switch` but more powerful:

- No `break` needed -- only the matched arm runs (no fall-through).
- `_` is the **wildcard** pattern, equivalent to `default`.
- `match` is an **expression** -- it can return a value (not used here, but it can).
- The compiler checks that all cases are covered (**exhaustiveness checking**). The `_` arm is required here because there are infinite possible strings.

### `unwrap_or_else` with Exit

```rust
let level: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
    usage();
    std::process::exit(1);
});
```

```js
const level = parseInt(args[3]);
if (isNaN(level)) {
    usage();
    process.exit(1);
}
```

Breaking down the chain:

1. `args.get(2)` -- returns `Option<&String>` (safe indexing -- `None` if out of bounds, unlike JS which gives `undefined`)
2. `.and_then(|s| s.parse().ok())` -- try to parse as `u16`; if it fails, return `None`. `|s|` is a **closure** (arrow function).
3. `.unwrap_or_else(|| { ... })` -- if `None`, run this closure. Unlike `.unwrap_or(default)` which takes a value, `_else` takes a function -- useful when the fallback has side effects (like exiting).

### Slices (`&[String]`)

```rust
fn dispatch<P: Platform>(cmd: &str, args: &[String]) {
```

```ts
function dispatch<P extends Platform>(cmd: string, args: string[]) {
```

`&[String]` is a **slice** -- a borrowed view into a contiguous sequence. It's like passing an array by reference in JS. The function can read the array but doesn't own it and can't resize it. You can pass a `&Vec<String>` or a portion of one as a `&[String]`.

### Generic Functions

```rust
fn dispatch<P: Platform>(cmd: &str, args: &[String]) {
```

```ts
function dispatch<P extends Platform>(cmd: string, args: string[]) {
```

`P: Platform` means "P must implement the Platform trait" -- identical to `P extends Platform` in TypeScript. The compiler generates a specialized copy of this function for each concrete type -- this is called **monomorphization** and has zero runtime cost.

Calling with an explicit type parameter:

```rust
dispatch::<macos::MacPlatform>(cmd, &args);
```

```ts
dispatch<MacPlatform>(cmd, args);
```

### `Option<T>` as a Function Parameter

```rust
fn cmd_get<P: Platform>(filter_id: Option<&String>) {
```

```ts
function cmdGet<P extends Platform>(filterId: string | null) {
```

`Option<&String>` means "maybe a reference to a String, maybe nothing." Called with `Some(id)` or `None`:

```rust
cmd_get::<P>(None);        // no filter
cmd_get::<P>(Some(id));    // filter by id
```

```js
cmdGet(null);              // no filter
cmdGet(id);                // filter by id
```

### Doc Comments (`///`)

```rust
/// Match a display by ID or name (case-insensitive). ID takes priority.
fn matches_display(info: &DisplayInfo, query: &str) -> bool {
    info.id == query || info.name.to_lowercase() == query.to_lowercase()
}
```

```js
/**
 * Match a display by ID or name (case-insensitive). ID takes priority.
 */
function matchesDisplay(info, query) {
    return info.id === query || info.name.toLowerCase() === query.toLowerCase();
}
```

`///` is a **doc comment** -- like JSDoc's `/** */`. It generates documentation via `cargo doc` (like JSDoc or TypeDoc). Regular comments use `//`.

`&DisplayInfo` means "a borrowed reference to a DisplayInfo" -- the function reads the struct without taking ownership. In JS, all objects are passed by reference automatically; in Rust, you must choose between passing ownership (move) or borrowing (`&`).

### Pattern Matching on Option (`if let`)

```rust
if let Some(id) = filter_id {
    if !matches_display(&info, id) {
        continue;
    }
}
```

```js
if (filterId !== null) {
    if (!matchesDisplay(info, filterId)) {
        continue;
    }
}
```

`if let Some(id) = ...` checks if an `Option` has a value **and** extracts it into `id` in one step. This is pattern matching -- one of Rust's most powerful features.

`&info` passes a reference to `info` -- the function borrows it for the duration of the call. In JS, objects are always passed by reference so there's no equivalent syntax.

`continue` skips to the next loop iteration, same as in JS.

### Rebinding as Mutable

```rust
let mut info = info;
info.brightness = ctrl.get_brightness();
```

```js
// In JS, you'd just mutate directly:
info.brightness = ctrl.getBrightness();
```

In Rust, variables are immutable by default. `let mut info = info;` creates a new mutable binding to the same data -- now you can modify its fields. This is a **move**, not a copy (the original `info` is gone).

### Iterator Chains with Map and Collect

```rust
let infos: Vec<DisplayInfo> = displays.into_iter().map(|(info, _)| info).collect();
```

```js
const infos = displays.map(([info, _]) => info);
```

- `.into_iter()` consumes the vector (moves ownership). After this, `displays` is gone.
- `.map(|(info, _)| info)` -- destructures each tuple, keeps only the first element. `_` is the **discard pattern** -- "I don't care about this value." Like an unused variable in JS destructuring.
- `.collect()` gathers the iterator back into a concrete collection (`Vec<DisplayInfo>`). Rust iterators are **lazy** -- nothing happens until you collect. JS `.map()` is eager (runs immediately).

### Serialization with Serde

```rust
println!("{}", serde_json::to_string_pretty(&results).unwrap());
```

```js
console.log(JSON.stringify(results, null, 2));
```

`serde_json::to_string_pretty` converts any `Serialize`-able struct to formatted JSON. `.unwrap()` panics if serialization fails (it won't for simple structs -- this is like omitting a try/catch when you know it can't throw).

`&results` passes a reference -- serde reads the data without taking ownership, so you could still use `results` afterwards.

### `&str` vs `String`

- `String` -- owned, heap-allocated, growable. Like a JS string you can reassign.
- `&str` -- a borrowed, read-only view into string data. Passed to avoid copying.

In Node, all strings are immutable and reference-counted, so there's no equivalent concern.

### `impl Trait for Struct` (Implementing an Interface)

```rust
struct BuiltinControl {
    display_id: u32,
    ds: DisplayServicesFns,
}

impl DisplayControl for BuiltinControl {
    fn get_brightness(&mut self) -> Option<u32> {
        let mut val: f32 = 0.0;
        let res = unsafe { (self.ds.get)(self.display_id, &mut val) };
        if res == 0 { Some((val * 100.0).round() as u32) } else { None }
    }

    fn set_brightness(&mut self, value: u16, _mode: &str) -> bool {
        // ...
    }
    // ... other trait methods
}
```

```js
class BuiltinControl {
    constructor(displayId, ds) {
        this.displayId = displayId;
        this.ds = ds;
    }

    getBrightness() { /* ... */ }
    setBrightness(value, mode) { /* ... */ }
}
```

`impl DisplayControl for BuiltinControl` says "BuiltinControl fulfills the DisplayControl interface." In JS, you just add the methods to a class and hope the caller passes the right object. In Rust, the compiler verifies the struct has *all* required methods with the correct signatures.

`_mode` -- the underscore prefix means "I accept this parameter but don't use it." Silences the unused variable warning. Same convention as `_` in JS destructuring.

### Enums (Tagged Unions)

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum DisplayServer {
    X11,
    Wayland,
    Unknown,
}
```

```ts
type DisplayServer = "x11" | "wayland" | "unknown";
// or
enum DisplayServer { X11, Wayland, Unknown }
```

Rust enums are much more powerful than TS enums -- they can carry data (like `Option<T>` which is `enum Option { Some(T), None }`). Here they're used as simple tags like a TS string union.

`#[derive(Debug, Clone, Copy, PartialEq)]`:
- `Debug` -- allows printing with `{:?}` (like `console.log` for debugging)
- `Clone` + `Copy` -- makes the value copyable like a primitive. Without this, assignment moves ownership.
- `PartialEq` -- enables `==` comparison. Unlike JS, Rust doesn't give you `==` for free.

### `.into()` Conversion

```rust
let info = DisplayInfo {
    id: BUILTIN_ID.into(),
    name: "Built-in Display".into(),
    display_type: "builtin".into(),
    brightness,
    // ...
};
```

```js
const info = {
    id: BUILTIN_ID,
    name: "Built-in Display",
    displayType: "builtin",
    brightness,
};
```

`.into()` converts between types -- here it converts `&str` (string literal) into `String` (owned string). In JS, strings are all one type so this isn't needed. Notice `brightness` without a colon -- Rust has **field init shorthand** just like JS ES6 (`{ brightness }` instead of `{ brightness: brightness }`).

### `Result<T, E>` and Error Handling

```rust
if let Ok(monitors) = ddc_macos::Monitor::enumerate() {
    for (idx, mut mon) in monitors.into_iter().enumerate() {
        // ...
    }
}
```

```js
try {
    const monitors = ddcMacos.Monitor.enumerate();
    monitors.forEach((mon, idx) => { /* ... */ });
} catch (e) {
    // silently skip
}
```

`Result<T, E>` is Rust's version of try/catch, but as a return value instead of an exception. It's either `Ok(value)` or `Err(error)`. `if let Ok(x) = ...` unwraps only the success case -- if enumeration fails, the block is silently skipped.

Other common patterns:
- `.ok()` -- converts `Result` to `Option`, discarding the error
- `.is_ok()` / `.is_err()` -- check without unwrapping
- `.unwrap()` -- get the value or panic (crash) if error

### Match Guards

```rust
let output = match Command::new("ddcutil").args(["detect", "--brief"]).output() {
    Ok(o) if o.status.success() => o,
    _ => return vec![],
};
```

```js
let output;
try {
    output = execSync("ddcutil detect --brief");
} catch {
    return [];
}
if (!output.status.success) return [];
```

`Ok(o) if o.status.success()` is a **match guard** -- it matches `Ok` *and* checks a condition. If the condition fails, it falls through to `_`. This combines pattern matching with an extra boolean check.

`vec![]` is a macro that creates an empty `Vec` -- like `[]` in JS.

### `format!()` Macro

```rust
let name = format!("External Display {}", idx + 1);
let cmd = format!("(Get-CimInstance ...).WmiSetBrightness(1, {})", value);
```

```js
const name = `External Display ${idx + 1}`;
const cmd = `(Get-CimInstance ...).WmiSetBrightness(1, ${value})`;
```

`format!()` is like JS template literals but returns a `String`. Used when you need to build a string but aren't printing it. `println!`/`eprintln!` print directly; `format!` just returns the string.

### `unsafe` Blocks and FFI

```rust
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGSetDisplayTransferByFormula(display: u32, /* ... */) -> i32;
    fn CGDisplayRestoreColorSyncSettings();
}

// Using it:
unsafe {
    CGSetDisplayTransferByFormula(display_id, 0.0, val, 1.0, /* ... */);
}
```

```js
// Node equivalent would be using node-ffi or N-API:
const ffi = require('ffi-napi');
const CoreGraphics = ffi.Library('CoreGraphics', {
    CGSetDisplayTransferByFormula: ['int', ['uint32', /* ... */]],
});
CoreGraphics.CGSetDisplayTransferByFormula(displayId, 0.0, val, 1.0);
```

`extern "C"` declares functions from a C library (here macOS frameworks). `unsafe` means "I'm doing something the compiler can't verify is safe" -- calling C code, raw pointer access, etc. The compiler trusts you inside `unsafe` blocks. This is needed for OS-level APIs that don't exist in Rust.

### `if let Some(ref x)` (Borrowing Inside a Pattern)

```rust
if let Some(ref output) = self.output_name {
    set_gamma(output, value as u32, self.display_server);
}
```

```js
if (this.outputName !== null) {
    setGamma(this.outputName, value, this.displayServer);
}
```

`ref` borrows the value instead of moving it out of `self`. Without `ref`, the string would be moved out of the struct and you couldn't use `self.output_name` again. In JS, all values are references so this isn't a concern.

### Iterator Methods: `filter_map`, `skip`, `nth`

```rust
String::from_utf8_lossy(&output.stdout)
    .lines().skip(1)
    .filter_map(|line| {
        let name = line.split_whitespace().last()?;
        if name.starts_with("eDP") { None }
        else { Some(name.to_string()) }
    })
    .collect()
```

```js
output.stdout.toString()
    .split('\n').slice(1)
    .map(line => {
        const name = line.trim().split(/\s+/).pop();
        if (!name || name.startsWith("eDP")) return null;
        return name;
    })
    .filter(Boolean)
```

- `.lines()` -- splits string by newline (like `.split('\n')`)
- `.skip(1)` -- skips the first element (like `.slice(1)`)
- `.filter_map()` -- combines `.map()` + `.filter()`. Returns `Some(x)` to keep, `None` to skip. The `?` operator inside returns `None` early if the value is missing.
- `.nth(n)` -- gets the nth element from an iterator (like `arr[n]` but for lazy iterators)

### `strip_prefix` and String Methods

```rust
if let Some(rest) = line.strip_prefix("Display ") {
    current_num = rest.trim().parse().ok();
}
```

```js
if (line.startsWith("Display ")) {
    const rest = line.slice("Display ".length);
    currentNum = parseInt(rest.trim()) || null;
}
```

`.strip_prefix()` returns `Option<&str>` -- the remaining string after the prefix, or `None` if the prefix doesn't match. Combined with `if let`, it checks and extracts in one step. JS doesn't have an equivalent (you need `startsWith` + `slice`).

### Infinite Loop (Conditional Keep-Alive)

```rust
fn maybe_keep_alive(mode: &str) {
    if mode == "force" || mode == "gamma" {
        eprintln!("\nPress Ctrl+C to exit (gamma will reset).");
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }
}
```

```js
function maybeKeepAlive(mode) {
    if (mode === "force" || mode === "gamma") {
        console.error("\nPress Ctrl+C to exit (gamma will reset).");
        setInterval(() => {}, 60_000);
    }
}
```

`loop` is Rust's infinite loop. `thread::sleep` blocks the current thread -- unlike JS which is single-threaded and uses `setTimeout`/`setInterval`. This function **never returns** when the condition is true (the loop runs forever).

### Quick Reference

| Rust | Node/JS |
|---|---|
| `Option<T>` | `T \| null` |
| `Result<T, E>` | `try/catch` (but as a return value) |
| `Vec<T>` | `Array<T>` |
| `&[T]` | `readonly T[]` (borrowed array view) |
| `&str` / `String` | `string` (one type) |
| `trait` | `interface` (TS) |
| `impl Trait for Struct` | `class Struct implements Interface` |
| `enum` | Tagged union / string literal union (TS) |
| `match` | `switch` (no fall-through, exhaustive) |
| `#[cfg(...)]` | `process.platform` check (but at compile time) |
| `#[derive(...)]` | Auto-generated implementations (no JS equivalent) |
| `pub` | `export` |
| `Box<dyn Trait>` | Just pass any object that fits the interface |
| `mut` | Default in JS -- everything is mutable |
| `unsafe` | No equivalent -- for raw pointers, FFI, OS APIs |
| `extern "C"` | `ffi-napi` / N-API bindings |
| `println!` / `eprintln!` | `console.log` / `console.error` |
| `format!("{}",x)` | `` `${x}` `` (template literal) |
| `vec![]` | `[]` (empty array literal) |
| `serde_json` | `JSON.stringify` / `JSON.parse` |
| `.into()` | No equivalent -- auto type conversion |
| `x.ok()` | Converts error to null |
| `.is_ok()` / `.is_err()` | No equivalent -- check without unwrapping |
| `///` | `/** */` (JSDoc) |
| `&T` (reference param) | Objects are always by-reference in JS |
| ownership / borrowing | GC handles it -- you never think about it |
| `ref` in patterns | No equivalent -- borrow instead of move |
| `_` (wildcard) | Unused variable in destructuring |
| `_param` (unused param) | Accepted but intentionally unused |
| `|x| expr` | `(x) => expr` |
| `.unwrap_or(default)` | `?? default` |
| `.unwrap_or_else(\|\| { })` | `?? (() => { })()` (but lazily) |
| `.filter_map()` | `.map().filter(Boolean)` |
| `.collect()` | No equivalent -- JS iterators are eager |
| `.strip_prefix()` | `startsWith()` + `slice()` |
| `as u32` | No equivalent -- JS has one `number` type |
