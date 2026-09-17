use kvm_core::protocol::{DisplayInfo, InputEvent};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PlatformError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Hook initialization failed: {0}")]
    HookFailed(String),
    #[error("Injection failed: {0}")]
    InjectionFailed(String),
    #[error("OS error: {0}")]
    OsError(String),
}

/// Callback for captured events.
/// If returns `true`, the local OS will swallow/suppress the event (not deliver to local apps).
/// If returns `false`, the event continues to local apps.
pub type EventFilterCallback = Box<dyn Fn(&InputEvent) -> bool + Send + Sync>;

/// Trait for capturing hardware mouse and keyboard events
pub trait InputCapturer: Send + Sync {
    /// Start capturing events. Whenever an event occurs, callback is invoked.
    fn start_capture(&mut self, callback: EventFilterCallback) -> Result<(), PlatformError>;

    /// Stop capture
    fn stop_capture(&mut self) -> Result<(), PlatformError>;

    /// Check if necessary OS permissions (Accessibility / Input Monitoring) are granted
    fn check_permissions(&self) -> bool;

    /// Prompt user to grant required system permissions
    fn request_permissions(&self);
}

/// Trait for injecting synthetic mouse and keyboard events into the local OS
pub trait InputInjector: Send + Sync {
    /// Inject a discrete or delta input event
    fn inject_event(&mut self, event: &InputEvent) -> Result<(), PlatformError>;

    /// Hide the system cursor (when remote machine is active)
    fn hide_cursor(&mut self) -> Result<(), PlatformError>;

    /// Show the system cursor (when returning to local machine)
    fn show_cursor(&mut self) -> Result<(), PlatformError>;

    /// Warp cursor to specific screen coordinates
    fn warp_cursor(&mut self, x: f32, y: f32) -> Result<(), PlatformError>;
}

/// Trait for querying display geometries and scale factors
pub trait ScreenManager: Send + Sync {
    /// Query all active displays on this machine
    fn get_displays(&self) -> Result<Vec<DisplayInfo>, PlatformError>;

    /// Get the primary display
    fn get_primary_display(&self) -> Result<DisplayInfo, PlatformError>;
}

/// Trait for native clipboard monitoring and setting
pub trait ClipboardHandler: Send + Sync {
    /// Read text from local clipboard
    fn read_text(&self) -> Result<Option<String>, PlatformError>;

    /// Write text to local clipboard
    fn write_text(&self, text: &str) -> Result<(), PlatformError>;

    /// Read image data (PNG bytes) if present
    fn read_image(&self) -> Result<Option<Vec<u8>>, PlatformError>;

    /// Write image data (PNG bytes)
    fn write_image(&self, png_bytes: &[u8]) -> Result<(), PlatformError>;

    /// Get current clipboard sequence / change counter
    fn get_change_count(&self) -> u64;
}

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "linux")]
pub mod linux;

pub fn create_input_capturer() -> Box<dyn InputCapturer> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacInputCapturer::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WinInputCapturer::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxInputCapturer)
    }
}

pub fn create_input_injector() -> Box<dyn InputInjector> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacInputInjector::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WinInputInjector::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxInputInjector)
    }
}

pub fn create_screen_manager() -> Box<dyn ScreenManager> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacScreenManager::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WinScreenManager::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxScreenManager)
    }
}

pub fn create_clipboard_handler() -> Box<dyn ClipboardHandler> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacClipboard::new())
    }
    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WinClipboard::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxClipboard)
    }
}
