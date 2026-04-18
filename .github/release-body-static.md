## Install

Download the binary for your platform, make it executable, and move to your PATH:

```bash
# macOS (Apple Silicon)
chmod +x display-dj-macos-arm64 && mv display-dj-macos-arm64 /usr/local/bin/display-dj

# macOS (Intel)
chmod +x display-dj-macos-x64 && mv display-dj-macos-x64 /usr/local/bin/display-dj

# Linux
chmod +x display-dj-linux-x64 && sudo mv display-dj-linux-x64 /usr/local/bin/display-dj

# Windows: move display-dj-windows-x64.exe somewhere in your PATH
```

## Quick start

```bash
display-dj list              # see your displays
display-dj set_all 50        # set all to 50%
display-dj serve             # start HTTP server on port 51337
```

## Platform dependencies

**macOS** -- No external dependencies. Everything uses native frameworks.

**Windows** -- Volume control requires a one-time PowerShell module install:

```powershell
Install-Module -Name AudioDeviceCmdlets
```

All other features (brightness, dark mode, scaling) work out of the box.

**Linux** -- Requires external CLI tools. Install for your distro:

<details>
<summary>Ubuntu / Debian</summary>

```bash
sudo apt install ddcutil i2c-tools brightnessctl pulseaudio-utils alsa-utils x11-xserver-utils wlr-randr
sudo modprobe i2c-dev
echo "i2c-dev" | sudo tee /etc/modules-load.d/i2c-dev.conf
sudo usermod -aG i2c $USER
```

</details>

<details>
<summary>Fedora / RHEL</summary>

```bash
sudo dnf install ddcutil i2c-tools brightnessctl pulseaudio-utils alsa-utils xrandr wlr-randr
sudo modprobe i2c-dev
echo "i2c-dev" | sudo tee /etc/modules-load.d/i2c-dev.conf
sudo usermod -aG i2c $USER
```

</details>

<details>
<summary>Arch / Manjaro</summary>

```bash
sudo pacman -S ddcutil i2c-tools brightnessctl libpulse alsa-utils xorg-xrandr wlr-randr
sudo modprobe i2c-dev
echo "i2c-dev" | sudo tee /etc/modules-load.d/i2c-dev.conf
sudo usermod -aG i2c $USER
```

</details>

| Feature | Tool | Required? |
|---------|------|-----------|
| Built-in brightness | `brightnessctl` or sysfs write access | Yes (for laptops) |
| External monitor brightness | `ddcutil` + `i2c-dev` module | Yes (for external monitors) |
| Volume | `pactl` (PulseAudio/PipeWire) | Yes, `amixer` as fallback |
| Dark mode (GNOME) | `gsettings` | Comes with GNOME |
| Dark mode (KDE) | `plasma-apply-colorscheme` | Comes with KDE |
| Dark mode (XFCE) | `xfconf-query` | Comes with XFCE |
| Scaling (X11) | `xrandr` | Yes |
| Scaling (Wayland) | `wlr-randr` | Yes (wlroots compositors) |
| Gamma (X11) | `xrandr` | Yes |
| Gamma (Wayland) | `wlr-randr` or `wl-gammarelay-rs` | Yes |
