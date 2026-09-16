use crate::{ClipboardHandler, PlatformError};
use cocoa::base::{id, nil};
use cocoa::foundation::NSString;
use objc::{class, msg_send, sel, sel_impl};

pub struct MacClipboard;

impl Default for MacClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl MacClipboard {
    pub fn new() -> Self {
        Self
    }

    fn general_pasteboard() -> id {
        unsafe {
            let cls = class!(NSPasteboard);
            msg_send![cls, generalPasteboard]
        }
    }
}

impl ClipboardHandler for MacClipboard {
    fn read_text(&self) -> Result<Option<String>, PlatformError> {
        unsafe {
            let pb = Self::general_pasteboard();
            let string_type = NSString::alloc(nil).init_str("public.utf8-plain-text");
            let ns_string: id = msg_send![pb, stringForType: string_type];

            if ns_string == nil {
                return Ok(None);
            }

            let utf8: *const std::os::raw::c_char = msg_send![ns_string, UTF8String];
            if utf8.is_null() {
                return Ok(None);
            }

            let c_str = std::ffi::CStr::from_ptr(utf8);
            Ok(Some(c_str.to_string_lossy().into_owned()))
        }
    }

    fn write_text(&self, text: &str) -> Result<(), PlatformError> {
        unsafe {
            let pb = Self::general_pasteboard();
            let _: i64 = msg_send![pb, clearContents];

            let string_type = NSString::alloc(nil).init_str("public.utf8-plain-text");
            let ns_string = NSString::alloc(nil).init_str(text);

            let ok: bool = msg_send![pb, setString:ns_string forType:string_type];
            if !ok {
                return Err(PlatformError::OsError("Failed to write to NSPasteboard".into()));
            }
            Ok(())
        }
    }

    fn read_image(&self) -> Result<Option<Vec<u8>>, PlatformError> {
        unsafe {
            let pb = Self::general_pasteboard();
            let png_type = NSString::alloc(nil).init_str("public.png");
            let ns_data: id = msg_send![pb, dataForType: png_type];

            if ns_data == nil {
                return Ok(None);
            }

            let len: usize = msg_send![ns_data, length];
            let bytes: *const u8 = msg_send![ns_data, bytes];

            if bytes.is_null() || len == 0 {
                return Ok(None);
            }

            let slice = std::slice::from_raw_parts(bytes, len);
            Ok(Some(slice.to_vec()))
        }
    }

    fn write_image(&self, png_bytes: &[u8]) -> Result<(), PlatformError> {
        unsafe {
            let pb = Self::general_pasteboard();
            let _: i64 = msg_send![pb, clearContents];

            let png_type = NSString::alloc(nil).init_str("public.png");
            let ns_data_cls = class!(NSData);
            let ns_data: id = msg_send![ns_data_cls, dataWithBytes:png_bytes.as_ptr() length:png_bytes.len()];

            let ok: bool = msg_send![pb, setData:ns_data forType:png_type];
            if !ok {
                return Err(PlatformError::OsError("Failed to write PNG to NSPasteboard".into()));
            }
            Ok(())
        }
    }

    fn get_change_count(&self) -> u64 {
        unsafe {
            let pb = Self::general_pasteboard();
            let count: i64 = msg_send![pb, changeCount];
            count as u64
        }
    }
}
