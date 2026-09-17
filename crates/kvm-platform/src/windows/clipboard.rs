use crate::{ClipboardHandler, PlatformError};

pub struct WinClipboard;

impl Default for WinClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl WinClipboard {
    pub fn new() -> Self {
        Self
    }
}

impl ClipboardHandler for WinClipboard {
    fn read_text(&self) -> Result<Option<String>, PlatformError> {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::Foundation::HWND;
            use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
            use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};

            const CF_UNICODETEXT: u32 = 13;

            unsafe {
                if OpenClipboard(HWND(std::ptr::null_mut())).is_err() {
                    return Ok(None);
                }

                let handle = GetClipboardData(CF_UNICODETEXT);
                if handle.is_err() {
                    let _ = CloseClipboard();
                    return Ok(None);
                }

                let ptr = GlobalLock(handle.unwrap().0 as _);
                if ptr.is_null() {
                    let _ = CloseClipboard();
                    return Ok(None);
                }

                let wide_slice = {
                    let mut len = 0;
                    let u16_ptr = ptr as *const u16;
                    while *u16_ptr.add(len) != 0 {
                        len += 1;
                    }
                    std::slice::from_raw_parts(u16_ptr, len)
                };

                let text = String::from_utf16_lossy(wide_slice);
                let _ = GlobalUnlock(handle.unwrap().0 as _);
                let _ = CloseClipboard();
                Ok(Some(text))
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            Ok(None)
        }
    }

    fn write_text(&self, text: &str) -> Result<(), PlatformError> {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::Foundation::{HANDLE, HWND};
            use windows::Win32::System::DataExchange::{
                CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
            };
            use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

            const CF_UNICODETEXT: u32 = 13;

            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let size = wide.len() * 2;

            unsafe {
                if OpenClipboard(HWND(std::ptr::null_mut())).is_err() {
                    return Err(PlatformError::OsError("Failed to open Windows clipboard".into()));
                }

                let _ = EmptyClipboard();

                let hmem = GlobalAlloc(GMEM_MOVEABLE, size);
                if let Ok(mem) = hmem {
                    let ptr = GlobalLock(mem.0 as _);
                    if !ptr.is_null() {
                        std::ptr::copy_nonoverlapping(wide.as_ptr() as *const u8, ptr as *mut u8, size);
                        let _ = GlobalUnlock(mem.0 as _);
                        let _ = SetClipboardData(CF_UNICODETEXT, HANDLE(mem.0));
                    }
                }

                let _ = CloseClipboard();
                Ok(())
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = text;
            Ok(())
        }
    }

    fn read_image(&self) -> Result<Option<Vec<u8>>, PlatformError> {
        Ok(None)
    }

    fn write_image(&self, png_bytes: &[u8]) -> Result<(), PlatformError> {
        let _ = png_bytes;
        Ok(())
    }

    fn get_change_count(&self) -> u64 {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::System::DataExchange::GetClipboardSequenceNumber;
            unsafe { GetClipboardSequenceNumber() as u64 }
        }

        #[cfg(not(target_os = "windows"))]
        {
            0
        }
    }
}
