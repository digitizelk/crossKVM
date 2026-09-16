use crate::{PlatformError, ScreenManager};
use kvm_core::protocol::DisplayInfo;

pub struct WinScreenManager;

impl Default for WinScreenManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WinScreenManager {
    pub fn new() -> Self {
        Self
    }
}

impl ScreenManager for WinScreenManager {
    fn get_displays(&self) -> Result<Vec<DisplayInfo>, PlatformError> {
        #[cfg(target_os = "windows")]
        {
            use std::mem::size_of;
            use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
            use windows::Win32::Graphics::Gdi::{
                EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
            };

            unsafe extern "system" fn monitor_enum_proc(
                hmonitor: HMONITOR,
                _hdc: HDC,
                _lprect: *mut RECT,
                lparam: LPARAM,
            ) -> BOOL {
                let list = &mut *(lparam.0 as *mut Vec<DisplayInfo>);
                let mut info = MONITORINFOEXW::default();
                info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;

                if GetMonitorInfoW(hmonitor, &mut info.monitorInfo as *mut _ as *mut _).as_bool() {
                    let rc = info.monitorInfo.rcMonitor;
                    let width = (rc.right - rc.left) as u32;
                    let height = (rc.bottom - rc.top) as u32;
                    let is_primary = (info.monitorInfo.dwFlags & 1) != 0; // MONITORINFOF_PRIMARY = 1

                    // Convert wide name to String
                    let name_len = info
                        .szDevice
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(info.szDevice.len());
                    let name = String::from_utf16_lossy(&info.szDevice[..name_len]);

                    let id = list.len() as u32 + 1;
                    list.push(DisplayInfo {
                        id,
                        name,
                        width,
                        height,
                        scale_factor: 1.0,
                        is_primary,
                        x: rc.left,
                        y: rc.top,
                    });
                }
                BOOL(1)
            }

            let mut displays: Vec<DisplayInfo> = Vec::new();
            unsafe {
                let lparam = LPARAM(&mut displays as *mut _ as isize);
                EnumDisplayMonitors(HDC(0), None, Some(monitor_enum_proc), lparam);
            }

            if displays.is_empty() {
                // Fallback default if monitor enumeration returns empty
                displays.push(DisplayInfo {
                    id: 1,
                    name: "Generic Display".into(),
                    width: 1920,
                    height: 1080,
                    scale_factor: 1.0,
                    is_primary: true,
                    x: 0,
                    y: 0,
                });
            }

            Ok(displays)
        }

        #[cfg(not(target_os = "windows"))]
        {
            Ok(vec![DisplayInfo {
                id: 1,
                name: "Windows Generic Monitor".into(),
                width: 1920,
                height: 1080,
                scale_factor: 1.0,
                is_primary: true,
                x: 0,
                y: 0,
            }])
        }
    }

    fn get_primary_display(&self) -> Result<DisplayInfo, PlatformError> {
        let displays = self.get_displays()?;
        displays
            .into_iter()
            .find(|d| d.is_primary)
            .ok_or_else(|| PlatformError::OsError("No primary display found on Windows".into()))
    }
}
