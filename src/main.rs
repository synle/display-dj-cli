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
}

// =========================================================================
// CLI — all human-readable output goes to stderr, JSON goes to stdout
// =========================================================================

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

// Option<&String> = "maybe a reference to a String" — None means get all displays.
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

fn cmd_list<P: Platform>() {
    let displays = P::enumerate();
    // .into_iter() consumes the Vec (moves ownership — displays is gone after this).
    // |(info, _)| = destructure tuple, _ discards the DisplayControl we don't need.
    // .collect() gathers the lazy iterator into a concrete Vec.
    let infos: Vec<DisplayInfo> = displays.into_iter().map(|(info, _)| info).collect();
    println!("{}", serde_json::to_string_pretty(&infos).unwrap());
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

fn cmd_serve<P: Platform>(port: u16) {
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("Failed to bind to {}: {}", addr, e);
        std::process::exit(1);
    });
    eprintln!("display-dj server listening on http://{}", addr);
    eprintln!();
    eprintln!("Routes:  /list  /get_all  /get_one/<id>  /set_all/<level>  /set_all/<level>/<mode>");
    eprintln!("         /set_one/<id>/<level>  /set_one/<id>/<level>/<mode>");
    eprintln!("         /dark  /light  /theme  /reset  /health");
    eprintln!("         /get_volume  /set_volume/<level>  /mute  /unmute");
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
                    "/dark", "/light", "/theme",
                    "/get_volume", "/set_volume/<level>", "/mute", "/unmute",
                    "/reset", "/health"
                ]
            })).unwrap(),
            "health" => r#"{"status":"ok"}"#.to_string(),
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
            _ => {
                let _ = write_http(&mut stream, 404, r#"{"error":"not found"}"#);
                continue;
            }
        };

        let status = if json.contains("\"error\"") { 400 } else { 200 };
        let _ = write_http(&mut stream, status, &json);
    }
}

fn write_http(stream: &mut std::net::TcpStream, status: u16, body: &str) -> std::io::Result<()> {
    use std::io::Write;
    let reason = match status { 200 => "OK", 400 => "Bad Request", 404 => "Not Found", _ => "Error" };
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status, reason, body.len(), body
    )
}

fn serve_list<P: Platform>() -> String {
    let displays = P::enumerate();
    let infos: Vec<DisplayInfo> = displays.into_iter().map(|(info, _)| info).collect();
    serde_json::to_string(&infos).unwrap_or_else(|_| "[]".into())
}

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

// =========================================================================
// Dark / Light mode — platform-specific implementations via #[cfg].
// Unlike display control (which uses traits + platform modules), dark mode
// is simple enough to live in main.rs behind conditional compilation.
// =========================================================================

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

// --- macOS: AppleScript via osascript ---

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

// --- Windows: registry keys control app and system theme ---

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
    app && sys // both must succeed
}

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

// --- Linux: tries desktop environments in order (GNOME -> KDE -> XFCE) ---

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

#[derive(Serialize)]
struct VolumeInfo {
    volume: u32,
    muted: bool,
}

fn cmd_get_volume() {
    match get_volume() {
        Some(info) => println!("{}", serde_json::to_string_pretty(&info).unwrap()),
        None => {
            eprintln!("Could not read volume.");
            std::process::exit(1);
        }
    }
}

fn cmd_set_volume(level: u16) {
    if set_volume(level) {
        eprintln!("Volume set to {}%.", level);
    } else {
        eprintln!("Failed to set volume.");
        std::process::exit(1);
    }
}

fn cmd_set_mute(mute: bool) {
    if set_mute(mute) {
        eprintln!("Audio {}.", if mute { "muted" } else { "unmuted" });
    } else {
        eprintln!("Failed to {} audio.", if mute { "mute" } else { "unmute" });
        std::process::exit(1);
    }
}

fn serve_get_volume() -> String {
    match get_volume() {
        Some(info) => serde_json::to_string(&info).unwrap_or_else(|_| "{}".into()),
        None => r#"{"error":"could not read volume"}"#.to_string(),
    }
}

fn serve_set_volume(level: u16) -> String {
    if set_volume(level) {
        format!(r#"{{"status":"ok","volume":{}}}"#, level)
    } else {
        r#"{"error":"failed to set volume"}"#.to_string()
    }
}

// --- macOS: osascript ---

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

#[cfg(target_os = "macos")]
fn set_volume(level: u16) -> bool {
    std::process::Command::new("osascript")
        .args(["-e", &format!("set volume output volume {}", level)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn set_mute(mute: bool) -> bool {
    let val = if mute { "true" } else { "false" };
    std::process::Command::new("osascript")
        .args(["-e", &format!("set volume output muted {}", val)])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// --- Windows: PowerShell + COM audio ---

#[cfg(target_os = "windows")]
fn get_volume() -> Option<VolumeInfo> {
    // Use PowerShell with audio COM objects
    let ps = r#"
        Add-Type -TypeDefinition @'
        using System.Runtime.InteropServices;
        [Guid("5CDF2C82-841E-4546-9722-0CF74078229A"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IAudioEndpointVolume {
            int _0(); int _1(); int _2(); int _3(); int _4(); int _5(); int _6(); int _7(); int _8(); int _9(); int _10(); int _11();
            int GetMasterVolumeLevelScalar(out float level);
            int SetMasterVolumeLevelScalar(float level, System.Guid ctx);
            int GetMute(out bool mute);
        }
        [Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IMMDevice { int Activate(ref System.Guid id, int ctx, System.IntPtr p, out IAudioEndpointVolume ep); }
        [Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IMMDeviceEnumerator { int GetDefaultAudioEndpoint(int flow, int role, out IMMDevice dev); }
        [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] class MMDeviceEnumerator {}
'@
        $e = New-Object MMDeviceEnumerator
        $dev = $null; $e.GetDefaultAudioEndpoint(0, 1, [ref]$dev) | Out-Null
        $id = [Guid]'5CDF2C82-841E-4546-9722-0CF74078229A'
        $vol = $null; $dev.Activate([ref]$id, 1, [IntPtr]::Zero, [ref]$vol) | Out-Null
        $level = 0.0; $vol.GetMasterVolumeLevelScalar([ref]$level) | Out-Null
        $mute = $false; $vol.GetMute([ref]$mute) | Out-Null
        Write-Output "$([math]::Round($level * 100)),$mute"
    "#;
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", ps])
        .output().ok()?;
    if !output.status.success() { return None; }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let mut parts = stdout.split(',');
    let volume: u32 = parts.next()?.parse().ok()?;
    let muted = parts.next()?.trim().to_lowercase() == "true";
    Some(VolumeInfo { volume, muted })
}

#[cfg(target_os = "windows")]
fn set_volume(level: u16) -> bool {
    let ps = format!(r#"
        Add-Type -TypeDefinition @'
        using System.Runtime.InteropServices;
        [Guid("5CDF2C82-841E-4546-9722-0CF74078229A"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IAudioEndpointVolume {{
            int _0(); int _1(); int _2(); int _3(); int _4(); int _5(); int _6(); int _7(); int _8(); int _9(); int _10(); int _11();
            int GetMasterVolumeLevelScalar(out float level);
            int SetMasterVolumeLevelScalar(float level, System.Guid ctx);
        }}
        [Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IMMDevice {{ int Activate(ref System.Guid id, int ctx, System.IntPtr p, out IAudioEndpointVolume ep); }}
        [Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IMMDeviceEnumerator {{ int GetDefaultAudioEndpoint(int flow, int role, out IMMDevice dev); }}
        [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] class MMDeviceEnumerator {{}}
'@
        $e = New-Object MMDeviceEnumerator
        $dev = $null; $e.GetDefaultAudioEndpoint(0, 1, [ref]$dev) | Out-Null
        $id = [Guid]'5CDF2C82-841E-4546-9722-0CF74078229A'
        $vol = $null; $dev.Activate([ref]$id, 1, [IntPtr]::Zero, [ref]$vol) | Out-Null
        $vol.SetMasterVolumeLevelScalar({:.2}, [Guid]::Empty) | Out-Null
    "#, level as f64 / 100.0);
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn set_mute(mute: bool) -> bool {
    // Use NirCmd or PowerShell — simpler approach with nircmd if available, else PowerShell
    let action = if mute { "1" } else { "0" };
    let ps = format!(r#"
        $obj = New-Object -ComObject WScript.Shell
        {}
    "#, if mute {
        // Toggle approach: send volume mute key
        "$obj.SendKeys([char]173)"
    } else {
        "$obj.SendKeys([char]173)"
    });
    // Simpler: use powershell to set mute via COM audio
    let ps = format!(r#"
        Add-Type -TypeDefinition @'
        using System.Runtime.InteropServices;
        [Guid("5CDF2C82-841E-4546-9722-0CF74078229A"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IAudioEndpointVolume {{
            int _0(); int _1(); int _2(); int _3(); int _4(); int _5(); int _6(); int _7(); int _8(); int _9(); int _10();
            int SetMute(bool mute, System.Guid ctx);
        }}
        [Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IMMDevice {{ int Activate(ref System.Guid id, int ctx, System.IntPtr p, out IAudioEndpointVolume ep); }}
        [Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
        interface IMMDeviceEnumerator {{ int GetDefaultAudioEndpoint(int flow, int role, out IMMDevice dev); }}
        [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] class MMDeviceEnumerator {{}}
'@
        $e = New-Object MMDeviceEnumerator
        $dev = $null; $e.GetDefaultAudioEndpoint(0, 1, [ref]$dev) | Out-Null
        $id = [Guid]'5CDF2C82-841E-4546-9722-0CF74078229A'
        $vol = $null; $dev.Activate([ref]$id, 1, [IntPtr]::Zero, [ref]$vol) | Out-Null
        $vol.SetMute(${}, [Guid]::Empty) | Out-Null
    "#, mute);
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// --- Linux: pactl (PulseAudio/PipeWire) with amixer fallback ---

#[cfg(target_os = "linux")]
fn get_volume() -> Option<VolumeInfo> {
    // Try pactl first (PulseAudio / PipeWire)
    if let Some(info) = get_volume_pactl() { return Some(info); }
    // Fallback to amixer (ALSA)
    get_volume_amixer()
}

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
}
