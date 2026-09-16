pub mod capture;
pub mod clipboard;
pub mod display;
pub mod inject;

pub use capture::WinInputCapturer;
pub use clipboard::WinClipboard;
pub use display::WinScreenManager;
pub use inject::WinInputInjector;
