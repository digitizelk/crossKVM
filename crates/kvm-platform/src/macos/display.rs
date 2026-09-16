use crate::macos::ffi::*;
use crate::{PlatformError, ScreenManager};
use kvm_core::protocol::DisplayInfo;

pub struct MacScreenManager;

impl Default for MacScreenManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MacScreenManager {
    pub fn new() -> Self {
        Self
    }
}

impl ScreenManager for MacScreenManager {
    fn get_displays(&self) -> Result<Vec<DisplayInfo>, PlatformError> {
        let mut displays = [0u32; 16];
        let mut count = 0u32;

        unsafe {
            let res = CGGetActiveDisplayList(16, displays.as_mut_ptr(), &mut count);
            if res != 0 {
                return Err(PlatformError::OsError(format!(
                    "CGGetActiveDisplayList failed with code {}",
                    res
                )));
            }

            let primary_id = CGMainDisplayID();
            let mut list = Vec::with_capacity(count as usize);

            for i in 0..count as usize {
                let id = displays[i];
                let bounds = CGDisplayBounds(id);
                let is_primary = id == primary_id;

                list.push(DisplayInfo {
                    id,
                    name: format!("Display {}", id),
                    width: bounds.size.width as u32,
                    height: bounds.size.height as u32,
                    scale_factor: 2.0, // Standard Retina factor default
                    is_primary,
                    x: bounds.origin.x as i32,
                    y: bounds.origin.y as i32,
                });
            }

            Ok(list)
        }
    }

    fn get_primary_display(&self) -> Result<DisplayInfo, PlatformError> {
        let displays = self.get_displays()?;
        displays
            .into_iter()
            .find(|d| d.is_primary)
            .ok_or_else(|| PlatformError::OsError("No primary display found".into()))
    }
}
