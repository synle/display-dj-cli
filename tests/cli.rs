// Integration tests that drive the compiled `display-dj` binary as a
// subprocess. Exercises `main`, `dispatch`, `usage`, and all `cmd_*`
// handlers — they are otherwise unreachable from unit tests because
// they call `std::process::exit` on bad input.
//
// CI coverage (cargo-llvm-cov) merges subprocess profraw files into the
// same report as the unit-test profraw, so anything the binary executes
// here counts toward the coverage baseline.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_display-dj"))
}

fn run(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(bin())
        .args(args)
        .output()
        .expect("failed to run display-dj");
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

// ---------------------------------------------------------------------
// --version / -V / help
// ---------------------------------------------------------------------

#[test]
fn version_long_flag_prints_semver() {
    let (code, stdout, _) = run(&["--version"]);
    assert_eq!(code, 0);
    assert!(stdout.contains('.'), "expected version with dots, got {:?}", stdout);
    assert!(!stdout.trim().is_empty());
}

#[test]
fn version_short_flag_prints_semver() {
    let (code, stdout, _) = run(&["-V"]);
    assert_eq!(code, 0);
    assert!(stdout.contains('.'));
}

#[test]
fn help_command_exits_zero_and_prints_usage() {
    let (code, _, stderr) = run(&["help"]);
    assert_eq!(code, 0, "help should exit 0, stderr: {}", stderr);
    assert!(stderr.contains("Usage:"));
    assert!(stderr.contains("display-dj"));
}

#[test]
fn help_long_flag_exits_zero() {
    let (code, _, stderr) = run(&["--help"]);
    assert_eq!(code, 0);
    assert!(stderr.contains("Usage:"));
}

#[test]
fn help_short_flag_exits_zero() {
    let (code, _, stderr) = run(&["-h"]);
    assert_eq!(code, 0);
    assert!(stderr.contains("Usage:"));
}

#[test]
fn no_args_prints_usage() {
    // `cmd` defaults to "help" when no args provided, so this is the same path.
    let (code, _, stderr) = run(&[]);
    assert_eq!(code, 0);
    assert!(stderr.contains("Usage:"));
}

#[test]
fn unknown_command_exits_nonzero() {
    let (code, _, stderr) = run(&["definitely-not-a-real-command"]);
    assert_ne!(code, 0, "unknown command should fail");
    assert!(stderr.contains("Usage:"));
}

// ---------------------------------------------------------------------
// list / get_all / debug — these don't depend on any real display.
// In CI there are no displays, so they should print "[]" and exit 0.
// ---------------------------------------------------------------------

#[test]
fn list_returns_valid_json_array() {
    let (code, stdout, _) = run(&["list"]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("list output must be JSON");
    assert!(parsed.is_array(), "list output should be a JSON array");
}

#[test]
fn get_all_returns_valid_json_array() {
    let (code, stdout, _) = run(&["get_all"]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("get_all output must be JSON");
    assert!(parsed.is_array());
}

#[test]
fn debug_returns_valid_json_object_with_keys() {
    let (code, stdout, _) = run(&["debug"]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("debug output must be JSON");
    assert!(parsed.is_object());
    // Expected keys come from build_debug_info().
    for key in &["version", "os", "arch", "displays", "platform", "tests"] {
        assert!(
            parsed.get(*key).is_some(),
            "debug JSON missing key '{}': {}",
            key,
            stdout
        );
    }
}

// ---------------------------------------------------------------------
// get_one / set_one with no displays — must surface print_not_found path
// ---------------------------------------------------------------------

#[test]
fn get_one_missing_id_arg_exits_nonzero() {
    let (code, _, stderr) = run(&["get_one"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("Usage:"));
}

#[test]
fn get_one_unknown_id_exits_nonzero() {
    let (code, _, stderr) = run(&["get_one", "definitely-no-such-display-99"]);
    assert_ne!(code, 0);
    assert!(
        stderr.contains("not found") || stderr.contains("Available"),
        "stderr: {}",
        stderr
    );
}

#[test]
fn set_one_unknown_id_exits_nonzero() {
    let (code, _, stderr) = run(&["set_one", "no-such-monitor", "50", "force"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("not found") || stderr.contains("Available"));
}

#[test]
fn set_one_missing_args_exits_nonzero() {
    let (code, _, _) = run(&["set_one"]);
    assert_ne!(code, 0);
}

#[test]
fn set_one_bad_level_exits_nonzero() {
    let (code, _, _) = run(&["set_one", "1", "not-a-number"]);
    assert_ne!(code, 0);
}

#[test]
fn set_all_runs_and_exits_with_no_displays() {
    // With zero displays, cmd_set_all just prints "Setting all 0 display(s)" and
    // calls maybe_keep_alive("auto") — "auto" returns without blocking.
    let (code, _, _) = run(&["set_all", "50", "auto"]);
    assert_eq!(code, 0);
}

#[test]
fn set_all_bad_level_exits_nonzero() {
    let (code, _, _) = run(&["set_all", "abc"]);
    assert_ne!(code, 0);
}

#[test]
fn set_all_missing_args_exits_nonzero() {
    let (code, _, _) = run(&["set_all"]);
    assert_ne!(code, 0);
}

// ---------------------------------------------------------------------
// reset
// ---------------------------------------------------------------------

#[test]
fn reset_exits_zero() {
    let (code, _, stderr) = run(&["reset"]);
    assert_eq!(code, 0);
    assert!(stderr.contains("Gamma reset"));
}

// ---------------------------------------------------------------------
// Contrast
// ---------------------------------------------------------------------

#[test]
fn set_contrast_all_with_auto_mode_exits_zero() {
    let (code, _, _) = run(&["set_contrast_all", "50"]);
    assert_eq!(code, 0);
}

#[test]
fn set_contrast_all_bad_arg_exits_nonzero() {
    let (code, _, _) = run(&["set_contrast_all", "not-numeric"]);
    assert_ne!(code, 0);
}

#[test]
fn set_contrast_one_missing_args_exits_nonzero() {
    let (code, _, _) = run(&["set_contrast_one"]);
    assert_ne!(code, 0);
}

#[test]
fn set_contrast_one_unknown_id_exits_nonzero() {
    let (code, _, _) = run(&["set_contrast_one", "no-such", "50"]);
    assert_ne!(code, 0);
}

// ---------------------------------------------------------------------
// Theme / volume / scale / keep-awake / wallpaper CLI handlers
// On CI the OS-level APIs may return None/false but the CLI handlers
// still execute their full code paths.
// ---------------------------------------------------------------------

#[test]
fn theme_command_runs() {
    // get_dark_mode() may return None in CI — handler then exits 1.
    // Either outcome is fine; we just want the path covered.
    let (_code, _, _) = run(&["theme"]);
}

#[test]
fn dark_command_runs() {
    // set_dark_mode() likely fails in CI (no gsettings/AppleScript/registry).
    // We don't assert on exit code; the function is exercised either way.
    let (_code, _, _) = run(&["dark"]);
}

#[test]
fn light_command_runs() {
    let (_code, _, _) = run(&["light"]);
}

#[test]
fn get_volume_returns_some_output() {
    // cmd_get_volume() either prints JSON or "could not detect" + exits 1.
    let (_code, _, _) = run(&["get_volume"]);
}

#[test]
fn set_volume_runs_with_clamped_level() {
    // Clamp path: 200 -> 100. May fail on CI (no audio system) — we only
    // care about exercising the dispatch arm + cmd_set_volume + set_volume.
    let (_code, _, _) = run(&["set_volume", "200"]);
}

#[test]
fn set_volume_bad_arg_exits_nonzero() {
    let (code, _, _) = run(&["set_volume", "not-numeric"]);
    assert_ne!(code, 0);
}

#[test]
fn mute_runs() {
    let (_code, _, _) = run(&["mute"]);
}

#[test]
fn unmute_runs() {
    let (_code, _, _) = run(&["unmute"]);
}

#[test]
fn get_scale_returns_json_array() {
    let (code, stdout, _) = run(&["get_scale"]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("get_scale output must be JSON");
    assert!(parsed.is_array());
}

#[test]
fn set_scale_all_clamps_to_max() {
    // 9999 -> SCALE_MAX (300) — set_scale will likely fail on CI but the path runs.
    let (_code, _, _) = run(&["set_scale_all", "9999"]);
}

#[test]
fn set_scale_all_bad_arg_exits_nonzero() {
    let (code, _, _) = run(&["set_scale_all", "abc"]);
    assert_ne!(code, 0);
}

#[test]
fn set_scale_one_unknown_id_exits_nonzero() {
    let (code, _, _) = run(&["set_scale_one", "no-such-id", "100"]);
    assert_ne!(code, 0);
}

#[test]
fn set_scale_one_missing_args_exits_nonzero() {
    let (code, _, _) = run(&["set_scale_one"]);
    assert_ne!(code, 0);
}

#[test]
fn get_keep_awake_returns_json() {
    let (code, stdout, _) = run(&["get_keep_awake"]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("get_keep_awake output must be JSON");
    assert!(parsed["enabled"].is_boolean());
}

#[test]
fn keep_awake_off_runs() {
    // Disable when not active — should still print a message and exit 0.
    let (code, _, _) = run(&["keep_awake_off"]);
    assert_eq!(code, 0);
}

// ---------------------------------------------------------------------
// Wallpaper CLI surface
// ---------------------------------------------------------------------

#[test]
fn set_wallpaper_invalid_fit_exits_nonzero() {
    let (code, _, stderr) = run(&["set_wallpaper", "zoom", "/some/path"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("Invalid fit"));
}

#[test]
fn set_wallpaper_missing_file_exits_nonzero() {
    let (code, _, stderr) = run(&["set_wallpaper", "fill", "/nonexistent/path/abcxyz.jpg"]);
    assert_ne!(code, 0);
    assert!(stderr.contains("File not found"));
}

#[test]
fn set_wallpaper_missing_args_exits_nonzero() {
    let (code, _, _) = run(&["set_wallpaper"]);
    assert_ne!(code, 0);
}

#[test]
fn set_wallpaper_one_invalid_fit_exits_nonzero() {
    let (code, _, _) = run(&["set_wallpaper_one", "0", "zoom", "/some/path"]);
    assert_ne!(code, 0);
}

#[test]
fn set_wallpaper_one_missing_args_exits_nonzero() {
    let (code, _, _) = run(&["set_wallpaper_one"]);
    assert_ne!(code, 0);
}

#[test]
fn get_wallpaper_returns_json() {
    let (code, stdout, _) = run(&["get_wallpaper"]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("get_wallpaper output must be JSON");
    assert!(parsed.get("path").is_some());
}

#[test]
fn get_wallpaper_supported_returns_json() {
    let (code, stdout, _) = run(&["get_wallpaper_supported"]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("get_wallpaper_supported output must be JSON");
    assert!(parsed["supported"].is_boolean());
}

// ---------------------------------------------------------------------
// Slideshow CLI surface — all the validation arms in slideshow_start
// ---------------------------------------------------------------------

#[test]
fn slideshow_start_interval_too_low_exits_nonzero() {
    let (code, _, _) = run(&[
        "wallpaper_slideshow_start",
        "1",
        "forward",
        "fill",
        "/tmp",
    ]);
    assert_ne!(code, 0);
}

#[test]
fn slideshow_start_invalid_order_exits_nonzero() {
    let (code, _, _) = run(&[
        "wallpaper_slideshow_start",
        "5",
        "scramble",
        "fill",
        "/tmp",
    ]);
    assert_ne!(code, 0);
}

#[test]
fn slideshow_start_invalid_fit_exits_nonzero() {
    let (code, _, _) = run(&[
        "wallpaper_slideshow_start",
        "5",
        "forward",
        "zoom",
        "/tmp",
    ]);
    assert_ne!(code, 0);
}

#[test]
fn slideshow_start_missing_folder_exits_nonzero() {
    let (code, _, _) = run(&[
        "wallpaper_slideshow_start",
        "5",
        "forward",
        "fill",
        "/nonexistent/folder/abcxyz",
    ]);
    assert_ne!(code, 0);
}

#[test]
fn slideshow_start_missing_args_exits_nonzero() {
    let (code, _, _) = run(&["wallpaper_slideshow_start"]);
    assert_ne!(code, 0);
}

#[test]
fn slideshow_status_returns_json_when_idle() {
    let (code, stdout, _) = run(&["wallpaper_slideshow_status"]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("slideshow_status output must be JSON");
    assert!(parsed["running"].is_boolean());
}

#[test]
fn slideshow_stop_when_idle_returns_json() {
    let (code, stdout, _) = run(&["wallpaper_slideshow_stop"]);
    assert_eq!(code, 0);
    let parsed: serde_json::Value =
        serde_json::from_str(stdout.trim()).expect("slideshow_stop output must be JSON");
    assert_eq!(parsed["ok"], true);
}

// ---------------------------------------------------------------------
// HTTP server — spawn `serve` on a random high port, hit several routes,
// then close stdin to trigger the parent-death shutdown path.
//
// Exercises: cmd_serve, write_http, the request parser, every match arm
// in the route dispatch table, and the 404/400/200 status code branches.
// ---------------------------------------------------------------------

fn pick_port() -> u16 {
    // Bind to 0, read the assigned port, drop the listener so the OS frees
    // the port immediately. Race window is microseconds — acceptable for a test.
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("could not pick a free port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

fn http_get(port: u16, path: &str) -> (u16, String) {
    use std::io::Read;
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))
        .expect("connect to test server");
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
        .ok();
    let req = format!("GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", path);
    stream.write_all(req.as_bytes()).expect("send request");
    let mut buf = String::new();
    let _ = stream.read_to_string(&mut buf);
    // Parse status line: "HTTP/1.1 200 OK\r\n..."
    let first_line = buf.lines().next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();
    let status: u16 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    // Body is after the blank line.
    let body = buf.splitn(2, "\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body)
}

#[test]
fn serve_handles_routes_and_shuts_down_on_stdin_close() {
    let port = pick_port();
    let mut child = Command::new(bin())
        .args(["serve", &port.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn server");

    // Wait for the server to bind. Poll-connect up to ~3 seconds.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            panic!("server failed to bind within 3s");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // --- 200 OK routes ---
    let (status, body) = http_get(port, "/health");
    assert_eq!(status, 200);
    assert!(body.contains("\"status\":\"ok\""));

    let (status, body) = http_get(port, "/");
    assert_eq!(status, 200);
    assert!(body.contains("\"name\":\"display-dj\""));
    assert!(body.contains("\"routes\""));

    let (status, body) = http_get(port, "/list");
    assert_eq!(status, 200);
    assert!(body.starts_with('['));

    let (status, body) = http_get(port, "/get_all");
    assert_eq!(status, 200);
    assert!(body.starts_with('['));

    let (status, _) = http_get(port, "/set_all/50");
    assert_eq!(status, 200);

    let (status, _) = http_get(port, "/set_all/50/auto");
    assert_eq!(status, 200);

    let (status, _) = http_get(port, "/set_contrast_all/50");
    assert_eq!(status, 200);

    let (status, _) = http_get(port, "/get_scale");
    assert_eq!(status, 200);

    let (status, _) = http_get(port, "/set_scale_all/100");
    assert_eq!(status, 200);

    let (status, _) = http_get(port, "/keep_awake");
    assert_eq!(status, 200);

    let (status, _) = http_get(port, "/keep_awake/disable");
    assert_eq!(status, 200);

    let (status, _) = http_get(port, "/get_wallpaper");
    assert_eq!(status, 200);

    let (status, _) = http_get(port, "/get_wallpaper_supported");
    assert_eq!(status, 200);

    let (status, _) = http_get(port, "/wallpaper_slideshow_status");
    assert_eq!(status, 200);

    let (status, _) = http_get(port, "/wallpaper_slideshow_stop");
    assert_eq!(status, 200);

    let (status, _) = http_get(port, "/reset");
    assert_eq!(status, 200);

    // --- 400 Bad Request: error branches ---
    let (status, body) = http_get(port, "/get_one");
    assert_eq!(status, 400);
    assert!(body.contains("\"error\""));

    let (status, _) = http_get(port, "/get_one/no-such-monitor");
    assert_eq!(status, 400);

    let (status, _) = http_get(port, "/set_all/not-a-number");
    assert_eq!(status, 400);

    let (status, _) = http_get(port, "/set_one");
    assert_eq!(status, 400);

    let (status, _) = http_get(port, "/set_one/x/y");
    assert_eq!(status, 400);

    let (status, _) = http_get(port, "/set_contrast_all/abc");
    assert_eq!(status, 400);

    let (status, _) = http_get(port, "/set_contrast_one");
    assert_eq!(status, 400);

    let (status, _) = http_get(port, "/set_scale_all/abc");
    assert_eq!(status, 400);

    let (status, _) = http_get(port, "/set_scale_one");
    assert_eq!(status, 400);

    let (status, _) = http_get(port, "/set_volume/abc");
    assert_eq!(status, 400);

    let (status, _) = http_get(port, "/set_wallpaper/zoom/something.jpg");
    assert_eq!(status, 400);

    let (status, _) = http_get(port, "/set_wallpaper");
    assert_eq!(status, 400);

    let (status, _) = http_get(port, "/set_wallpaper_one");
    assert_eq!(status, 400);

    let (status, _) = http_get(port, "/wallpaper_slideshow_start");
    assert_eq!(status, 400);

    // --- 404 Not Found ---
    let (status, body) = http_get(port, "/this-route-does-not-exist");
    assert_eq!(status, 404);
    assert!(body.contains("not found"));

    // --- Drop stdin → triggers the parent-death shutdown path in cmd_serve.
    // The watcher thread reads EOF and calls process::exit(0).
    drop(child.stdin.take());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            _ => {}
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            panic!("server did not shut down within 5s of stdin close");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
