//! Linux platform stub (supports uinput / X11 in future expansion).

use crate::{ClipboardHandler, EventFilterCallback, InputCapturer, InputInjector, PlatformError, ScreenManager};
use kvm_core::protocol::{DisplayInfo, InputEvent};

pub struct LinuxInputCapturer;
impl InputCapturer for LinuxInputCapturer {
    fn check_permissions(&self) -> bool { true }
    fn request_permissions(&self) {}
    fn start_capture(&mut self, _callback: EventFilterCallback) -> Result<(), PlatformError> {
        Err(PlatformError::OsError("Linux capture not yet enabled".into()))
    }
    fn stop_capture(&mut self) -> Result<(), PlatformError> { Ok(()) }
}

pub struct LinuxInputInjector;
impl InputInjector for LinuxInputInjector {
    fn inject_event(&mut self, _event: &InputEvent) -> Result<(), PlatformError> {
        Err(PlatformError::OsError("Linux injection not yet enabled".into()))
    }
    fn hide_cursor(&mut self) -> Result<(), PlatformError> { Ok(()) }
    fn show_cursor(&mut self) -> Result<(), PlatformError> { Ok(()) }
    fn warp_cursor(&mut self, _x: f32, _y: f32) -> Result<(), PlatformError> { Ok(()) }
}

pub struct LinuxScreenManager;
impl ScreenManager for LinuxScreenManager {
    fn get_displays(&self) -> Result<Vec<DisplayInfo>, PlatformError> {
        Ok(vec![DisplayInfo {
            id: 1,
            name: "Default".into(),
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
            is_primary: true,
            x: 0,
            y: 0,
        }])
    }
    fn get_primary_display(&self) -> Result<DisplayInfo, PlatformError> {
        Ok(DisplayInfo {
            id: 1,
            name: "Default".into(),
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
            is_primary: true,
            x: 0,
            y: 0,
        })
    }
}

pub struct LinuxClipboard;
impl ClipboardHandler for LinuxClipboard {
    fn read_text(&self) -> Result<Option<String>, PlatformError> { Ok(None) }
    fn write_text(&self, _text: &str) -> Result<(), PlatformError> { Ok(()) }
    fn read_image(&self) -> Result<Option<Vec<u8>>, PlatformError> { Ok(None) }
    fn write_image(&self, _png_bytes: &[u8]) -> Result<(), PlatformError> { Ok(()) }
    fn get_change_count(&self) -> u64 { 0 }
}
