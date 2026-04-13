use crate::{DisplayControl, DisplayInfo, Platform, BUILTIN_ID, VCP_BRIGHTNESS, VCP_CONTRAST};
use ddc::Ddc; // trait providing get_vcp_feature / set_vcp_feature
use std::thread;
use std::time::Duration;

// Win32 API imports — the `windows` crate provides safe-ish Rust bindings to Win32.
// Each feature must be explicitly enabled in Cargo.toml.
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use winapi::um::wingdi::SetDeviceGammaRamp;

// =========================================================================
// Built-in display — WMI (Windows Management Instrumentation) via PowerShell.
// Laptops expose brightness through the WmiMonitorBrightness WMI class.
// This only works for the built-in panel, not external monitors.
// =========================================================================

struct BuiltinControl; // unit struct (no fields) — just a type to impl traits on

impl BuiltinControl {
    /// Read current brightness from WMI. Returns 0-100 or None if not a laptop.
    fn wmi_get() -> Option<u32> {
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command",
                "(Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightness -ErrorAction SilentlyContinue).CurrentBrightness"])
            .output().ok()?; // .ok()? = convert Result->Option, return None on error
        if !output.status.success() { return None; }
        // from_utf8_lossy handles invalid UTF-8 gracefully (replaces with ?).
        // .trim() removes whitespace, .parse() converts "75" -> 75u32.
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    }

    /// Set brightness via WMI. value is 0-100.
    fn wmi_set(value: u16) -> bool {
        let cmd = format!(
            "(Get-WmiObject -Namespace root/WMI -Class WmiMonitorBrightnessMethods).WmiSetBrightness(1, {})",
            value
        );
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &cmd])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl DisplayControl for BuiltinControl {
    fn get_brightness(&mut self) -> Option<u32> { Self::wmi_get() } // Self = BuiltinControl
    fn get_contrast(&mut self) -> Option<u32> { None }              // WMI doesn't expose contrast
    fn set_brightness(&mut self, value: u16, _mode: &str) -> bool { Self::wmi_set(value) }
    fn set_contrast(&mut self, _value: u16) -> bool { false }
    fn reset_gamma(&self) {}
}

// =========================================================================
// External monitor — DDC/CI via ddc-winapi + gamma ramp via GDI32.
// ddc-winapi uses the Dxva2.dll API to send I2C commands to monitors.
// =========================================================================

struct ExternalControl {
    ddc_monitor: ddc_winapi::Monitor, // wraps a physical monitor handle from Dxva2
    hmonitor: HMONITOR,               // GDI monitor handle for gamma ramp access
    ddc_supported: bool,
}

impl DisplayControl for ExternalControl {
    fn get_brightness(&mut self) -> Option<u32> {
        // Read VCP brightness register via DDC/CI, convert to 0-100 percentage
        self.ddc_monitor.get_vcp_feature(VCP_BRIGHTNESS).ok().map(|val| {
            let max = val.maximum() as f64;
            let cur = val.value() as f64;
            if max > 0.0 { (cur / max * 100.0).round() as u32 } else { 50 }
        })
    }

    fn get_contrast(&mut self) -> Option<u32> {
        self.ddc_monitor.get_vcp_feature(VCP_CONTRAST).ok().map(|val| {
            let max = val.maximum() as f64;
            let cur = val.value() as f64;
            if max > 0.0 { (cur / max * 100.0).round() as u32 } else { 50 }
        })
    }

    fn set_brightness(&mut self, value: u16, mode: &str) -> bool {
        let use_ddc = mode == "ddc" || mode == "force" || (mode == "auto" && self.ddc_supported);
        let use_gamma = mode == "gamma" || mode == "force" || (mode == "auto" && !self.ddc_supported);
        let mut ok = true;

        if use_ddc && self.ddc_supported {
            let ddc_val = if value == 0 { 1 } else { value }; // clamp to 1 to avoid standby
            if self.ddc_monitor.set_vcp_feature(VCP_BRIGHTNESS, ddc_val).is_err() {
                ok = false;
            }
            thread::sleep(Duration::from_millis(100)); // DDC processing delay
        }

        if use_gamma {
            set_gamma_for_hmonitor(self.hmonitor, value as u32);
        }

        ok
    }

    fn set_contrast(&mut self, value: u16) -> bool {
        if !self.ddc_supported { return false; } // early return
        self.ddc_monitor.set_vcp_feature(VCP_CONTRAST, value).is_ok()
    }

    fn reset_gamma(&self) {
        set_gamma_for_hmonitor(self.hmonitor, 100);
    }
}

/// Set software brightness by writing a gamma ramp to the GPU via GDI32.
/// brightness 0-100 scales the ramp linearly.
///
/// The gamma ramp is a 768-element u16 array: [256 red, 256 green, 256 blue].
/// Each entry maps an input intensity (0-255) to an output intensity (0-65535).
fn set_gamma_for_hmonitor(hmonitor: HMONITOR, brightness: u32) {
    let factor = (brightness.min(100) as f64) / 100.0;
    unsafe {
        // Get the monitor's device name so we can create a DC (device context) for it
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(hmonitor, &mut info.monitorInfo as *mut _).as_bool() {
            let hdc = CreateDCW(
                windows::core::PCWSTR(info.szDevice.as_ptr()),
                None, None, None,
            );
            if !hdc.is_invalid() {
                // Build the gamma ramp — linear from 0 to (factor * 65535) for each channel
                let mut ramp = [0u16; 768];
                for i in 0..256 {
                    let val = ((i as f64 / 255.0 * factor) * 65535.0) as u16;
                    ramp[i] = val;       // red
                    ramp[256 + i] = val; // green
                    ramp[512 + i] = val; // blue
                }
                let _ = SetDeviceGammaRamp(hdc.0 as winapi::shared::windef::HDC, ramp.as_ptr() as *mut _);
                let _ = DeleteDC(hdc); // clean up the device context
            }
        }
    }
}

/// Enumerate all HMONITOR handles using the Win32 EnumDisplayMonitors callback.
/// HMONITOR is the GDI handle for gamma ramp access (separate from DDC handles).
fn enum_hmonitors() -> Vec<HMONITOR> {
    let mut hmonitors: Vec<HMONITOR> = Vec::new();
    unsafe {
        // Win32 callback — called once per monitor. We push each handle into the Vec.
        // LPARAM carries our Vec pointer through the callback (Win32's version of closure context).
        unsafe extern "system" fn enum_proc(
            hmonitor: HMONITOR, _hdc: HDC, _lprect: *mut RECT, lparam: LPARAM,
        ) -> BOOL {
            let monitors = &mut *(lparam.0 as *mut Vec<HMONITOR>);
            monitors.push(hmonitor);
            BOOL(1) // return TRUE to continue enumeration
        }
        let _ = EnumDisplayMonitors(
            None, None, Some(enum_proc),
            LPARAM(&mut hmonitors as *mut Vec<HMONITOR> as isize),
        );
    }
    hmonitors
}

/// Get the PnP device identifier and primary flag for an HMONITOR.
/// Returns (device_identifier, is_primary).
/// The identifier is extracted from the monitor's PnP device ID via EnumDisplayDevicesW
/// (e.g. "DEL40F4" for a Dell, "GSM5BBF" for an LG), or falls back to the display
/// device name (e.g. "DISPLAY2").
fn get_hmonitor_details(hmonitor: HMONITOR) -> (String, bool) {
    unsafe {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(hmonitor, &mut info.monitorInfo as *mut _).as_bool() {
            return (String::new(), false);
        }

        let is_primary = (info.monitorInfo.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY

        // Call EnumDisplayDevicesW with the adapter device name to get the monitor's PnP ID.
        let mut dd: DISPLAY_DEVICEW = std::mem::zeroed();
        dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
        if EnumDisplayDevicesW(
            windows::core::PCWSTR(info.szDevice.as_ptr()),
            0,
            &mut dd,
            0,
        ).as_bool() {
            let full_id = String::from_utf16_lossy(
                &dd.DeviceID[..dd.DeviceID.iter().position(|&c| c == 0).unwrap_or(dd.DeviceID.len())]
            );
            // PnP device ID looks like "MONITOR\DEL40F4\{guid}\NNNN" — extract "DEL40F4"
            let parts: Vec<&str> = full_id.split('\\').collect();
            if parts.len() >= 2 && !parts[1].is_empty() {
                return (parts[1].to_string(), is_primary);
            }
        }

        // Fallback: extract display number from device name ("\\.\DISPLAY2" -> "DISPLAY2")
        let device_name = String::from_utf16_lossy(
            &info.szDevice[..info.szDevice.iter().position(|&c| c == 0).unwrap_or(info.szDevice.len())]
        );
        let fallback = device_name.trim_start_matches("\\\\.\\").to_string();
        (fallback, is_primary)
    }
}

// =========================================================================
// Platform implementation — discovers all displays on Windows
// =========================================================================

pub struct WinPlatform;

impl Platform for WinPlatform {
    fn enumerate() -> Vec<(DisplayInfo, Box<dyn DisplayControl>)> {
        let mut result: Vec<(DisplayInfo, Box<dyn DisplayControl>)> = Vec::new();

        // --- Built-in display ---
        // WMI brightness is only available on laptops with an internal panel.
        // If wmi_get returns None, there's no built-in display (or it's a desktop).
        if let Some(brightness) = BuiltinControl::wmi_get() {
            let info = DisplayInfo {
                id: BUILTIN_ID.into(),
                name: "Built-in Display".into(),
                display_type: "builtin".into(),
                brightness: Some(brightness),
                contrast: None,
                ddc_supported: false,
            };
            // BuiltinControl is a unit struct — no fields to initialize
            result.push((info, Box::new(BuiltinControl)));
        }

        // --- External displays ---
        // We need both DDC handles (for brightness) and HMONITOR handles (for gamma).
        // These come from different APIs so we zip them together by index.
        // ddc_winapi::Monitor::enumerate() and enum_hmonitors() both use
        // EnumDisplayMonitors internally, so indices align (assuming 1 physical per logical).
        let has_builtin = !result.is_empty();
        let hmonitors = enum_hmonitors();
        let hmonitor_details: Vec<(String, bool)> = hmonitors.iter()
            .map(|&hm| get_hmonitor_details(hm))
            .collect();

        if let Ok(monitors) = ddc_winapi::Monitor::enumerate() {
            let mut ext_id = 1usize;
            for (idx, mut mon) in monitors.into_iter().enumerate() {
                let hmonitor = hmonitors.get(idx).copied().unwrap_or(HMONITOR::default());
                let (device_id, is_primary) = hmonitor_details.get(idx)
                    .cloned()
                    .unwrap_or((String::new(), false));

                // Skip the primary (built-in) monitor if we already added it via WMI.
                // On laptops, the built-in panel often appears in both WMI and DDC
                // enumeration, causing a duplicate "Generic PnP Monitor" entry.
                if has_builtin && is_primary {
                    continue;
                }

                let brightness = mon.get_vcp_feature(VCP_BRIGHTNESS).ok().map(|val| {
                    let max = val.maximum() as f64;
                    let cur = val.value() as f64;
                    if max > 0.0 { (cur / max * 100.0).round() as u32 } else { 50 }
                });
                let contrast = mon.get_vcp_feature(VCP_CONTRAST).ok().map(|val| {
                    let max = val.maximum() as f64;
                    let cur = val.value() as f64;
                    if max > 0.0 { (cur / max * 100.0).round() as u32 } else { 50 }
                });
                let ddc_supported = brightness.is_some();

                // Append device PnP identifier to distinguish monitors with the
                // same generic description (e.g. "Generic PnP Monitor (DEL40F4)")
                let base_name = mon.description();
                let name = if device_id.is_empty() {
                    base_name
                } else {
                    format!("{} ({})", base_name, device_id)
                };

                let info = DisplayInfo {
                    id: ext_id.to_string(),
                    name,
                    display_type: "external".into(),
                    brightness,
                    contrast,
                    ddc_supported,
                };
                result.push((info, Box::new(ExternalControl { ddc_monitor: mon, hmonitor, ddc_supported })));
                ext_id += 1;
            }
        }

        result
    }

    fn reset_all_gamma() {
        // Reset gamma on every monitor to the identity ramp (100% = no dimming)
        for hmonitor in enum_hmonitors() {
            set_gamma_for_hmonitor(hmonitor, 100);
        }
    }
}
