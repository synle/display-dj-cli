# display-dj-cli

Cross-platform Rust CLI binary for controlling monitor brightness, display scaling, system volume, dark mode, keep-awake, and desktop wallpaper. Uses platform-abstracted modules (`macos.rs`, `windows.rs`, `linux.rs`) compiled conditionally via `#[cfg(target_os)]`. Supports macOS, Windows, and Linux (X11 + Wayland).

## Quick Start

Build (debug):

```bash
cargo build
```

Build (release):

```bash
cargo build --release
```

Run locally:

```bash
cargo run -- list
cargo run -- set_all 50
cargo run -- debug
```

Run tests:

```bash
cargo test
```
