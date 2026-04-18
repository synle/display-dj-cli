// Platform modules — only one is compiled per OS (like build-time if/else).
// `mod` both declares and imports the file (e.g., mod macos => src/macos.rs).
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

use serde::{Serialize, Deserialize};
use std::thread;
use std::time::Duration;

// VCP (Virtual Control Panel) codes — standard DDC/CI register addresses.
// These are the same across all monitor brands.
pub const VCP_BRIGHTNESS: u8 = 0x10;
pub const VCP_CONTRAST: u8 = 0x12;
pub const BUILTIN_ID: &str = "builtin"; // &str = borrowed string literal baked into the binary

// =========================================================================
// Shared types — used by all platform modules
// =========================================================================

// #[derive(Serialize, Clone)] auto-generates JSON serialization and .clone()
// support at compile time. Without Serialize, serde_json can't convert this
// to JSON. Without Clone, we can't copy it (Rust moves by default).
#[derive(Serialize, Deserialize, Clone)]
pub struct DisplayInfo {
    pub id: String,            // "builtin", "1", "2", ...
    pub name: String,          // human-readable name from the monitor's EDID
    pub display_type: String,  // "builtin" or "external"
    pub brightness: Option<u32>, // Option = nullable — Some(75) or None
    pub contrast: Option<u32>,
    pub ddc_supported: bool,
}

// Trait = interface. Each platform module implements this for its display types.
// &mut self = mutable reference to the object (like `this` but must opt in to mutation).
// &self = read-only reference.
pub trait DisplayControl {
    fn get_brightness(&mut self) -> Option<u32>;
    fn get_contrast(&mut self) -> Option<u32>;
    fn set_brightness(&mut self, value: u16, mode: &str) -> bool;
    fn set_contrast(&mut self, value: u16) -> bool;
    fn reset_gamma(&self);
}

// No &self — these are static methods called on the type itself (Platform::enumerate()).
// Box<dyn DisplayControl> = heap-allocated trait object — lets us store different
// concrete types (BuiltinControl, ExternalControl) in the same Vec.
pub trait Platform {
    fn enumerate() -> Vec<(DisplayInfo, Box<dyn DisplayControl>)>;
    fn reset_all_gamma();
    fn debug_info() -> serde_json::Value;
}

// =========================================================================
// CLI — all human-readable output goes to stderr, JSON goes to stdout
// =========================================================================

/// Print CLI usage/help text to stderr.
/// Lists all commands, modes, and platform-specific dependency info.
fn usage() {
    eprintln!("display-dj v{} — cross-platform display brightness control\n", env!("CARGO_PKG_VERSION"));
    eprintln!("Usage:");
    eprintln!("  display-dj set_all <level> [mode]       Set all displays (0-100)");
    eprintln!("  display-dj set_one <id> <level> [mode]   Set one display");
    eprintln!("  display-dj get_all                      Get brightness for all (JSON)");
    eprintln!("  display-dj get_one <id>                 Get brightness for one (JSON)");
    eprintln!("  display-dj list                         List all displays (JSON)");
    eprintln!("  display-dj reset                        Reset gamma to defaults");
    eprintln!("  display-dj dark                         Switch to dark mode");
    eprintln!("  display-dj light                        Switch to light mode");
    eprintln!("  display-dj theme                        Get current theme (JSON)");
    eprintln!("  display-dj get_volume                   Get volume (JSON)");
    eprintln!("  display-dj set_volume <level>           Set volume (0-100)");
    eprintln!("  display-dj mute                         Mute audio");
    eprintln!("  display-dj unmute                       Unmute audio");
    eprintln!("  display-dj set_contrast_all <level>      Set contrast on all displays (0-100, DDC only)");
    eprintln!("  display-dj set_contrast_one <id> <level> Set contrast on one display (0-100, DDC only)");
    eprintln!("  display-dj get_scale                    Get display scaling (JSON)");
    eprintln!("  display-dj set_scale_all <percent>       Set all displays scaling (75-300)");
    eprintln!("  display-dj set_scale_one <id> <percent>  Set one display scaling (75-300)");
    eprintln!("  display-dj keep_awake_on                Prevent system sleep (blocks until Ctrl+C)");
    eprintln!("  display-dj keep_awake_off               Stop preventing system sleep");
    eprintln!("  display-dj get_keep_awake               Get keep-awake status (JSON)");
    eprintln!("  display-dj set_wallpaper <fit> <path>   Set wallpaper on all monitors");
    eprintln!("  display-dj set_wallpaper_one <index> <fit> <path>  Set wallpaper on one monitor (0-based)");
    eprintln!("  display-dj get_wallpaper                Get current wallpaper (JSON)");
    eprintln!("  display-dj get_wallpaper_supported      Check wallpaper support (JSON)");
    eprintln!("  display-dj debug                        Dump debug info for all displays (JSON)");
    eprintln!("  display-dj serve [port]                 Start HTTP server (default: 51337)");
    eprintln!();
    eprintln!("Modes: force (default), auto, ddc, gamma");
    eprintln!("Display IDs: \"builtin\" or \"0\", \"1\", \"2\", ... (see `display-dj list`)");
    eprintln!("OS: {}", std::env::consts::OS);
    eprintln!();

    #[cfg(target_os = "macos")]
    eprintln!("Dependencies: none (all native)");

    #[cfg(target_os = "windows")]
    eprintln!("Dependencies: none (all native)");

    #[cfg(target_os = "linux")]
    {
        eprintln!("Linux dependencies:");
        eprintln!("  Ubuntu/Debian:  sudo apt install ddcutil i2c-tools brightnessctl x11-xserver-utils wlr-randr");
        eprintln!("  Fedora/RHEL:    sudo dnf install ddcutil i2c-tools brightnessctl xrandr wlr-randr");
        eprintln!("  Arch/Manjaro:   sudo pacman -S ddcutil i2c-tools brightnessctl xorg-xrandr wlr-randr");
        eprintln!("  Then run:       sudo modprobe i2c-dev && sudo usermod -aG i2c $USER");
    }
}

fn main() {
    // Collect CLI args into a Vec (like process.argv in Node)
    let args: Vec<String> = std::env::args().collect();

    // Safe indexing: .get(1) returns Option instead of panicking on out-of-bounds.
    // .map() transforms the inner value, .unwrap_or() provides a default.
    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    if cmd == "--version" || cmd == "-V" {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }

    // Compile-time platform dispatch — only one of these lines exists in the binary.
    // The ::<Type> syntax (turbofish) passes the platform type to the generic function.
    #[cfg(target_os = "macos")]
    dispatch::<macos::MacPlatform>(cmd, &args);

    #[cfg(target_os = "windows")]
    dispatch::<windows::WinPlatform>(cmd, &args);

    #[cfg(target_os = "linux")]
    dispatch::<linux::LinuxPlatform>(cmd, &args);

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        eprintln!("Platform not supported.");
        std::process::exit(1);
    }
}

// Generic function — the compiler generates a specialized copy for each Platform type.
// P: Platform = "P must implement the Platform trait" (like <P extends Platform> in TS).
// &[String] = slice — a borrowed, read-only view into the args Vec.
fn dispatch<P: Platform>(cmd: &str, args: &[String]) {
    // match = switch but exhaustive (compiler ensures all cases are covered).
    // No break needed — only the matched arm runs.
    match cmd {
        "set_all" => {
            // Chain: safe index -> try parse -> exit on failure.
            // .and_then() = flatMap for Option — if any step returns None, the whole chain is None.
            // .unwrap_or_else() runs the closure only if None (lazy eval, unlike unwrap_or).
            let level: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            let mode = args.get(3).map(|s| s.as_str()).unwrap_or("force");
            cmd_set_all::<P>(level, mode);
        }
        "set_one" => {
            let id = args.get(2).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            let level: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            let mode = args.get(4).map(|s| s.as_str()).unwrap_or("force");
            cmd_set_one::<P>(id, level, mode);
        }
        "get_all" => cmd_get::<P>(None),   // None = no filter, get all displays
        "get_one" => {
            let id = args.get(2).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            cmd_get::<P>(Some(id));        // Some(id) = filter to this display
        }
        "list" => cmd_list::<P>(),
        "debug" => cmd_debug::<P>(),
        "reset" => {
            P::reset_all_gamma(); // static method call on the Platform type
            eprintln!("Gamma reset to defaults.");
        }
        "dark" => cmd_theme(true),
        "light" => cmd_theme(false),
        "theme" => cmd_get_theme(),
        "get_volume" => cmd_get_volume(),
        "set_volume" => {
            let level: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            cmd_set_volume(level.min(100));
        }
        "mute" => cmd_set_mute(true),
        "unmute" => cmd_set_mute(false),
        "set_contrast_all" => {
            let level: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            cmd_set_contrast_all::<P>(level.min(100));
        }
        "set_contrast_one" => {
            let id = args.get(2).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            let level: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            cmd_set_contrast_one::<P>(id, level.min(100));
        }
        "get_scale" => cmd_get_scale(),
        "set_scale_all" => {
            let pct: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            cmd_set_scale_all(clamp_scale(pct));
        }
        "set_scale_one" => {
            let id = args.get(2).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            let pct: u16 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            cmd_set_scale_one(id, clamp_scale(pct));
        }
        "keep_awake_on" => cmd_keep_awake_on(),
        "keep_awake_off" => cmd_keep_awake_off(),
        "get_keep_awake" => cmd_get_keep_awake(),
        "set_wallpaper" => {
            let fit = args.get(2).map(|s| s.as_str()).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            let path = args.get(3).map(|s| s.as_str()).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            cmd_set_wallpaper(fit, path);
        }
        "set_wallpaper_one" => {
            let index: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            let fit = args.get(3).map(|s| s.as_str()).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            let path = args.get(4).map(|s| s.as_str()).unwrap_or_else(|| {
                usage();
                std::process::exit(1);
            });
            cmd_set_wallpaper_one(index, fit, path);
        }
        "get_wallpaper" => cmd_get_wallpaper(),
        "get_wallpaper_supported" => cmd_get_wallpaper_supported(),
        "serve" => {
            let port: u16 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(51337);
            cmd_serve::<P>(port);
        }
        _ => {
            // _ = wildcard/default — matches anything not covered above
            usage();
            if cmd != "help" && cmd != "--help" && cmd != "-h" {
                std::process::exit(1); // unknown command = error
            }
        }
    }
}

/// Print available displays when a lookup fails.
fn print_not_found(query: &str, displays: &[DisplayInfo]) {
    eprintln!("Display \"{}\" not found. Available displays:", query);
    for d in displays {
        let id = if d.id == BUILTIN_ID { "0" } else { &d.id };
        eprintln!("  {}: {}", id, d.name);
    }
}

/// Match a display by ID, name (case-insensitive), or "0" as alias for builtin.
fn matches_display(info: &DisplayInfo, query: &str) -> bool {
    if query == "0" {
        return info.id == BUILTIN_ID;
    }
    info.id == query || info.name.to_lowercase() == query.to_lowercase()
}

/// Set brightness on all displays. Enumerates, sets each, prints status to stderr.
/// Calls maybe_keep_alive() at the end to hold gamma state if needed.
fn cmd_set_all<P: Platform>(level: u16, mode: &str) {
    let displays = P::enumerate();
    eprintln!("Setting all {} display(s) to {}% [mode={}]\n", displays.len(), level, mode);

    // Destructure each tuple: (DisplayInfo, Box<dyn DisplayControl>).
    // `mut ctrl` = we need mutability because set_brightness takes &mut self.
    for (info, mut ctrl) in displays {
        eprint!("  {} ({}): ", info.id, info.name); // eprint = stderr, no newline
        if ctrl.set_brightness(level, mode) {
            eprintln!("OK");
        } else {
            eprintln!("FAILED");
        }
    }

    maybe_keep_alive(mode);
}

/// Set brightness on a single display by ID or name.
/// Exits with code 1 if the display is not found (prints available displays).
fn cmd_set_one<P: Platform>(id: &str, level: u16, mode: &str) {
    let displays = P::enumerate();
    // Clone display infos before the loop consumes the Vec — we need them for error messages.
    // .iter() borrows, .map() + .clone() copies each DisplayInfo, .collect() gathers into a Vec.
    let all_infos: Vec<DisplayInfo> = displays.iter().map(|(info, _)| info.clone()).collect();
    for (info, mut ctrl) in displays {
        // &info = pass by reference (borrows without moving ownership)
        if matches_display(&info, id) {
            eprint!("  {} ({}): ", info.id, info.name);
            if ctrl.set_brightness(level, mode) {
                eprintln!("OK");
            } else {
                eprintln!("FAILED");
            }
            maybe_keep_alive(mode);
            return; // early exit — display found
        }
    }
    // Only reached if no display matched
    print_not_found(id, &all_infos);
    std::process::exit(1);
}

/// Set contrast on all displays. Only works on DDC-capable monitors.
/// Built-in displays and monitors without DDC will report FAILED (expected).
fn cmd_set_contrast_all<P: Platform>(level: u16) {
    let displays = P::enumerate();
    eprintln!("Setting contrast on all {} display(s) to {}%\n", displays.len(), level);
    for (info, mut ctrl) in displays {
        eprint!("  {} ({}): ", info.id, info.name);
        if ctrl.set_contrast(level) {
            eprintln!("OK");
        } else {
            eprintln!("FAILED (DDC not supported or contrast unavailable)");
        }
    }
}

/// Set contrast on a single display by ID or name.
/// Exits with code 1 if the display is not found.
fn cmd_set_contrast_one<P: Platform>(id: &str, level: u16) {
    let displays = P::enumerate();
    let all_infos: Vec<DisplayInfo> = displays.iter().map(|(info, _)| info.clone()).collect();
    for (info, mut ctrl) in displays {
        if matches_display(&info, id) {
            eprint!("  {} ({}): ", info.id, info.name);
            if ctrl.set_contrast(level) {
                eprintln!("OK");
            } else {
                eprintln!("FAILED (DDC not supported or contrast unavailable)");
            }
            return;
        }
    }
    print_not_found(id, &all_infos);
    std::process::exit(1);
}

/// Get live brightness/contrast for displays. Outputs JSON to stdout.
/// If filter_id is Some, returns a single DisplayInfo object for that display.
/// If filter_id is None, returns a JSON array of all displays.
/// Re-reads brightness/contrast from hardware (not cached values from enumerate).
fn cmd_get<P: Platform>(filter_id: Option<&String>) {
    let displays = P::enumerate();
    let all_infos: Vec<DisplayInfo> = displays.iter().map(|(info, _)| info.clone()).collect();
    let mut results: Vec<DisplayInfo> = Vec::new();

    for (info, mut ctrl) in displays {
        // if let = pattern match + extract in one step.
        // Only enters the block if filter_id is Some(id), binding the inner value to `id`.
        if let Some(id) = filter_id {
            if !matches_display(&info, id) {
                continue; // skip non-matching displays
            }
        }
        // Re-bind as mutable to update fields (Rust vars are immutable by default).
        // This is a move, not a copy — the original `info` is gone.
        let mut info = info;
        info.brightness = ctrl.get_brightness(); // re-read live values from hardware
        info.contrast = ctrl.get_contrast();
        results.push(info);
    }

    if let Some(id) = filter_id {
        if let Some(item) = results.first() {
            // stdout = JSON only (machine-readable, safe to pipe)
            println!("{}", serde_json::to_string_pretty(item).unwrap());
        } else {
            print_not_found(id, &all_infos);
            std::process::exit(1);
        }
    } else {
        // &results = pass by reference so serde reads without taking ownership
        println!("{}", serde_json::to_string_pretty(&results).unwrap());
    }
}

/// List all detected displays without re-reading live values.
/// Outputs a JSON array of DisplayInfo to stdout. Brightness/contrast values
/// come from the initial enumerate() call, not fresh hardware reads.
fn cmd_list<P: Platform>() {
    let displays = P::enumerate();
    // .into_iter() consumes the Vec (moves ownership — displays is gone after this).
    // |(info, _)| = destructure tuple, _ discards the DisplayControl we don't need.
    // .collect() gathers the lazy iterator into a concrete Vec.
    let infos: Vec<DisplayInfo> = displays.into_iter().map(|(info, _)| info).collect();
    println!("{}", serde_json::to_string_pretty(&infos).unwrap());
}

/// Run full diagnostics: enumerate displays, exercise each control path
/// (DDC, gamma, force), test volume and theme, then restore everything.
/// Outputs a comprehensive JSON report to stdout.
fn cmd_debug<P: Platform>() {
    eprintln!("Running diagnostics — brightness, volume, and theme will change momentarily...");
    eprintln!();
    let debug = build_debug_info::<P>();
    println!("{}", serde_json::to_string_pretty(&debug).unwrap());
}

/// Build the full debug JSON object. Separated from cmd_debug so it can also
/// be called by the HTTP server's /debug endpoint without printing to stderr.
fn build_debug_info<P: Platform>() -> serde_json::Value {
    let displays = P::enumerate();
    let infos: Vec<DisplayInfo> = displays.iter().map(|(info, _)| info.clone()).collect();

    // --- Active tests: exercise each control path and report results ---
    let mut display_tests = Vec::new();
    for (info, mut ctrl) in displays {
        eprintln!("  Testing display {} ({})...", info.id, info.name);
        display_tests.push(debug_test_display(&info, &mut *ctrl));
    }
    eprintln!("  Testing volume...");
    let volume_tests = debug_test_volume();
    eprintln!("  Testing theme...");
    let theme_tests = debug_test_theme();
    eprintln!();

    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "displays": infos,
        "scale": get_all_scales(),
        "platform": P::debug_info(),
        "tests": {
            "displays": display_tests,
            "volume": volume_tests,
            "theme": theme_tests,
        },
    })
}

/// HTTP handler for /debug — builds the debug report and returns it as a JSON string.
fn serve_debug<P: Platform>() -> String {
    serde_json::to_string_pretty(&build_debug_info::<P>()).unwrap_or_else(|_| "{}".into())
}

/// Test brightness get/set for a single display across all modes.
/// Saves the initial value, runs tests, and restores.
fn debug_test_display(info: &DisplayInfo, ctrl: &mut dyn DisplayControl) -> serde_json::Value {
    let initial_brightness = ctrl.get_brightness();
    let initial_contrast = ctrl.get_contrast();

    // --- Brightness: try each mode at 25%, read back after each ---
    let set_25_ddc = ctrl.set_brightness(25, "ddc");
    let get_after_ddc = ctrl.get_brightness();

    let set_25_gamma = ctrl.set_brightness(25, "gamma");
    let get_after_gamma = ctrl.get_brightness();

    let set_25_force = ctrl.set_brightness(25, "force");
    let get_after_force = ctrl.get_brightness();

    // Restore brightness to initial (or 100 if we couldn't read it)
    let restore_val = initial_brightness.unwrap_or(100) as u16;
    let restore_ok = ctrl.set_brightness(restore_val, "force");
    ctrl.reset_gamma();
    let get_after_restore = ctrl.get_brightness();

    // --- Contrast: set to 50, read back, restore ---
    let set_contrast_50 = ctrl.set_contrast(50);
    let get_after_contrast_set = ctrl.get_contrast();
    let restore_contrast_val = initial_contrast.unwrap_or(20) as u16;
    ctrl.set_contrast(restore_contrast_val);

    serde_json::json!({
        "id": info.id,
        "name": info.name,
        "ddc_supported": info.ddc_supported,
        "initial_brightness": initial_brightness,
        "initial_contrast": initial_contrast,
        "set_brightness_25_ddc": set_25_ddc,
        "get_after_ddc": get_after_ddc,
        "set_brightness_25_gamma": set_25_gamma,
        "get_after_gamma": get_after_gamma,
        "set_brightness_25_force": set_25_force,
        "get_after_force": get_after_force,
        "restore_brightness": restore_ok,
        "get_after_restore": get_after_restore,
        "set_contrast_50": set_contrast_50,
        "get_after_contrast_set": get_after_contrast_set,
    })
}

/// Test volume get/set/mute. Saves initial state and restores after.
fn debug_test_volume() -> serde_json::Value {
    let initial = get_volume();

    let set_25 = set_volume(25);
    let after_25 = get_volume();

    let set_100 = set_volume(100);
    let after_100 = get_volume();

    let mute_ok = set_mute(true);
    let after_mute = get_volume();

    let unmute_ok = set_mute(false);
    let after_unmute = get_volume();

    // Restore original volume and mute state
    if let Some(ref orig) = initial {
        set_volume(orig.volume as u16);
        if orig.muted { set_mute(true); }
    }

    let vol_json = |v: &Option<VolumeInfo>| -> serde_json::Value {
        match v {
            Some(vi) => serde_json::json!({"volume": vi.volume, "muted": vi.muted}),
            None => serde_json::Value::Null,
        }
    };

    serde_json::json!({
        "initial": vol_json(&initial),
        "set_25": set_25,
        "get_after_25": vol_json(&after_25),
        "set_100": set_100,
        "get_after_100": vol_json(&after_100),
        "mute": mute_ok,
        "get_after_mute": vol_json(&after_mute),
        "unmute": unmute_ok,
        "get_after_unmute": vol_json(&after_unmute),
    })
}

/// Test dark/light mode toggle. Saves initial theme and restores after.
fn debug_test_theme() -> serde_json::Value {
    let theme_json = |v: Option<bool>| -> serde_json::Value {
        match v {
            Some(true) => serde_json::json!("dark"),
            Some(false) => serde_json::json!("light"),
            None => serde_json::Value::Null,
        }
    };

    let initial = get_dark_mode();

    let set_dark_ok = set_dark_mode(true);
    let after_dark = get_dark_mode();

    let set_light_ok = set_dark_mode(false);
    let after_light = get_dark_mode();

    // Restore original theme
    let restored = if let Some(was_dark) = initial {
        set_dark_mode(was_dark)
    } else {
        false
    };

    serde_json::json!({
        "initial": theme_json(initial),
        "set_dark": set_dark_ok,
        "get_after_dark": theme_json(after_dark),
        "set_light": set_light_ok,
        "get_after_light": theme_json(after_light),
        "restored": restored,
    })
}

/// Keep the process alive when gamma mode is used — gamma tables reset when the process exits.
/// DDC changes are persistent (stored in monitor firmware), so no keep-alive needed.
fn maybe_keep_alive(mode: &str) {
    if mode == "force" || mode == "gamma" {
        eprintln!("\nPress Ctrl+C to exit (gamma will reset).");
        // loop = infinite loop. thread::sleep blocks the thread (unlike JS setInterval).
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }
}

// =========================================================================
// HTTP server — keeps process alive for gamma, accepts commands via HTTP.
// Binds to 127.0.0.1 only (localhost, not exposed to network).
// =========================================================================

/// Start the HTTP server on localhost. Handles one request at a time (single-threaded).
/// The server keeps the process alive, which is essential for gamma persistence on macOS/Windows.
/// All routes are GET with path-based parameters. Responses are JSON with CORS headers.
fn cmd_serve<P: Platform>(port: u16) {
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;
    use std::time::Instant;

    let started = Instant::now();
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("Failed to bind to {}: {}", addr, e);
        std::process::exit(1);
    });
    eprintln!("display-dj server listening on http://{}", addr);
    eprintln!();
    eprintln!("Routes:  /list  /get_all  /get_one/<id>  /set_all/<level>  /set_all/<level>/<mode>");
    eprintln!("         /set_one/<id>/<level>  /set_one/<id>/<level>/<mode>");
    eprintln!("         /set_contrast_all/<level>  /set_contrast_one/<id>/<level>");
    eprintln!("         /dark  /light  /theme  /reset  /health  /debug");
    eprintln!("         /get_volume  /set_volume/<level>  /mute  /unmute");
    eprintln!("         /keep_awake  /keep_awake/enable  /keep_awake/disable");
    eprintln!("         /get_scale  /set_scale_all/<percent>  /set_scale_one/<id>/<percent>");
    eprintln!("         /set_wallpaper/<fit>/<path>  /set_wallpaper_one/<index>/<fit>/<path>");
    eprintln!("         /get_wallpaper  /get_wallpaper_supported");
    eprintln!();
    eprintln!("Example: curl http://{}:{}/set_all/50", "127.0.0.1", port);

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };

        let reader = BufReader::new(&stream);
        let request_line = match reader.lines().next() {
            Some(Ok(line)) => line,
            _ => continue,
        };

        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 2 {
            let _ = write_http(&mut stream, 400, r#"{"error":"bad request"}"#);
            continue;
        }

        // Split path into segments: "/set_one/2/50/force" -> ["set_one", "2", "50", "force"]
        let segments: Vec<&str> = parts[1].trim_start_matches('/')
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();

        let cmd = segments.first().copied().unwrap_or("");
        let url_decode = |s: &str| s.replace("%20", " ").replace("+", " ");

        let json = match cmd {
            "" => serde_json::to_string(&serde_json::json!({
                "name": "display-dj",
                "version": env!("CARGO_PKG_VERSION"),
                "routes": [
                    "/list", "/get_all", "/get_one/<id>",
                    "/set_all/<level>", "/set_all/<level>/<mode>",
                    "/set_one/<id>/<level>", "/set_one/<id>/<level>/<mode>",
                    "/set_contrast_all/<level>", "/set_contrast_one/<id>/<level>",
                    "/dark", "/light", "/theme",
                    "/get_volume", "/set_volume/<level>", "/mute", "/unmute",
                    "/keep_awake", "/keep_awake/enable", "/keep_awake/disable",
                    "/get_scale", "/set_scale_all/<percent>", "/set_scale_one/<id>/<percent>",
                    "/set_wallpaper/<fit>/<path>", "/set_wallpaper_one/<index>/<fit>/<path>",
                    "/get_wallpaper", "/get_wallpaper_supported",
                    "/reset", "/health", "/debug"
                ]
            })).unwrap(),
            "health" => serve_health(&started),
            "debug" => serve_debug::<P>(),
            "list" => serve_list::<P>(),
            "get_all" => serve_get::<P>(None),
            "get_one" => match segments.get(1) {
                Some(id) => serve_get::<P>(Some(&url_decode(id))),
                None => r#"{"error":"usage: /get_one/<id>"}"#.to_string(),
            },
            "set_all" => match segments.get(1).and_then(|l| l.parse::<u16>().ok()) {
                Some(level) => {
                    let mode = segments.get(2).copied().unwrap_or("force");
                    serve_set_all::<P>(level, mode)
                }
                None => r#"{"error":"usage: /set_all/<level> or /set_all/<level>/<mode>"}"#.to_string(),
            },
            "set_one" => {
                let id = segments.get(1).map(|s| url_decode(s));
                let level = segments.get(2).and_then(|l| l.parse::<u16>().ok());
                match (id, level) {
                    (Some(id), Some(level)) => {
                        let mode = segments.get(3).copied().unwrap_or("force");
                        serve_set_one::<P>(&id, level, mode)
                    }
                    _ => r#"{"error":"usage: /set_one/<id>/<level> or /set_one/<id>/<level>/<mode>"}"#.to_string(),
                }
            }
            "set_contrast_all" => match segments.get(1).and_then(|l| l.parse::<u16>().ok()) {
                Some(level) => serve_set_contrast_all::<P>(level.min(100)),
                None => r#"{"error":"usage: /set_contrast_all/<level> (0-100)"}"#.to_string(),
            },
            "set_contrast_one" => {
                let id = segments.get(1).map(|s| url_decode(s));
                let level = segments.get(2).and_then(|l| l.parse::<u16>().ok());
                match (id, level) {
                    (Some(id), Some(level)) => serve_set_contrast_one::<P>(&id, level.min(100)),
                    _ => r#"{"error":"usage: /set_contrast_one/<id>/<level>"}"#.to_string(),
                }
            }
            "reset" => { P::reset_all_gamma(); r#"{"status":"ok"}"#.to_string() }
            "dark" => format!(r#"{{"status":"{}"}}"#, if set_dark_mode(true) { "ok" } else { "failed" }),
            "light" => format!(r#"{{"status":"{}"}}"#, if set_dark_mode(false) { "ok" } else { "failed" }),
            "theme" => match get_dark_mode() {
                Some(true) => r#"{"theme":"dark"}"#.to_string(),
                Some(false) => r#"{"theme":"light"}"#.to_string(),
                None => r#"{"error":"could not detect theme"}"#.to_string(),
            },
            "get_volume" => serve_get_volume(),
            "set_volume" => match segments.get(1).and_then(|l| l.parse::<u16>().ok()) {
                Some(level) => serve_set_volume(level.min(100)),
                None => r#"{"error":"usage: /set_volume/<level>"}"#.to_string(),
            },
            "mute" => { set_mute(true); r#"{"status":"ok"}"#.to_string() }
            "unmute" => { set_mute(false); r#"{"status":"ok"}"#.to_string() }
            "keep_awake" => match segments.get(1).copied() {
                Some("enable") => serve_keep_awake_enable(),
                Some("disable") => serve_keep_awake_disable(),
                _ => serve_get_keep_awake(),
            },
            "get_scale" => serve_get_scale(),
            "set_scale_all" => match segments.get(1).and_then(|l| l.parse::<u16>().ok()) {
                Some(pct) => serve_set_scale_all(clamp_scale(pct)),
                None => r#"{"error":"usage: /set_scale_all/<percent> (75-300)"}"#.to_string(),
            },
            "set_scale_one" => {
                let id = segments.get(1).map(|s| url_decode(s));
                let pct = segments.get(2).and_then(|l| l.parse::<u16>().ok());
                match (id, pct) {
                    (Some(id), Some(pct)) => serve_set_scale_one(&id, clamp_scale(pct)),
                    _ => r#"{"error":"usage: /set_scale_one/<id>/<percent>"}"#.to_string(),
                }
            }
            "set_wallpaper" => {
                let fit = segments.get(1).copied().unwrap_or("");
                if segments.len() < 3 {
                    r#"{"error":"usage: /set_wallpaper/<fit>/<path>"}"#.to_string()
                } else {
                    // Rejoin remaining segments to reconstruct the absolute path
                    let path = format!("/{}", segments[2..].join("/"));
                    let path = url_decode(&path);
                    serve_set_wallpaper(fit, &path)
                }
            }
            "set_wallpaper_one" => {
                let index = segments.get(1).and_then(|s| s.parse::<usize>().ok());
                let fit = segments.get(2).copied().unwrap_or("");
                match index {
                    Some(idx) if segments.len() >= 4 => {
                        let path = format!("/{}", segments[3..].join("/"));
                        let path = url_decode(&path);
                        serve_set_wallpaper_one(idx, fit, &path)
                    }
                    _ => r#"{"error":"usage: /set_wallpaper_one/<index>/<fit>/<path>"}"#.to_string(),
                }
            }
            "get_wallpaper" => serve_get_wallpaper(),
            "get_wallpaper_supported" => serve_get_wallpaper_supported(),
            _ => {
                let _ = write_http(&mut stream, 404, r#"{"error":"not found"}"#);
                continue;
            }
        };

        let status = if json.contains("\"error\"") { 400 } else { 200 };
        let _ = write_http(&mut stream, status, &json);
    }
}

/// Write a minimal HTTP/1.1 response with JSON content type and CORS headers.
/// Connection: close ensures the client doesn't wait for more data.
fn write_http(stream: &mut std::net::TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    use std::io::Write;
    let reason = match status { 200 => "OK", 400 => "Bad Request", 404 => "Not Found", _ => "Error" };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status, reason, body.len(), body
    )
}

/// HTTP handler for /health — returns server status, process ID, and uptime in seconds.
fn serve_health(started: &std::time::Instant) -> String {
    serde_json::to_string(&serde_json::json!({
        "status": "ok",
        "pid": std::process::id(),
        "uptime": started.elapsed().as_secs()
    })).unwrap()
}

/// HTTP handler for /list — returns all displays as a JSON array (no live re-reads).
fn serve_list<P: Platform>() -> String {
    let displays = P::enumerate();
    let infos: Vec<DisplayInfo> = displays.into_iter().map(|(info, _)| info).collect();
    serde_json::to_string(&infos).unwrap_or_else(|_| "[]".into())
}

/// HTTP handler for /get_all and /get_one/<id>.
/// Re-reads live brightness/contrast from hardware. Returns JSON array (get_all)
/// or single object (get_one). Returns error JSON if display not found.
fn serve_get<P: Platform>(filter_id: Option<&str>) -> String {
    let displays = P::enumerate();
    let mut results: Vec<DisplayInfo> = Vec::new();
    for (info, mut ctrl) in displays {
        if let Some(id) = filter_id {
            if !matches_display(&info, id) { continue; }
        }
        let mut info = info;
        info.brightness = ctrl.get_brightness();
        info.contrast = ctrl.get_contrast();
        results.push(info);
    }
    if let Some(id) = filter_id {
        match results.first() {
            Some(item) => serde_json::to_string(item).unwrap_or_else(|_| "{}".into()),
            None => format!(r#"{{"error":"display '{}' not found"}}"#, id),
        }
    } else {
        serde_json::to_string(&results).unwrap_or_else(|_| "[]".into())
    }
}

/// HTTP handler for /set_all/<level>[/<mode>].
/// Sets brightness on all displays and returns per-display status as JSON array.
fn serve_set_all<P: Platform>(level: u16, mode: &str) -> String {
    let displays = P::enumerate();
    let mut results: Vec<serde_json::Value> = Vec::new();
    for (info, mut ctrl) in displays {
        let ok = ctrl.set_brightness(level, mode);
        results.push(serde_json::json!({
            "id": info.id,
            "name": info.name,
            "status": if ok { "ok" } else { "failed" }
        }));
    }
    serde_json::to_string(&results).unwrap_or_else(|_| "[]".into())
}

/// HTTP handler for /set_one/<id>/<level>[/<mode>].
/// Sets brightness on a single display by ID or name. Returns status JSON
/// or error JSON if display not found.
fn serve_set_one<P: Platform>(id: &str, level: u16, mode: &str) -> String {
    let displays = P::enumerate();
    for (info, mut ctrl) in displays {
        if matches_display(&info, id) {
            let ok = ctrl.set_brightness(level, mode);
            return serde_json::to_string(&serde_json::json!({
                "id": info.id,
                "name": info.name,
                "status": if ok { "ok" } else { "failed" }
            })).unwrap_or_else(|_| "{}".into());
        }
    }
    format!(r#"{{"error":"display '{}' not found"}}"#, id)
}

/// HTTP handler for /set_contrast_all/<level>.
/// Sets contrast on all displays via DDC/CI and returns per-display status.
/// Monitors without DDC support will report "failed".
fn serve_set_contrast_all<P: Platform>(level: u16) -> String {
    let displays = P::enumerate();
    let mut results: Vec<serde_json::Value> = Vec::new();
    for (info, mut ctrl) in displays {
        let ok = ctrl.set_contrast(level);
        results.push(serde_json::json!({
            "id": info.id,
            "name": info.name,
            "status": if ok { "ok" } else { "failed" }
        }));
    }
    serde_json::to_string(&results).unwrap_or_else(|_| "[]".into())
}

/// HTTP handler for /set_contrast_one/<id>/<level>.
/// Sets contrast on a single display by ID or name via DDC/CI.
fn serve_set_contrast_one<P: Platform>(id: &str, level: u16) -> String {
    let displays = P::enumerate();
    for (info, mut ctrl) in displays {
        if matches_display(&info, id) {
            let ok = ctrl.set_contrast(level);
            return serde_json::to_string(&serde_json::json!({
                "id": info.id,
                "name": info.name,
                "status": if ok { "ok" } else { "failed" }
            })).unwrap_or_else(|_| "{}".into());
        }
    }
    format!(r#"{{"error":"display '{}' not found"}}"#, id)
}

// =========================================================================
// Dark / Light mode — platform-specific implementations via #[cfg].
// Unlike display control (which uses traits + platform modules), dark mode
// is simple enough to live in main.rs behind conditional compilation.
// =========================================================================

/// CLI handler for `dark` and `light` commands.
/// Prints status to stderr and exits with code 1 on failure.
fn cmd_theme(dark: bool) {
    // if-as-expression: returns a value like JS ternary (condition ? a : b)
    let label = if dark { "dark" } else { "light" };
    if set_dark_mode(dark) {
        eprintln!("Switched to {} mode.", label);
    } else {
        eprintln!("Failed to switch to {} mode.", label);
        std::process::exit(1);
    }
}

/// CLI handler for `theme` command — outputs {"theme": "dark"|"light"} to stdout.
fn cmd_get_theme() {
    // Inline struct — only used here, so defined locally.
    // #[derive(Serialize)] makes it JSON-serializable.
    #[derive(Serialize)]
    struct ThemeInfo { theme: String }

    match get_dark_mode() {
        // Pattern matching on Option<bool> — three possible values:
        Some(true) => println!("{}", serde_json::to_string_pretty(&ThemeInfo { theme: "dark".into() }).unwrap()),
        Some(false) => println!("{}", serde_json::to_string_pretty(&ThemeInfo { theme: "light".into() }).unwrap()),
        None => {
            eprintln!("Could not detect current theme.");
            std::process::exit(1);
        }
    }
}

// --- macOS dark mode: AppleScript via osascript ---

/// Set dark/light mode on macOS via System Events AppleScript.
/// Toggles the system-wide appearance preference.
#[cfg(target_os = "macos")]
fn set_dark_mode(dark: bool) -> bool {
    let val = if dark { "true" } else { "false" };
    let script = format!(
        "tell application \"System Events\" to tell appearance preferences to set dark mode to {}",
        val
    );
    // .map() transforms Ok(output) -> Ok(bool), .unwrap_or(false) handles Err case
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get current dark mode state on macOS. Returns Some(true) for dark, Some(false)
/// for light, None if detection fails.
#[cfg(target_os = "macos")]
fn get_dark_mode() -> Option<bool> {
    let output = std::process::Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to tell appearance preferences to get dark mode"])
        .output()
        .ok()?; // .ok() converts Result->Option, ? returns None early on failure
    if !output.status.success() { return None; }
    let val = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
    Some(val == "true")
}

// --- Windows dark mode: registry keys + WM_SETTINGCHANGE broadcast ---

/// Set dark/light mode on Windows by writing to the Personalize registry keys.
/// Sets both AppsUseLightTheme (app chrome) and SystemUsesLightTheme (taskbar/start menu).
/// Broadcasts WM_SETTINGCHANGE so already-open windows refresh their title bars.
#[cfg(target_os = "windows")]
fn set_dark_mode(dark: bool) -> bool {
    // Windows uses 0=dark, 1=light (inverted from what you'd expect)
    let val = if dark { "0" } else { "1" };
    // Must set both keys — AppsUseLightTheme for app chrome, SystemUsesLightTheme for taskbar
    let app = std::process::Command::new("reg")
        .args(["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
               "/v", "AppsUseLightTheme", "/t", "REG_DWORD", "/d", val, "/f"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let sys = std::process::Command::new("reg")
        .args(["add", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
               "/v", "SystemUsesLightTheme", "/t", "REG_DWORD", "/d", val, "/f"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if app && sys {
        // Broadcast WM_SETTINGCHANGE so existing windows refresh their title bars
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", r#"
                Add-Type -TypeDefinition @'
                using System;
                using System.Runtime.InteropServices;
                public class ThemeBroadcast {
                    [DllImport("user32.dll", SetLastError = true, CharSet = CharSet.Auto)]
                    public static extern IntPtr SendMessageTimeout(
                        IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam,
                        uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);
                    public static void Broadcast() {
                        UIntPtr result;
                        SendMessageTimeout((IntPtr)0xffff, 0x001A, UIntPtr.Zero,
                            "ImmersiveColorSet", 0x0002, 5000, out result);
                    }
                }
'@
                [ThemeBroadcast]::Broadcast()
            "#])
            .output();
        true
    } else {
        false
    }
}

/// Get current dark mode state on Windows by reading the registry.
/// AppsUseLightTheme: 0 = dark mode ON, 1 = light mode (note: inverted naming).
#[cfg(target_os = "windows")]
fn get_dark_mode() -> Option<bool> {
    let output = std::process::Command::new("reg")
        .args(["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
               "/v", "AppsUseLightTheme"])
        .output()
        .ok()?;
    if !output.status.success() { return None; }
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("0x0") {
        Some(true)  // 0 = dark mode ON
    } else if stdout.contains("0x1") {
        Some(false) // 1 = light mode
    } else {
        None
    }
}

// --- Linux dark mode: tries desktop environments in order (GNOME -> KDE -> XFCE) ---

/// Set dark/light mode on Linux. Tries GNOME (gsettings color-scheme + gtk-theme),
/// KDE (plasma-apply-colorscheme), and XFCE (xfconf-query) in order.
/// Returns true on first success, false if no supported DE was found.
#[cfg(target_os = "linux")]
fn set_dark_mode(dark: bool) -> bool {
    let gtk_theme = if dark { "Adwaita-dark" } else { "Adwaita" };
    let color_scheme = if dark { "prefer-dark" } else { "prefer-light" };

    // GNOME 42+ uses color-scheme (the modern way)
    if std::process::Command::new("gsettings")
        .args(["set", "org.gnome.desktop.interface", "color-scheme", color_scheme])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        // Also set gtk-theme for older GTK3 apps that don't read color-scheme
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.desktop.interface", "gtk-theme", gtk_theme])
            .output();
        return true;
    }

    // KDE Plasma
    if std::process::Command::new("plasma-apply-colorscheme")
        .arg(if dark { "BreezeDark" } else { "BreezeLight" })
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return true;
    }

    // XFCE — uses xfconf for settings
    let xfce_theme = if dark { "Adwaita-dark" } else { "Adwaita" };
    if std::process::Command::new("xfconf-query")
        .args(["-c", "xsettings", "-p", "/Net/ThemeName", "-s", xfce_theme])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return true;
    }

    false // no supported desktop environment found
}

/// Get current dark mode state on Linux. Tries GNOME color-scheme, GNOME gtk-theme
/// (fallback), and KDE color scheme in order. Returns None if no DE detected.
#[cfg(target_os = "linux")]
fn get_dark_mode() -> Option<bool> {
    // GNOME: check color-scheme first (more reliable than theme name)
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            if val.contains("dark") { return Some(true); }
            if val.contains("light") || val.contains("default") { return Some(false); }
        }
    }

    // GNOME fallback: check the GTK theme name for "dark" substring
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "gtk-theme"])
        .output()
    {
        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            return Some(val.contains("dark"));
        }
    }

    // KDE: read the color scheme name
    if let Ok(output) = std::process::Command::new("kreadconfig5")
        .args(["--group", "General", "--key", "ColorScheme"])
        .output()
    {
        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
            return Some(val.contains("dark"));
        }
    }

    None // couldn't detect theme on any DE
}

// =========================================================================
// Volume control — adjusts the default/currently-selected audio output.
// Cross-platform: macOS (osascript), Windows (PowerShell), Linux (pactl/amixer).
// =========================================================================

/// System audio volume state. Returned by get_volume and the /get_volume endpoint.
#[derive(Serialize)]
struct VolumeInfo {
    volume: u32, // 0-100 percentage
    muted: bool, // true if the default output is muted
}

/// CLI handler for `get_volume` — outputs {"volume": N, "muted": bool} to stdout.
fn cmd_get_volume() {
    match get_volume() {
        Some(info) => println!("{}", serde_json::to_string_pretty(&info).unwrap()),
        None => {
            eprintln!("Could not read volume.");
            std::process::exit(1);
        }
    }
}

/// CLI handler for `set_volume <level>` — sets system volume and prints status to stderr.
fn cmd_set_volume(level: u16) {
    if set_volume(level) {
        eprintln!("Volume set to {}%.", level);
    } else {
        eprintln!("Failed to set volume.");
        std::process::exit(1);
    }
}

/// CLI handler for `mute` and `unmute` — toggles audio mute state.
fn cmd_set_mute(mute: bool) {
    if set_mute(mute) {
        eprintln!("Audio {}.", if mute { "muted" } else { "unmuted" });
    } else {
        eprintln!("Failed to {} audio.", if mute { "mute" } else { "unmute" });
        std::process::exit(1);
    }
}

/// HTTP handler for /get_volume — returns {"volume": N, "muted": bool} or error.
fn serve_get_volume() -> String {
    match get_volume() {
        Some(info) => serde_json::to_string(&info).unwrap_or_else(|_| "{}".into()),
        None => r#"{"error":"could not read volume"}"#.to_string(),
    }
}

/// HTTP handler for /set_volume/<level> — sets volume and returns status JSON.
fn serve_set_volume(level: u16) -> String {
    if set_volume(level) {
        format!(r#"{{"status":"ok","volume":{}}}"#, level)
    } else {
        r#"{"error":"failed to set volume"}"#.to_string()
    }
}

// --- macOS volume: osascript wrapping AppleScript commands ---

/// Get current volume and mute state on macOS via `osascript`.
/// Makes two osascript calls: one for volume level, one for mute state.
#[cfg(target_os = "macos")]
fn get_volume() -> Option<VolumeInfo> {
    let output = std::process::Command::new("osascript")
        .args(["-e", "output volume of (get volume settings)"])
        .output().ok()?;
    if !output.status.success() { return None; }
    let volume: u32 = String::from_utf8_lossy(&output.stdout).trim().parse().ok()?;

    let output = std::process::Command::new("osascript")
        .args(["-e", "output muted of (get volume settings)"])
        .output().ok()?;
    let muted = String::from_utf8_lossy(&output.stdout).trim().to_lowercase() == "true";

    Some(VolumeInfo { volume, muted })
}

/// Set system volume on macOS via osascript. Level is 0-100.
#[cfg(target_os = "macos")]
fn set_volume(level: u16) -> bool {
    std::process::Command::new("osascript")
        .args(["-e", &format!("set volume output volume {}", level)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Toggle mute on macOS via osascript.
#[cfg(target_os = "macos")]
fn set_mute(mute: bool) -> bool {
    let val = if mute { "true" } else { "false" };
    std::process::Command::new("osascript")
        .args(["-e", &format!("set volume output muted {}", val)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// --- Windows volume: AudioDeviceCmdlets PowerShell module ---
// Requires one-time setup: Install-Module -Name AudioDeviceCmdlets
// https://www.powershellgallery.com/packages/AudioDeviceCmdlets

/// Get current volume and mute state on Windows via AudioDeviceCmdlets PowerShell module.
/// Reads both playback volume and mute state in a single PowerShell invocation.
#[cfg(target_os = "windows")]
fn get_volume() -> Option<VolumeInfo> {
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command",
            "Import-Module AudioDeviceCmdlets; $v = Get-AudioDevice -PlaybackVolume; $m = Get-AudioDevice -PlaybackMute; Write-Output \"$v,$m\""])
        .output().ok()?;
    if !output.status.success() { return None; }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let mut parts = stdout.split(',');
    let volume: f64 = parts.next()?.parse().ok()?;
    let muted = parts.next()?.trim().to_lowercase() == "true";
    Some(VolumeInfo { volume: volume.round() as u32, muted })
}

/// Set system volume on Windows via AudioDeviceCmdlets. Level is 0-100.
#[cfg(target_os = "windows")]
fn set_volume(level: u16) -> bool {
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command",
            &format!("Import-Module AudioDeviceCmdlets; Set-AudioDevice -PlaybackVolume {}", level)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Toggle mute on Windows via AudioDeviceCmdlets. Uses 1/0 instead of true/false.
#[cfg(target_os = "windows")]
fn set_mute(mute: bool) -> bool {
    let val = if mute { "1" } else { "0" };
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command",
            &format!("Import-Module AudioDeviceCmdlets; Set-AudioDevice -PlaybackMute {}", val)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// --- Linux volume: pactl (PulseAudio/PipeWire) with amixer (ALSA) fallback ---

/// Get current volume on Linux. Tries pactl (PulseAudio/PipeWire) first,
/// falls back to amixer (raw ALSA) for minimal setups without PulseAudio.
#[cfg(target_os = "linux")]
fn get_volume() -> Option<VolumeInfo> {
    // Try pactl first (PulseAudio / PipeWire)
    if let Some(info) = get_volume_pactl() { return Some(info); }
    // Fallback to amixer (ALSA)
    get_volume_amixer()
}

/// Get volume via pactl (PulseAudio/PipeWire). Parses the percentage from
/// pactl's output format: "Volume: front-left: 32768 /  50% / -17.50 dB".
#[cfg(target_os = "linux")]
fn get_volume_pactl() -> Option<VolumeInfo> {
    let output = std::process::Command::new("pactl")
        .args(["get-sink-volume", "@DEFAULT_SINK@"])
        .output().ok()?;
    if !output.status.success() { return None; }
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Parse "Volume: front-left: 32768 /  50% / ..."
    let volume = stdout.split('/')
        .find(|s| s.contains('%'))
        .and_then(|s| s.trim().trim_end_matches('%').parse::<u32>().ok())?;

    let mute_output = std::process::Command::new("pactl")
        .args(["get-sink-mute", "@DEFAULT_SINK@"])
        .output().ok()?;
    let muted = String::from_utf8_lossy(&mute_output.stdout)
        .to_lowercase().contains("yes");

    Some(VolumeInfo { volume, muted })
}

/// Get volume via amixer (ALSA fallback). Parses "[75%]" and "[on]/[off]" from output.
#[cfg(target_os = "linux")]
fn get_volume_amixer() -> Option<VolumeInfo> {
    let output = std::process::Command::new("amixer")
        .args(["get", "Master"])
        .output().ok()?;
    if !output.status.success() { return None; }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let volume = stdout.split('[')
        .find(|s| s.contains("%]"))
        .and_then(|s| s.split('%').next())
        .and_then(|s| s.parse::<u32>().ok())?;
    let muted = stdout.contains("[off]");
    Some(VolumeInfo { volume, muted })
}

/// Set volume on Linux. Tries pactl first, falls back to amixer.
#[cfg(target_os = "linux")]
fn set_volume(level: u16) -> bool {
    // Try pactl first
    if std::process::Command::new("pactl")
        .args(["set-sink-volume", "@DEFAULT_SINK@", &format!("{}%", level)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    { return true; }
    // Fallback to amixer
    std::process::Command::new("amixer")
        .args(["set", "Master", &format!("{}%", level)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Toggle mute on Linux. Tries pactl first, falls back to amixer.
#[cfg(target_os = "linux")]
fn set_mute(mute: bool) -> bool {
    let val = if mute { "1" } else { "0" };
    // Try pactl
    if std::process::Command::new("pactl")
        .args(["set-sink-mute", "@DEFAULT_SINK@", val])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    { return true; }
    // Fallback to amixer
    let toggle = if mute { "mute" } else { "unmute" };
    std::process::Command::new("amixer")
        .args(["set", "Master", toggle])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// =========================================================================
// Display scaling — per-monitor scale factor (75% – 300%).
// macOS: resolution switching via displayplacer/system_profiler.
// Windows: DPI registry + rundll32 refresh.
// Linux X11: xrandr --scale. Linux Wayland: wlr-randr --scale.
// =========================================================================

const SCALE_MIN: u16 = 75;  // below 75% UI elements become unusably small
const SCALE_MAX: u16 = 300; // above 300% UI elements become unusably large

/// Clamp a scale percentage to the safe range (75%-300%).
/// Prints a warning to stderr if the value was clamped.
fn clamp_scale(pct: u16) -> u16 {
    let clamped = pct.max(SCALE_MIN).min(SCALE_MAX);
    if clamped != pct {
        eprintln!("Scale clamped to {}% (range: {}%-{}%)", clamped, SCALE_MIN, SCALE_MAX);
    }
    clamped
}

/// Per-display scale factor info. Returned by get_scale and the /get_scale endpoint.
#[derive(Serialize)]
struct ScaleInfo {
    id: String,          // "builtin", "1", "2", ... or "system" (Windows)
    name: String,        // human-readable display/output name
    scale_percent: u32,  // current scale as percentage (100 = native, 200 = 2x/Retina)
}

/// CLI handler for `get_scale` — outputs per-display scale info as JSON array.
fn cmd_get_scale() {
    let scales = get_all_scales();
    println!("{}", serde_json::to_string_pretty(&scales).unwrap());
}

/// CLI handler for `set_scale_all <percent>` — sets scale on all displays.
fn cmd_set_scale_all(pct: u16) {
    let scales = get_all_scales();
    for s in &scales {
        eprint!("  {} ({}): ", s.id, s.name);
        if set_scale(&s.id, pct) {
            eprintln!("OK ({}%)", pct);
        } else {
            eprintln!("FAILED");
        }
    }
}

/// CLI handler for `set_scale_one <id> <percent>` — sets scale on a single display.
fn cmd_set_scale_one(id: &str, pct: u16) {
    let scales = get_all_scales();
    for s in &scales {
        if s.id == id || s.name.to_lowercase() == id.to_lowercase() || (id == "0" && s.id == BUILTIN_ID) {
            eprint!("  {} ({}): ", s.id, s.name);
            if set_scale(&s.id, pct) {
                eprintln!("OK ({}%)", pct);
            } else {
                eprintln!("FAILED");
            }
            return;
        }
    }
    eprintln!("Display \"{}\" not found. Available displays:", id);
    for s in &scales {
        let display_id = if s.id == BUILTIN_ID { "0" } else { &s.id };
        eprintln!("  {}: {}", display_id, s.name);
    }
    std::process::exit(1);
}

/// HTTP handler for /get_scale — returns per-display scale info as JSON array.
fn serve_get_scale() -> String {
    serde_json::to_string(&get_all_scales()).unwrap_or_else(|_| "[]".into())
}

/// HTTP handler for /set_scale_all/<percent> — sets scale on all displays.
fn serve_set_scale_all(pct: u16) -> String {
    let scales = get_all_scales();
    let mut results: Vec<serde_json::Value> = Vec::new();
    for s in &scales {
        let ok = set_scale(&s.id, pct);
        results.push(serde_json::json!({
            "id": s.id, "name": s.name,
            "status": if ok { "ok" } else { "failed" },
            "scale_percent": pct
        }));
    }
    serde_json::to_string(&results).unwrap_or_else(|_| "[]".into())
}

/// HTTP handler for /set_scale_one/<id>/<percent> — sets scale on a single display.
fn serve_set_scale_one(id: &str, pct: u16) -> String {
    let scales = get_all_scales();
    for s in &scales {
        if s.id == id || s.name.to_lowercase() == id.to_lowercase() || (id == "0" && s.id == BUILTIN_ID) {
            let ok = set_scale(&s.id, pct);
            return serde_json::to_string(&serde_json::json!({
                "id": s.id, "name": s.name,
                "status": if ok { "ok" } else { "failed" },
                "scale_percent": pct
            })).unwrap_or_else(|_| "{}".into());
        }
    }
    format!(r#"{{"error":"display '{}' not found"}}"#, id)
}

// --- macOS: CoreGraphics display mode APIs (native, no external deps) ---

#[cfg(target_os = "macos")]
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
    fn CGDisplayIsBuiltin(display: u32) -> i32;
    fn CGDisplayPixelsWide(display: u32) -> u64;
    fn CGDisplayPixelsHigh(display: u32) -> u64;
    fn CGDisplayCopyDisplayMode(display: u32) -> *const std::ffi::c_void;
    fn CGDisplayCopyAllDisplayModes(display: u32, options: *const std::ffi::c_void) -> *const std::ffi::c_void;
    fn CGDisplaySetDisplayMode(display: u32, mode: *const std::ffi::c_void, options: *const std::ffi::c_void) -> i32;
    fn CGDisplayModeGetWidth(mode: *const std::ffi::c_void) -> u64;
    fn CGDisplayModeGetHeight(mode: *const std::ffi::c_void) -> u64;
    fn CGDisplayModeGetPixelWidth(mode: *const std::ffi::c_void) -> u64;
    fn CGDisplayModeGetPixelHeight(mode: *const std::ffi::c_void) -> u64;
    fn CGDisplayModeGetIOFlags(mode: *const std::ffi::c_void) -> u32;
    fn CFArrayGetCount(arr: *const std::ffi::c_void) -> i64;
    fn CFArrayGetValueAtIndex(arr: *const std::ffi::c_void, idx: i64) -> *const std::ffi::c_void;
    fn CFRelease(cf: *const std::ffi::c_void);
}

/// Get per-display scale factors on macOS via CoreGraphics.
/// Scale = pixel_width / logical_width (e.g., 3456/1728 = 2.0 = 200% Retina).
#[cfg(target_os = "macos")]
fn get_all_scales() -> Vec<ScaleInfo> {
    unsafe {
        let mut displays = [0u32; 10];
        let mut count: u32 = 0;
        CGGetActiveDisplayList(10, displays.as_mut_ptr(), &mut count);

        let mut scales = Vec::new();
        let mut ext_idx = 0u32;

        for i in 0..count as usize {
            let did = displays[i];
            let is_builtin = CGDisplayIsBuiltin(did) != 0;

            let id = if is_builtin {
                BUILTIN_ID.to_string()
            } else {
                ext_idx += 1;
                ext_idx.to_string()
            };

            // Get current mode to read physical pixel width and logical width
            let cur_mode = CGDisplayCopyDisplayMode(did);
            if cur_mode.is_null() { continue; }
            let pixel_w = CGDisplayModeGetPixelWidth(cur_mode);
            let logical_w = CGDisplayModeGetWidth(cur_mode);
            CFRelease(cur_mode);

            // Scale = pixel_width / logical_width (e.g., 3456/1728 = 2.0 = 200%)
            let scale = if logical_w > 0 {
                ((pixel_w as f64 / logical_w as f64) * 100.0).round() as u32
            } else {
                100
            };

            let name = if is_builtin {
                "Built-in Display".to_string()
            } else {
                // Try to get product name from ddc-macos enumeration
                format!("Display {}", ext_idx)
            };

            scales.push(ScaleInfo { id, name, scale_percent: scale });
        }
        scales
    }
}

/// Set display scale on macOS by switching to a different display mode.
/// Finds the mode whose logical width best matches the target scale percentage,
/// preferring HiDPI modes when available. This changes the effective resolution.
#[cfg(target_os = "macos")]
fn set_scale(id: &str, pct: u16) -> bool {
    unsafe {
        // Find the display ID
        let mut displays = [0u32; 10];
        let mut count: u32 = 0;
        CGGetActiveDisplayList(10, displays.as_mut_ptr(), &mut count);

        let mut ext_idx = 0u32;
        let mut target_did: Option<u32> = None;

        for i in 0..count as usize {
            let did = displays[i];
            let is_builtin = CGDisplayIsBuiltin(did) != 0;
            let display_id = if is_builtin {
                BUILTIN_ID.to_string()
            } else {
                ext_idx += 1;
                ext_idx.to_string()
            };
            if display_id == id || (id == "0" && is_builtin) {
                target_did = Some(did);
                break;
            }
        }

        let did = match target_did {
            Some(d) => d,
            None => return false,
        };

        // Get current mode's pixel width (physical resolution)
        let cur_mode = CGDisplayCopyDisplayMode(did);
        if cur_mode.is_null() { return false; }
        let pixel_w = CGDisplayModeGetPixelWidth(cur_mode);
        CFRelease(cur_mode);
        if pixel_w == 0 { return false; }

        // Target logical width = pixel_width / (pct/100)
        // e.g., 3456 pixels at 200% scale → 3456/2.0 = 1728 logical
        let target_w = (pixel_w as f64 / (pct as f64 / 100.0)).round() as u64;

        // Enumerate all modes and find the best match
        let modes = CGDisplayCopyAllDisplayModes(did, std::ptr::null());
        if modes.is_null() { return false; }

        let mode_count = CFArrayGetCount(modes);
        let mut best_mode: *const std::ffi::c_void = std::ptr::null();
        let mut best_diff = u64::MAX;
        let mut best_is_hidpi = false;

        // Prefer HiDPI modes (pixel_width > logical_width)
        let want_hidpi = pct >= 100;

        for j in 0..mode_count {
            let mode = CFArrayGetValueAtIndex(modes, j);
            let flags = CGDisplayModeGetIOFlags(mode);
            // kDisplayModeValidFlag (0x1) | kDisplayModeSafeFlag (0x2)
            if flags & 0x3 != 0x3 { continue; }

            let mode_logical_w = CGDisplayModeGetWidth(mode);
            let mode_pixel_w = CGDisplayModeGetPixelWidth(mode);
            let is_hidpi = mode_pixel_w > mode_logical_w;

            let diff = (mode_logical_w as i64 - target_w as i64).unsigned_abs();
            // Prefer HiDPI modes when available, then closest width
            let better = diff < best_diff
                || (diff == best_diff && is_hidpi && !best_is_hidpi && want_hidpi);
            if better {
                best_diff = diff;
                best_mode = mode;
                best_is_hidpi = is_hidpi;
            }
        }

        let result = if !best_mode.is_null() {
            let res = CGDisplaySetDisplayMode(did, best_mode, std::ptr::null());
            res == 0
        } else {
            false
        };

        CFRelease(modes);
        result
    }
}

// --- Windows: DPI scaling via registry + GetDpiForSystem ---

/// Get system-wide DPI scale on Windows.
/// Uses GetDpiForSystem() to read the current DPI (96 = 100%, 144 = 150%, etc.).
/// Windows DPI is system-wide, not per-monitor in this implementation.
#[cfg(target_os = "windows")]
fn get_all_scales() -> Vec<ScaleInfo> {
    // Read DPI settings from registry
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", r#"
            Get-CimInstance -Namespace root\wmi -ClassName WmiMonitorID -ErrorAction SilentlyContinue | ForEach-Object {
                $name = ($_.UserFriendlyName | Where-Object {$_ -ne 0} | ForEach-Object {[char]$_}) -join ''
                Write-Output "$name"
            }
        "#])
        .output();

    // Simpler approach: just read the system DPI
    let dpi_output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command",
            "Add-Type -TypeDefinition 'using System.Runtime.InteropServices; public class DPI { [DllImport(\"user32.dll\")] public static extern int GetDpiForSystem(); }'; [DPI]::GetDpiForSystem()"])
        .output();

    let system_dpi = dpi_output.ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u32>().ok())
        .unwrap_or(96);

    let scale = ((system_dpi as f64 / 96.0) * 100.0).round() as u32;

    vec![ScaleInfo {
        id: "system".into(),
        name: "System DPI".into(),
        scale_percent: scale,
    }]
}

/// Set DPI scale on Windows via registry. Requires logout to take effect.
/// Converts percentage to DPI value (100% = 96 DPI, 150% = 144 DPI).
/// Sets both LogPixels and Win8DpiScaling registry keys.
#[cfg(target_os = "windows")]
fn set_scale(_id: &str, pct: u16) -> bool {
    // Windows requires logout to fully apply DPI changes.
    // Set via registry for the current user.
    let dpi = ((pct as f64 / 100.0) * 96.0).round() as u32;
    let cmd = format!(
        "Set-ItemProperty -Path 'HKCU:\\Control Panel\\Desktop' -Name LogPixels -Value {} -Type DWord; \
         Set-ItemProperty -Path 'HKCU:\\Control Panel\\Desktop' -Name Win8DpiScaling -Value 1 -Type DWord",
        dpi
    );
    let ok = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if ok {
        eprintln!("(Logout required to apply DPI change on Windows)");
    }
    ok
}

// --- Linux: xrandr --scale (X11) or wlr-randr --scale (Wayland) ---

#[cfg(target_os = "linux")]
fn get_all_scales() -> Vec<ScaleInfo> {
    let display_server = detect_display_server_for_scale();
    match display_server {
        "x11" => get_scales_xrandr(),
        "wayland" => get_scales_wayland(),
        _ => vec![],
    }
}

#[cfg(target_os = "linux")]
fn detect_display_server_for_scale() -> &'static str {
    if let Ok(session) = std::env::var("XDG_SESSION_TYPE") {
        match session.to_lowercase().as_str() {
            "wayland" => return "wayland",
            "x11" => return "x11",
            _ => {}
        }
    }
    if std::env::var("WAYLAND_DISPLAY").is_ok() { return "wayland"; }
    if std::env::var("DISPLAY").is_ok() { return "x11"; }
    "unknown"
}

#[cfg(target_os = "linux")]
fn get_scales_xrandr() -> Vec<ScaleInfo> {
    let output = match std::process::Command::new("xrandr").arg("--query").output() {
        Ok(o) if o.status.success() => o,
        _ => return vec![],
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut scales = Vec::new();
    let mut idx = 0u32;

    for line in stdout.lines() {
        if line.contains(" connected") {
            let name = line.split_whitespace().next().unwrap_or("").to_string();
            let is_builtin = name.starts_with("eDP") || name.starts_with("LVDS");
            let id = if is_builtin {
                BUILTIN_ID.to_string()
            } else {
                idx += 1;
                idx.to_string()
            };
            // xrandr doesn't directly report scale, default to 100%
            scales.push(ScaleInfo { id, name, scale_percent: 100 });
        }
    }
    scales
}

#[cfg(target_os = "linux")]
fn get_scales_wayland() -> Vec<ScaleInfo> {
    // Try wlr-randr
    if let Ok(output) = std::process::Command::new("wlr-randr").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut scales = Vec::new();
            let mut idx = 0u32;
            let mut current_name = String::new();
            let mut current_scale = 100u32;

            for line in stdout.lines() {
                if !line.starts_with(' ') && !line.is_empty() {
                    // Emit previous if any
                    if !current_name.is_empty() {
                        let is_builtin = current_name.starts_with("eDP") || current_name.starts_with("LVDS");
                        let id = if is_builtin { BUILTIN_ID.to_string() } else { idx += 1; idx.to_string() };
                        scales.push(ScaleInfo { id, name: current_name.clone(), scale_percent: current_scale });
                    }
                    current_name = line.split_whitespace().next().unwrap_or("").to_string();
                    current_scale = 100;
                }
                let trimmed = line.trim();
                if trimmed.starts_with("Scale:") {
                    if let Some(val) = trimmed.strip_prefix("Scale:") {
                        current_scale = (val.trim().parse::<f64>().unwrap_or(1.0) * 100.0).round() as u32;
                    }
                }
            }
            // Last one
            if !current_name.is_empty() {
                let is_builtin = current_name.starts_with("eDP") || current_name.starts_with("LVDS");
                let id = if is_builtin { BUILTIN_ID.to_string() } else { idx += 1; idx.to_string() };
                scales.push(ScaleInfo { id, name: current_name, scale_percent: current_scale });
            }
            return scales;
        }
    }
    vec![]
}

#[cfg(target_os = "linux")]
fn set_scale(id: &str, pct: u16) -> bool {
    let display_server = detect_display_server_for_scale();
    let factor = format!("{:.2}", pct as f64 / 100.0);

    // Find the output name for the given ID
    let scales = get_all_scales();
    let output_name = scales.iter()
        .find(|s| s.id == id || s.name.to_lowercase() == id.to_lowercase() || (id == "0" && s.id == BUILTIN_ID))
        .map(|s| s.name.clone());

    let output_name = match output_name {
        Some(n) => n,
        None => return false,
    };

    match display_server {
        "x11" => {
            // xrandr uses inverse scale: 1.5x means things appear 1.5x larger,
            // so for 150% scaling we want --scale 0.67x0.67 (render more pixels).
            // But for simplicity and to match user expectations:
            // 100% = --scale 1x1, 200% = --scale 0.5x0.5 (things appear 2x bigger)
            let xrandr_scale = 100.0 / pct as f64;
            let scale_str = format!("{:.2}x{:.2}", xrandr_scale, xrandr_scale);
            std::process::Command::new("xrandr")
                .args(["--output", &output_name, "--scale", &scale_str])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
        "wayland" => {
            // wlr-randr uses direct scale: 1.5 means render at 1.5x density
            std::process::Command::new("wlr-randr")
                .args(["--output", &output_name, "--scale", &factor])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
        _ => false,
    }
}

// =========================================================================
// Keep-awake — prevent system idle sleep.
// macOS: caffeinate -di as a child process.
// Windows: SetThreadExecutionState via Win32 API.
// Linux: systemd-inhibit as a child process.
// =========================================================================

// State: child process handle (macOS/Linux) or bool flag (Windows).
// Static Mutex allows both CLI and server modes to track keep-awake state.
#[cfg(any(target_os = "macos", target_os = "linux"))]
static KEEP_AWAKE_CHILD: std::sync::Mutex<Option<std::process::Child>> = std::sync::Mutex::new(None);

#[cfg(target_os = "windows")]
static KEEP_AWAKE_ACTIVE: std::sync::Mutex<bool> = std::sync::Mutex::new(false);

// --- macOS: caffeinate ---

/// Enable keep-awake on macOS by spawning `caffeinate -di`.
/// -d = prevent display sleep, -i = prevent idle sleep.
/// The child process is held in KEEP_AWAKE_CHILD; killing it disables keep-awake.
#[cfg(target_os = "macos")]
fn enable_keep_awake() -> bool {
    let mut guard = KEEP_AWAKE_CHILD.lock().unwrap();
    if guard.is_some() { return true; } // already active
    match std::process::Command::new("caffeinate")
        .args(["-di"])
        .spawn()
    {
        Ok(child) => { *guard = Some(child); true }
        Err(_) => false,
    }
}

/// Disable keep-awake on macOS by killing the caffeinate child process.
#[cfg(target_os = "macos")]
fn disable_keep_awake() -> bool {
    let mut guard = KEEP_AWAKE_CHILD.lock().unwrap();
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
        return true;
    }
    // Fallback: kill external caffeinate processes (e.g., started by a prior CLI invocation)
    std::process::Command::new("pkill")
        .args(["-f", "caffeinate -di"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if keep-awake is active on macOS.
#[cfg(target_os = "macos")]
fn is_keep_awake_active() -> bool {
    if KEEP_AWAKE_CHILD.lock().unwrap().is_some() { return true; }
    // Check for external caffeinate process
    std::process::Command::new("pgrep")
        .args(["-f", "caffeinate -di"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// --- Windows: SetThreadExecutionState ---
// Direct FFI to kernel32.dll — avoids adding windows crate features for a single function.

#[cfg(target_os = "windows")]
extern "system" {
    fn SetThreadExecutionState(esflags: u32) -> u32;
}

#[cfg(target_os = "windows")]
const ES_CONTINUOUS: u32 = 0x80000000;
#[cfg(target_os = "windows")]
const ES_SYSTEM_REQUIRED: u32 = 0x00000001;
#[cfg(target_os = "windows")]
const ES_DISPLAY_REQUIRED: u32 = 0x00000002;

/// Enable keep-awake on Windows via SetThreadExecutionState.
/// Sets ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED to prevent
/// both system sleep and display sleep.
#[cfg(target_os = "windows")]
fn enable_keep_awake() -> bool {
    let ok = unsafe {
        SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED) != 0
    };
    if ok { *KEEP_AWAKE_ACTIVE.lock().unwrap() = true; }
    ok
}

/// Disable keep-awake on Windows by resetting to ES_CONTINUOUS only.
#[cfg(target_os = "windows")]
fn disable_keep_awake() -> bool {
    let ok = unsafe {
        SetThreadExecutionState(ES_CONTINUOUS) != 0
    };
    *KEEP_AWAKE_ACTIVE.lock().unwrap() = false;
    ok
}

/// Check if keep-awake is active on Windows.
#[cfg(target_os = "windows")]
fn is_keep_awake_active() -> bool {
    *KEEP_AWAKE_ACTIVE.lock().unwrap()
}

// --- Linux: systemd-inhibit ---

/// Enable keep-awake on Linux by spawning `systemd-inhibit` with an idle inhibitor.
/// Runs `sleep infinity` as the held command — the inhibitor lock is released when
/// the process is killed.
#[cfg(target_os = "linux")]
fn enable_keep_awake() -> bool {
    let mut guard = KEEP_AWAKE_CHILD.lock().unwrap();
    if guard.is_some() { return true; }
    match std::process::Command::new("systemd-inhibit")
        .args([
            "--what=idle",
            "--who=display-dj",
            "--why=Keep Awake",
            "--mode=block",
            "sleep", "infinity",
        ])
        .spawn()
    {
        Ok(child) => { *guard = Some(child); true }
        Err(_) => false,
    }
}

/// Disable keep-awake on Linux by killing the systemd-inhibit child process.
#[cfg(target_os = "linux")]
fn disable_keep_awake() -> bool {
    let mut guard = KEEP_AWAKE_CHILD.lock().unwrap();
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
        return true;
    }
    // Fallback: kill external systemd-inhibit processes started by display-dj
    std::process::Command::new("pkill")
        .args(["-f", "systemd-inhibit.*display-dj"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if keep-awake is active on Linux.
#[cfg(target_os = "linux")]
fn is_keep_awake_active() -> bool {
    if KEEP_AWAKE_CHILD.lock().unwrap().is_some() { return true; }
    std::process::Command::new("pgrep")
        .args(["-f", "systemd-inhibit.*display-dj"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// --- CLI handlers ---

/// CLI handler for `keep_awake_on` — enables keep-awake and blocks until Ctrl+C.
fn cmd_keep_awake_on() {
    if enable_keep_awake() {
        eprintln!("Keep-awake enabled. Press Ctrl+C to stop.");
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    } else {
        eprintln!("Failed to enable keep-awake.");
        std::process::exit(1);
    }
}

/// CLI handler for `keep_awake_off` — disables keep-awake.
fn cmd_keep_awake_off() {
    if disable_keep_awake() {
        eprintln!("Keep-awake disabled.");
    } else {
        eprintln!("Keep-awake was not active.");
    }
}

/// CLI handler for `get_keep_awake` — outputs {"enabled": true/false} to stdout.
fn cmd_get_keep_awake() {
    let enabled = is_keep_awake_active();
    println!(r#"{{"enabled":{}}}"#, enabled);
}

// --- HTTP handlers ---

/// HTTP handler for GET /keep_awake — returns {"enabled": true/false}.
fn serve_get_keep_awake() -> String {
    format!(r#"{{"enabled":{}}}"#, is_keep_awake_active())
}

/// HTTP handler for /keep_awake/enable — starts preventing sleep.
fn serve_keep_awake_enable() -> String {
    if enable_keep_awake() {
        r#"{"status":"ok","enabled":true}"#.to_string()
    } else {
        r#"{"error":"failed to enable keep-awake"}"#.to_string()
    }
}

/// HTTP handler for /keep_awake/disable — stops preventing sleep.
fn serve_keep_awake_disable() -> String {
    disable_keep_awake();
    r#"{"status":"ok","enabled":false}"#.to_string()
}

// =========================================================================
// Wallpaper — set/get desktop wallpaper with fit mode control.
// macOS: osascript (System Events). Windows: registry + SystemParametersInfoW.
// Linux: gsettings (GNOME), xfconf-query (XFCE), feh fallback.
// =========================================================================

/// Valid wallpaper fit/scaling modes. The CLI rejects unknown values.
const VALID_FITS: &[&str] = &["fill", "fit", "stretch", "center", "tile"];

/// Check if a fit mode string is valid.
fn validate_fit(fit: &str) -> bool {
    VALID_FITS.contains(&fit)
}

/// Current wallpaper state — path and fit mode. Returned by get_wallpaper and /get_wallpaper.
#[derive(Serialize)]
struct WallpaperInfo {
    path: Option<String>,
    fit: Option<String>,
}

// --- CLI handlers ---

/// CLI handler for `set_wallpaper <fit> <path>` — sets wallpaper with the given fit mode.
fn cmd_set_wallpaper(fit: &str, path: &str) {
    if !validate_fit(fit) {
        eprintln!("Invalid fit mode: \"{}\". Valid: fill, fit, stretch, center, tile", fit);
        std::process::exit(1);
    }
    if !std::path::Path::new(path).exists() {
        eprintln!("File not found: {}", path);
        std::process::exit(1);
    }
    if set_wallpaper(path, fit) {
        eprintln!("Wallpaper set to {} (fit: {}).", path, fit);
    } else {
        eprintln!("Failed to set wallpaper.");
        std::process::exit(1);
    }
}

/// CLI handler for `get_wallpaper` — outputs {"path": "...", "fit": "..."} to stdout.
fn cmd_get_wallpaper() {
    match get_wallpaper() {
        Some(info) => println!("{}", serde_json::to_string_pretty(&info).unwrap()),
        None => {
            let info = WallpaperInfo { path: None, fit: None };
            println!("{}", serde_json::to_string_pretty(&info).unwrap());
        }
    }
}

/// CLI handler for `get_wallpaper_supported` — outputs {"supported": true/false} to stdout.
fn cmd_get_wallpaper_supported() {
    println!(r#"{{"supported":{}}}"#, is_wallpaper_supported());
}

// --- HTTP handlers ---

/// HTTP handler for /set_wallpaper/<fit>/<path> — sets wallpaper on all monitors.
fn serve_set_wallpaper(fit: &str, path: &str) -> String {
    if !validate_fit(fit) {
        return format!(r#"{{"error":"invalid fit mode: '{}'. Valid: fill, fit, stretch, center, tile"}}"#, fit);
    }
    if !std::path::Path::new(path).exists() {
        return format!(r#"{{"error":"file not found: {}"}}"#, path);
    }
    if set_wallpaper(path, fit) {
        r#"{"ok":true}"#.to_string()
    } else {
        r#"{"error":"failed to set wallpaper"}"#.to_string()
    }
}

/// HTTP handler for /get_wallpaper — returns current wallpaper path and fit mode.
fn serve_get_wallpaper() -> String {
    match get_wallpaper() {
        Some(info) => serde_json::to_string(&info).unwrap_or_else(|_| r#"{"path":null,"fit":null}"#.into()),
        None => r#"{"path":null,"fit":null}"#.to_string(),
    }
}

/// HTTP handler for /get_wallpaper_supported — returns {"supported": true/false}.
fn serve_get_wallpaper_supported() -> String {
    format!(r#"{{"supported":{}}}"#, is_wallpaper_supported())
}

// --- Per-monitor wallpaper ---

/// CLI handler for `set_wallpaper_one <index> <fit> <path>` — sets wallpaper on one monitor.
fn cmd_set_wallpaper_one(index: usize, fit: &str, path: &str) {
    if !validate_fit(fit) {
        eprintln!("Invalid fit mode: \"{}\". Valid: fill, fit, stretch, center, tile", fit);
        std::process::exit(1);
    }
    if !std::path::Path::new(path).exists() {
        eprintln!("File not found: {}", path);
        std::process::exit(1);
    }
    match set_wallpaper_one(index, path, fit) {
        Ok(()) => eprintln!("Wallpaper set on monitor {} to {} (fit: {}).", index, path, fit),
        Err(msg) => {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    }
}

/// HTTP handler for /set_wallpaper_one/<index>/<fit>/<path>.
fn serve_set_wallpaper_one(index: usize, fit: &str, path: &str) -> String {
    if !validate_fit(fit) {
        return format!(r#"{{"error":"invalid fit mode: '{}'. Valid: fill, fit, stretch, center, tile"}}"#, fit);
    }
    if !std::path::Path::new(path).exists() {
        return format!(r#"{{"error":"file not found: {}"}}"#, path);
    }
    match set_wallpaper_one(index, path, fit) {
        Ok(()) => r#"{"ok":true}"#.to_string(),
        Err(msg) => format!(r#"{{"error":"{}"}}"#, msg),
    }
}

// --- macOS per-monitor: osascript with desktop index ---

/// Set wallpaper on a specific monitor on macOS.
/// AppleScript desktop indices are 1-based, so we add 1 to the 0-based index.
#[cfg(target_os = "macos")]
fn set_wallpaper_one(index: usize, path: &str, _fit: &str) -> Result<(), String> {
    let desktop_num = index + 1; // AppleScript uses 1-based indexing
    let script = format!(
        "tell application \"System Events\" to tell desktop {} to set picture to \"{}\"",
        desktop_num, path
    );
    let output = std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map_err(|e| format!("failed to run osascript: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("failed to set wallpaper on monitor {}: {}", index, stderr))
    }
}

// --- Windows per-monitor: IDesktopWallpaper COM via PowerShell ---

/// Set wallpaper on a specific monitor on Windows using IDesktopWallpaper COM interface.
/// Gets the monitor device path by index, then sets wallpaper + fit on that monitor.
#[cfg(target_os = "windows")]
fn set_wallpaper_one(index: usize, path: &str, fit: &str) -> Result<(), String> {
    let position = match fit {
        "fill" => "4",    // DWPOS_FILL
        "fit" => "3",     // DWPOS_FIT
        "stretch" => "2", // DWPOS_STRETCH
        "center" => "0",  // DWPOS_CENTER
        "tile" => "1",    // DWPOS_TILE
        _ => "4",
    };
    let escaped_path = path.replace('\'', "''");
    let cmd = format!(
        r#"$wp = New-Object -ComObject 'DesktopWallpaper'
$count = $wp.GetMonitorDevicePathCount()
if ({idx} -ge $count) {{ Write-Error "monitor index {idx} out of range (0..$($count-1))"; exit 1 }}
$id = $wp.GetMonitorDevicePathAt({idx})
$wp.SetWallpaper($id, '{path}')
$wp.SetPosition({pos})
"#,
        idx = index, path = escaped_path, pos = position
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
        .output()
        .map_err(|e| format!("failed to run powershell: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("failed to set wallpaper on monitor {}: {}", index, stderr))
    }
}

// --- Linux per-monitor: not natively supported ---

/// Per-monitor wallpaper is not supported on Linux GNOME.
#[cfg(target_os = "linux")]
fn set_wallpaper_one(_index: usize, _path: &str, _fit: &str) -> Result<(), String> {
    Err("per-monitor wallpaper not supported on this platform".to_string())
}

// --- macOS: osascript via System Events ---

/// Set wallpaper on macOS using System Events AppleScript.
/// Sets the desktop picture on all desktops/spaces.
#[cfg(target_os = "macos")]
fn set_wallpaper(path: &str, _fit: &str) -> bool {
    // System Events sets the picture on all desktops
    let script = format!(
        "tell application \"System Events\" to tell every desktop to set picture to \"{}\"",
        path
    );
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get current wallpaper path on macOS via System Events.
#[cfg(target_os = "macos")]
fn get_wallpaper() -> Option<WallpaperInfo> {
    let output = std::process::Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to get picture of desktop 1"])
        .output().ok()?;
    if !output.status.success() { return None; }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() { return None; }
    Some(WallpaperInfo { path: Some(path), fit: Some("fill".into()) })
}

/// macOS always supports wallpaper operations.
#[cfg(target_os = "macos")]
fn is_wallpaper_supported() -> bool { true }

// --- Windows: registry + SystemParametersInfoW via PowerShell ---

/// Set wallpaper on Windows. Sets fit mode via registry keys, then applies
/// the wallpaper using SystemParametersInfoW P/Invoke through PowerShell.
#[cfg(target_os = "windows")]
fn set_wallpaper(path: &str, fit: &str) -> bool {
    // Set fit mode via registry: WallpaperStyle + TileWallpaper
    let (style, tile) = match fit {
        "fill" => ("10", "0"),
        "fit" => ("6", "0"),
        "stretch" => ("2", "0"),
        "center" => ("0", "0"),
        "tile" => ("0", "1"),
        _ => ("10", "0"), // default to fill
    };
    let _ = std::process::Command::new("reg")
        .args(["add", r"HKCU\Control Panel\Desktop", "/v", "WallpaperStyle", "/t", "REG_SZ", "/d", style, "/f"])
        .output();
    let _ = std::process::Command::new("reg")
        .args(["add", r"HKCU\Control Panel\Desktop", "/v", "TileWallpaper", "/t", "REG_SZ", "/d", tile, "/f"])
        .output();

    // Set wallpaper via SystemParametersInfoW (SPI_SETDESKWALLPAPER = 0x0014)
    let escaped_path = path.replace('\'', "''");
    let cmd = format!(
        r#"Add-Type -TypeDefinition @'
using System.Runtime.InteropServices;
public class Wallpaper {{
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern int SystemParametersInfo(int uAction, int uParam, string lpvParam, int fuWinIni);
}}
'@
[Wallpaper]::SystemParametersInfo(0x0014, 0, '{}', 0x01 -bor 0x02)
"#,
        escaped_path
    );
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get current wallpaper on Windows by reading registry keys.
#[cfg(target_os = "windows")]
fn get_wallpaper() -> Option<WallpaperInfo> {
    let output = std::process::Command::new("reg")
        .args(["query", r"HKCU\Control Panel\Desktop", "/v", "Wallpaper"])
        .output().ok()?;
    if !output.status.success() { return None; }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout.lines()
        .find(|l| l.contains("Wallpaper"))
        .and_then(|l| l.split("REG_SZ").nth(1))
        .map(|s| s.trim().to_string())?;
    if path.is_empty() { return None; }

    // Read fit mode from WallpaperStyle + TileWallpaper
    let style_val = std::process::Command::new("reg")
        .args(["query", r"HKCU\Control Panel\Desktop", "/v", "WallpaperStyle"])
        .output().ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout).lines()
                .find(|l| l.contains("WallpaperStyle"))
                .and_then(|l| l.split("REG_SZ").nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_default();
    let tile_val = std::process::Command::new("reg")
        .args(["query", r"HKCU\Control Panel\Desktop", "/v", "TileWallpaper"])
        .output().ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout).lines()
                .find(|l| l.contains("TileWallpaper"))
                .and_then(|l| l.split("REG_SZ").nth(1))
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_default();

    let fit = match (style_val.as_str(), tile_val.as_str()) {
        ("10", "0") => "fill",
        ("6", "0") => "fit",
        ("2", "0") => "stretch",
        ("0", "1") => "tile",
        ("0", "0") => "center",
        _ => "fill",
    };
    Some(WallpaperInfo { path: Some(path), fit: Some(fit.into()) })
}

/// Windows always supports wallpaper operations.
#[cfg(target_os = "windows")]
fn is_wallpaper_supported() -> bool { true }

// --- Linux: gsettings (GNOME), xfconf-query (XFCE), feh fallback ---

/// Set wallpaper on Linux. Tries GNOME (gsettings), XFCE (xfconf-query), and feh in order.
#[cfg(target_os = "linux")]
fn set_wallpaper(path: &str, fit: &str) -> bool {
    let gnome_mode = match fit {
        "fill" => "zoom",
        "fit" => "scaled",
        "stretch" => "stretched",
        "center" => "centered",
        "tile" => "wallpaper",
        _ => "zoom",
    };

    // Try GNOME (gsettings)
    let mode_ok = std::process::Command::new("gsettings")
        .args(["set", "org.gnome.desktop.background", "picture-options", gnome_mode])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if mode_ok {
        let uri = format!("file://{}", path);
        let set_ok = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.desktop.background", "picture-uri", &uri])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        // Also set picture-uri-dark for GNOME 42+ dark mode wallpaper
        let _ = std::process::Command::new("gsettings")
            .args(["set", "org.gnome.desktop.background", "picture-uri-dark", &uri])
            .output();
        if set_ok { return true; }
    }

    // Try XFCE (xfconf-query)
    if std::process::Command::new("xfconf-query")
        .args(["-c", "xfce4-desktop", "-p", "/backdrop/screen0/monitor0/workspace0/last-image", "-s", path])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return true;
    }

    // Fallback: feh
    let feh_mode = match fit {
        "fill" => "--bg-fill",
        "fit" => "--bg-max",
        "stretch" => "--bg-scale",
        "center" => "--bg-center",
        "tile" => "--bg-tile",
        _ => "--bg-fill",
    };
    std::process::Command::new("feh")
        .args([feh_mode, path])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Get current wallpaper on Linux via gsettings (GNOME).
#[cfg(target_os = "linux")]
fn get_wallpaper() -> Option<WallpaperInfo> {
    // Try GNOME
    let uri_output = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.background", "picture-uri"])
        .output().ok()?;
    if uri_output.status.success() {
        let uri = String::from_utf8_lossy(&uri_output.stdout).trim()
            .trim_matches('\'').to_string();
        let path = uri.strip_prefix("file://").unwrap_or(&uri).to_string();
        if !path.is_empty() {
            let gnome_mode = std::process::Command::new("gsettings")
                .args(["get", "org.gnome.desktop.background", "picture-options"])
                .output().ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().trim_matches('\'').to_string())
                .unwrap_or_default();
            let fit = match gnome_mode.as_str() {
                "zoom" => "fill",
                "scaled" => "fit",
                "stretched" => "stretch",
                "centered" => "center",
                "wallpaper" => "tile",
                _ => "fill",
            };
            return Some(WallpaperInfo { path: Some(path), fit: Some(fit.into()) });
        }
    }
    None
}

/// Check if wallpaper operations are supported on this Linux session.
/// Returns true if any supported DE/tool is available.
#[cfg(target_os = "linux")]
fn is_wallpaper_supported() -> bool {
    // GNOME
    if std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.background", "picture-uri"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    { return true; }
    // XFCE
    if std::process::Command::new("xfconf-query")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    { return true; }
    // feh
    std::process::Command::new("feh")
        .args(["--version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// =========================================================================
// Tests — everything that can be tested without a physical display
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_display(id: &str, name: &str, dtype: &str, ddc: bool) -> DisplayInfo {
        DisplayInfo {
            id: id.into(),
            name: name.into(),
            display_type: dtype.into(),
            brightness: Some(50),
            contrast: Some(70),
            ddc_supported: ddc,
        }
    }

    // --- matches_display ---

    #[test]
    fn match_by_id() {
        let d = make_display("1", "Dell U2723QE", "external", true);
        assert!(matches_display(&d, "1"));
        assert!(!matches_display(&d, "2"));
    }

    #[test]
    fn match_by_name_exact() {
        let d = make_display("1", "Dell U2723QE", "external", true);
        assert!(matches_display(&d, "Dell U2723QE"));
    }

    #[test]
    fn match_by_name_case_insensitive() {
        let d = make_display("1", "XZ322QU V3", "external", false);
        assert!(matches_display(&d, "xz322qu v3"));
        assert!(matches_display(&d, "XZ322QU V3"));
        assert!(matches_display(&d, "Xz322qu V3"));
    }

    #[test]
    fn match_builtin_by_zero() {
        let d = make_display("builtin", "Built-in Display", "builtin", false);
        assert!(matches_display(&d, "0"));
        assert!(matches_display(&d, "builtin"));
    }

    #[test]
    fn zero_does_not_match_external() {
        let d = make_display("1", "External", "external", true);
        assert!(!matches_display(&d, "0"));
    }

    #[test]
    fn no_match() {
        let d = make_display("1", "Dell", "external", true);
        assert!(!matches_display(&d, "99"));
        assert!(!matches_display(&d, "nonexistent"));
    }

    // --- DisplayInfo serialization ---

    #[test]
    fn display_info_serializes_to_json() {
        let d = make_display("builtin", "Built-in Display", "builtin", false);
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("\"id\":\"builtin\""));
        assert!(json.contains("\"name\":\"Built-in Display\""));
        assert!(json.contains("\"display_type\":\"builtin\""));
        assert!(json.contains("\"brightness\":50"));
        assert!(json.contains("\"ddc_supported\":false"));
    }

    #[test]
    fn display_info_null_brightness() {
        let d = DisplayInfo {
            id: "1".into(),
            name: "Test".into(),
            display_type: "external".into(),
            brightness: None,
            contrast: None,
            ddc_supported: false,
        };
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("\"brightness\":null"));
        assert!(json.contains("\"contrast\":null"));
    }

    #[test]
    fn display_info_array_serializes() {
        let displays = vec![
            make_display("builtin", "Built-in", "builtin", false),
            make_display("1", "External 1", "external", true),
        ];
        let json = serde_json::to_string(&displays).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 2);
    }

    // --- Constants ---

    #[test]
    fn builtin_id_is_builtin() {
        assert_eq!(BUILTIN_ID, "builtin");
    }

    #[test]
    fn vcp_codes_are_standard() {
        assert_eq!(VCP_BRIGHTNESS, 0x10);
        assert_eq!(VCP_CONTRAST, 0x12);
    }

    // --- Version ---

    #[test]
    fn version_is_set() {
        let version = env!("CARGO_PKG_VERSION");
        assert!(!version.is_empty());
        // Should be semver-ish
        assert!(version.contains('.'));
    }

    // --- Mock platform for integration tests ---

    struct MockControl {
        brightness: u32,
        contrast: u32,
    }

    impl DisplayControl for MockControl {
        fn get_brightness(&mut self) -> Option<u32> { Some(self.brightness) }
        fn get_contrast(&mut self) -> Option<u32> { Some(self.contrast) }
        fn set_brightness(&mut self, value: u16, _mode: &str) -> bool {
            self.brightness = value as u32;
            true
        }
        fn set_contrast(&mut self, value: u16) -> bool {
            self.contrast = value as u32;
            true
        }
        fn reset_gamma(&self) {}
    }

    struct MockPlatform;

    impl Platform for MockPlatform {
        fn enumerate() -> Vec<(DisplayInfo, Box<dyn DisplayControl>)> {
            vec![
                (
                    make_display("builtin", "Built-in Display", "builtin", false),
                    Box::new(MockControl { brightness: 80, contrast: 50 }),
                ),
                (
                    make_display("1", "XZ322QU V3", "external", false),
                    Box::new(MockControl { brightness: 50, contrast: 50 }),
                ),
                (
                    make_display("2", "VX2718-2KPC", "external", true),
                    Box::new(MockControl { brightness: 70, contrast: 60 }),
                ),
            ]
        }
        fn reset_all_gamma() {}
        fn debug_info() -> serde_json::Value {
            serde_json::json!({"mock": true})
        }
    }

    #[test]
    fn mock_enumerate_returns_three_displays() {
        let displays = MockPlatform::enumerate();
        assert_eq!(displays.len(), 3);
        assert_eq!(displays[0].0.id, "builtin");
        assert_eq!(displays[1].0.id, "1");
        assert_eq!(displays[2].0.id, "2");
    }

    #[test]
    fn mock_set_brightness_updates_value() {
        let mut ctrl = MockControl { brightness: 50, contrast: 50 };
        assert!(ctrl.set_brightness(80, "force"));
        assert_eq!(ctrl.get_brightness(), Some(80));
    }

    #[test]
    fn mock_set_contrast_updates_value() {
        let mut ctrl = MockControl { brightness: 50, contrast: 50 };
        assert!(ctrl.set_contrast(30));
        assert_eq!(ctrl.get_contrast(), Some(30));
    }

    // --- serve_list / serve_get / serve_set_all / serve_set_one with mock ---

    #[test]
    fn serve_list_returns_json_array() {
        let json = serve_list::<MockPlatform>();
        let parsed: Vec<DisplayInfo> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].id, "builtin");
    }

    #[test]
    fn serve_get_all_returns_live_values() {
        let json = serve_get::<MockPlatform>(None);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 3);
        // MockControl returns its stored brightness
        assert_eq!(parsed[0]["brightness"], 80);
        assert_eq!(parsed[2]["brightness"], 70);
    }

    #[test]
    fn serve_get_one_by_id() {
        let json = serve_get::<MockPlatform>(Some("2"));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "2");
        assert_eq!(parsed["name"], "VX2718-2KPC");
    }

    #[test]
    fn serve_get_one_by_name() {
        let json = serve_get::<MockPlatform>(Some("xz322qu v3"));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "1");
    }

    #[test]
    fn serve_get_one_builtin_by_zero() {
        let json = serve_get::<MockPlatform>(Some("0"));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "builtin");
    }

    #[test]
    fn serve_get_one_not_found() {
        let json = serve_get::<MockPlatform>(Some("99"));
        assert!(json.contains("error"));
        assert!(json.contains("not found"));
    }

    #[test]
    fn serve_set_all_returns_status_per_display() {
        let json = serve_set_all::<MockPlatform>(50, "force");
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 3);
        for item in &parsed {
            assert_eq!(item["status"], "ok");
        }
    }

    #[test]
    fn serve_set_one_by_id() {
        let json = serve_set_one::<MockPlatform>("2", 30, "force");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "2");
        assert_eq!(parsed["status"], "ok");
    }

    #[test]
    fn serve_set_one_not_found() {
        let json = serve_set_one::<MockPlatform>("99", 50, "force");
        assert!(json.contains("error"));
        assert!(json.contains("not found"));
    }

    // --- Volume ---

    #[test]
    fn volume_info_serializes() {
        let info = VolumeInfo { volume: 75, muted: false };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"volume\":75"));
        assert!(json.contains("\"muted\":false"));
    }

    #[test]
    fn volume_info_muted_serializes() {
        let info = VolumeInfo { volume: 0, muted: true };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"volume\":0"));
        assert!(json.contains("\"muted\":true"));
    }

    #[test]
    fn serve_get_volume_returns_json() {
        // get_volume() calls OS APIs — may return None in CI.
        // Just verify serve_get_volume returns valid JSON either way.
        let json = serve_get_volume();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("volume").is_some() || parsed.get("error").is_some());
    }

    #[test]
    fn serve_set_volume_returns_json() {
        // set_volume() calls OS APIs — may fail in CI.
        let json = serve_set_volume(50);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("status").is_some() || parsed.get("error").is_some());
    }

    // --- Scale ---

    #[test]
    fn clamp_scale_within_range() {
        assert_eq!(clamp_scale(100), 100);
        assert_eq!(clamp_scale(150), 150);
        assert_eq!(clamp_scale(75), 75);
        assert_eq!(clamp_scale(300), 300);
    }

    #[test]
    fn clamp_scale_below_min() {
        assert_eq!(clamp_scale(50), SCALE_MIN);
        assert_eq!(clamp_scale(0), SCALE_MIN);
    }

    #[test]
    fn clamp_scale_above_max() {
        assert_eq!(clamp_scale(400), SCALE_MAX);
        assert_eq!(clamp_scale(500), SCALE_MAX);
    }

    #[test]
    fn scale_info_serializes() {
        let info = ScaleInfo { id: "1".into(), name: "Test".into(), scale_percent: 150 };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"scale_percent\":150"));
        assert!(json.contains("\"id\":\"1\""));
    }

    #[test]
    fn scale_constants() {
        assert_eq!(SCALE_MIN, 75);
        assert_eq!(SCALE_MAX, 300);
    }

    #[test]
    fn serve_get_scale_returns_json() {
        let json = serve_get_scale();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
    }

    #[test]
    fn serve_set_scale_all_returns_json() {
        // Calls OS APIs — may fail, just verify valid JSON response
        let json = serve_set_scale_all(100);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
    }

    #[test]
    fn serve_set_scale_one_not_found() {
        let json = serve_set_scale_one("99", 100);
        assert!(json.contains("error"));
        assert!(json.contains("not found"));
    }

    // --- serve_set_one by name ---

    #[test]
    fn serve_set_one_by_name() {
        let json = serve_set_one::<MockPlatform>("VX2718-2KPC", 30, "force");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "2");
        assert_eq!(parsed["status"], "ok");
    }

    #[test]
    fn serve_set_one_by_name_case_insensitive() {
        let json = serve_set_one::<MockPlatform>("vx2718-2kpc", 30, "force");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "2");
    }

    #[test]
    fn serve_set_one_builtin_by_zero() {
        let json = serve_set_one::<MockPlatform>("0", 50, "force");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "builtin");
        assert_eq!(parsed["status"], "ok");
    }

    // --- serve_set_all with different modes ---

    #[test]
    fn serve_set_all_mode_auto() {
        let json = serve_set_all::<MockPlatform>(50, "auto");
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 3);
        for item in &parsed {
            assert_eq!(item["status"], "ok");
        }
    }

    #[test]
    fn serve_set_all_mode_ddc() {
        let json = serve_set_all::<MockPlatform>(50, "ddc");
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 3);
    }

    // --- DisplayInfo roundtrip serialization ---

    #[test]
    fn display_info_roundtrip() {
        let original = make_display("2", "VX2718-2KPC", "external", true);
        let json = serde_json::to_string(&original).unwrap();
        let restored: DisplayInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, "2");
        assert_eq!(restored.name, "VX2718-2KPC");
        assert_eq!(restored.display_type, "external");
        assert_eq!(restored.brightness, Some(50));
        assert_eq!(restored.contrast, Some(70));
        assert_eq!(restored.ddc_supported, true);
    }

    #[test]
    fn display_info_roundtrip_nulls() {
        let original = DisplayInfo {
            id: "1".into(),
            name: "Test".into(),
            display_type: "external".into(),
            brightness: None,
            contrast: None,
            ddc_supported: false,
        };
        let json = serde_json::to_string(&original).unwrap();
        let restored: DisplayInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.brightness, None);
        assert_eq!(restored.contrast, None);
    }

    // --- ScaleInfo ---

    #[test]
    fn scale_info_builtin() {
        let info = ScaleInfo { id: BUILTIN_ID.into(), name: "Built-in Display".into(), scale_percent: 200 };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"id\":\"builtin\""));
        assert!(json.contains("\"scale_percent\":200"));
    }

    #[test]
    fn scale_info_array_serializes() {
        let scales = vec![
            ScaleInfo { id: BUILTIN_ID.into(), name: "Built-in".into(), scale_percent: 200 },
            ScaleInfo { id: "1".into(), name: "External".into(), scale_percent: 100 },
        ];
        let json = serde_json::to_string(&scales).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 2);
    }

    // --- clamp_scale edge cases ---

    #[test]
    fn clamp_scale_at_boundary_minus_one() {
        assert_eq!(clamp_scale(74), SCALE_MIN);
        assert_eq!(clamp_scale(301), SCALE_MAX);
    }

    #[test]
    fn clamp_scale_u16_max() {
        assert_eq!(clamp_scale(u16::MAX), SCALE_MAX);
    }

    // --- VolumeInfo ---

    #[test]
    fn volume_info_boundary_values() {
        let v0 = VolumeInfo { volume: 0, muted: false };
        let v100 = VolumeInfo { volume: 100, muted: false };
        let json0 = serde_json::to_string(&v0).unwrap();
        let json100 = serde_json::to_string(&v100).unwrap();
        assert!(json0.contains("\"volume\":0"));
        assert!(json100.contains("\"volume\":100"));
    }

    // --- Keep-awake ---

    #[test]
    fn serve_get_keep_awake_returns_json() {
        let json = serve_get_keep_awake();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("enabled").is_some());
        assert!(parsed["enabled"].is_boolean());
    }

    #[test]
    fn serve_keep_awake_enable_returns_json() {
        // enable_keep_awake() calls OS APIs — may fail in CI.
        let json = serve_keep_awake_enable();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("status").is_some() || parsed.get("error").is_some());
        // Clean up if it succeeded
        if parsed.get("status").is_some() {
            disable_keep_awake();
        }
    }

    #[test]
    fn serve_keep_awake_disable_returns_json() {
        let json = serve_keep_awake_disable();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert_eq!(parsed["enabled"], false);
    }

    #[test]
    fn keep_awake_enable_disable_roundtrip() {
        // Enable — may fail in CI but should return valid result
        let enabled = enable_keep_awake();
        if enabled {
            assert!(is_keep_awake_active());
            assert!(disable_keep_awake());
        }
    }

    // --- MockControl edge cases ---

    #[test]
    fn mock_set_brightness_zero() {
        let mut ctrl = MockControl { brightness: 50, contrast: 50 };
        assert!(ctrl.set_brightness(0, "force"));
        assert_eq!(ctrl.get_brightness(), Some(0));
    }

    #[test]
    fn mock_set_brightness_max() {
        let mut ctrl = MockControl { brightness: 50, contrast: 50 };
        assert!(ctrl.set_brightness(100, "force"));
        assert_eq!(ctrl.get_brightness(), Some(100));
    }

    #[test]
    fn mock_reset_gamma_does_not_panic() {
        let ctrl = MockControl { brightness: 50, contrast: 50 };
        ctrl.reset_gamma(); // should be a no-op
    }

    // --- serve_get with all three mock displays ---

    #[test]
    fn serve_get_all_includes_contrast() {
        let json = serve_get::<MockPlatform>(None);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["contrast"], 50);
        assert_eq!(parsed[2]["contrast"], 60);
    }

    #[test]
    fn serve_get_one_by_builtin_string() {
        let json = serve_get::<MockPlatform>(Some("builtin"));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "builtin");
        assert_eq!(parsed["brightness"], 80);
    }

    // --- Contrast ---

    #[test]
    fn serve_set_contrast_all_returns_status_per_display() {
        let json = serve_set_contrast_all::<MockPlatform>(50);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 3);
        for item in &parsed {
            assert_eq!(item["status"], "ok");
        }
    }

    #[test]
    fn serve_set_contrast_one_by_id() {
        let json = serve_set_contrast_one::<MockPlatform>("2", 30);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "2");
        assert_eq!(parsed["status"], "ok");
    }

    #[test]
    fn serve_set_contrast_one_by_name() {
        let json = serve_set_contrast_one::<MockPlatform>("VX2718-2KPC", 70);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "2");
        assert_eq!(parsed["status"], "ok");
    }

    #[test]
    fn serve_set_contrast_one_not_found() {
        let json = serve_set_contrast_one::<MockPlatform>("99", 50);
        assert!(json.contains("error"));
        assert!(json.contains("not found"));
    }

    #[test]
    fn serve_set_contrast_one_builtin_by_zero() {
        let json = serve_set_contrast_one::<MockPlatform>("0", 50);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["id"], "builtin");
        assert_eq!(parsed["status"], "ok");
    }

    // --- Health ---

    #[test]
    fn serve_health_returns_status_pid_uptime() {
        let started = std::time::Instant::now();
        let json = serve_health(&started);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["status"], "ok");
        assert!(parsed["pid"].is_u64());
        assert!(parsed["uptime"].is_u64());
    }

    // --- Wallpaper ---

    #[test]
    fn validate_fit_valid_modes() {
        assert!(validate_fit("fill"));
        assert!(validate_fit("fit"));
        assert!(validate_fit("stretch"));
        assert!(validate_fit("center"));
        assert!(validate_fit("tile"));
    }

    #[test]
    fn validate_fit_invalid_modes() {
        assert!(!validate_fit(""));
        assert!(!validate_fit("zoom"));
        assert!(!validate_fit("FILL"));
        assert!(!validate_fit("unknown"));
    }

    #[test]
    fn valid_fits_constant_has_five_modes() {
        assert_eq!(VALID_FITS.len(), 5);
    }

    #[test]
    fn wallpaper_info_serializes() {
        let info = WallpaperInfo { path: Some("/path/to/image.jpg".into()), fit: Some("fill".into()) };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"path\":\"/path/to/image.jpg\""));
        assert!(json.contains("\"fit\":\"fill\""));
    }

    #[test]
    fn wallpaper_info_null_fields() {
        let info = WallpaperInfo { path: None, fit: None };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"path\":null"));
        assert!(json.contains("\"fit\":null"));
    }

    #[test]
    fn serve_get_wallpaper_returns_valid_json() {
        let json = serve_get_wallpaper();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("path").is_some());
        assert!(parsed.get("fit").is_some());
    }

    #[test]
    fn serve_get_wallpaper_supported_returns_valid_json() {
        let json = serve_get_wallpaper_supported();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("supported").is_some());
        assert!(parsed["supported"].is_boolean());
    }

    #[test]
    fn serve_set_wallpaper_invalid_fit() {
        let json = serve_set_wallpaper("zoom", "/some/path.jpg");
        assert!(json.contains("error"));
        assert!(json.contains("invalid fit mode"));
    }

    #[test]
    fn serve_set_wallpaper_missing_file() {
        let json = serve_set_wallpaper("fill", "/nonexistent/path/image.jpg");
        assert!(json.contains("error"));
        assert!(json.contains("file not found"));
    }

    #[test]
    fn serve_set_wallpaper_invalid_fit_all_variants() {
        for bad_fit in &["", "FILL", "Fit", "unknown", "scale", "crop"] {
            let json = serve_set_wallpaper(bad_fit, "/some/path.jpg");
            assert!(json.contains("error"), "expected error for fit mode: {}", bad_fit);
        }
    }

    // --- Per-monitor wallpaper ---

    #[test]
    fn serve_set_wallpaper_one_invalid_fit() {
        let json = serve_set_wallpaper_one(0, "zoom", "/some/path.jpg");
        assert!(json.contains("error"));
        assert!(json.contains("invalid fit mode"));
    }

    #[test]
    fn serve_set_wallpaper_one_missing_file() {
        let json = serve_set_wallpaper_one(0, "fill", "/nonexistent/path/image.jpg");
        assert!(json.contains("error"));
        assert!(json.contains("file not found"));
    }

    #[test]
    fn serve_set_wallpaper_one_invalid_fit_all_variants() {
        for bad_fit in &["", "FILL", "unknown", "scale"] {
            let json = serve_set_wallpaper_one(0, bad_fit, "/some/path.jpg");
            assert!(json.contains("error"), "expected error for fit mode: {}", bad_fit);
        }
    }
}
