use crate::macos::ffi::*;
use crate::{EventFilterCallback, InputCapturer, PlatformError};
use core_foundation::base::TCFType;
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::mach_port::CFMachPort;
use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_foundation::string::CFString;
use kvm_core::protocol::{InputEvent, KeyModifiers, MouseButton, MouseDeltaPacket};
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

static SEQ_COUNTER: AtomicU64 = AtomicU64::new(1);

pub struct MacInputCapturer {
    callback: Option<Arc<EventFilterCallback>>,
    run_loop: Option<CFRunLoop>,
}

impl Default for MacInputCapturer {
    fn default() -> Self {
        Self::new()
    }
}

impl MacInputCapturer {
    pub fn new() -> Self {
        Self {
            callback: None,
            run_loop: None,
        }
    }

    fn extract_modifiers(flags: u64) -> KeyModifiers {
        KeyModifiers {
            shift: (flags & K_CG_EVENT_FLAG_MASK_SHIFT) != 0,
            control: (flags & K_CG_EVENT_FLAG_MASK_CONTROL) != 0,
            alt_option: (flags & K_CG_EVENT_FLAG_MASK_ALTERNATE) != 0,
            meta_command: (flags & K_CG_EVENT_FLAG_MASK_COMMAND) != 0,
            caps_lock: (flags & K_CG_EVENT_FLAG_MASK_ALPHA_LOCK) != 0,
        }
    }
}

extern "C" fn event_tap_callback(
    _proxy: *mut c_void,
    event_type: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    if event.is_null() || user_info.is_null() {
        return event;
    }

    let cb = unsafe { &*(user_info as *const EventFilterCallback) };

    let flags = unsafe { CGEventGetFlags(event) };
    let modifiers = MacInputCapturer::extract_modifiers(flags);

    let parsed_event: Option<InputEvent> = match event_type {
        K_CG_EVENT_MOUSE_MOVED | K_CG_EVENT_LEFT_MOUSE_DRAGGED | K_CG_EVENT_RIGHT_MOUSE_DRAGGED => {
            let loc = unsafe { CGEventGetLocation(event) };
            let dx = unsafe { CGEventGetDoubleValueField(event, K_CG_MOUSE_EVENT_DELTA_X) } as f32;
            let dy = unsafe { CGEventGetDoubleValueField(event, K_CG_MOUSE_EVENT_DELTA_Y) } as f32;
            let seq = SEQ_COUNTER.fetch_add(1, Ordering::Relaxed);
            Some(InputEvent::MouseMoveDelta(MouseDeltaPacket {
                seq,
                dx,
                dy,
                current_x: loc.x as f32,
                current_y: loc.y as f32,
            }))
        }
        K_CG_EVENT_LEFT_MOUSE_DOWN => Some(InputEvent::MouseDown(MouseButton::Left)),
        K_CG_EVENT_LEFT_MOUSE_UP => Some(InputEvent::MouseUp(MouseButton::Left)),
        K_CG_EVENT_RIGHT_MOUSE_DOWN => Some(InputEvent::MouseDown(MouseButton::Right)),
        K_CG_EVENT_RIGHT_MOUSE_UP => Some(InputEvent::MouseUp(MouseButton::Right)),
        K_CG_EVENT_OTHER_MOUSE_DOWN => Some(InputEvent::MouseDown(MouseButton::Middle)),
        K_CG_EVENT_OTHER_MOUSE_UP => Some(InputEvent::MouseUp(MouseButton::Middle)),
        K_CG_EVENT_SCROLL_WHEEL => {
            let dy = unsafe { CGEventGetIntegerValueField(event, K_CG_SCROLL_WHEEL_EVENT_DELTA_AXIS_1) } as f32;
            let dx = unsafe { CGEventGetIntegerValueField(event, K_CG_SCROLL_WHEEL_EVENT_DELTA_AXIS_2) } as f32;
            Some(InputEvent::Scroll {
                delta_x: dx,
                delta_y: dy,
            })
        }
        K_CG_EVENT_KEY_DOWN => {
            let keycode = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) } as u32;
            Some(InputEvent::KeyDown {
                scancode: keycode,
                keycode,
                modifiers,
            })
        }
        K_CG_EVENT_KEY_UP => {
            let keycode = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) } as u32;
            Some(InputEvent::KeyUp {
                scancode: keycode,
                keycode,
                modifiers,
            })
        }
        _ => None,
    };

    if let Some(ref ev) = parsed_event {
        // If callback returns true, swallow event locally (return NULL)
        let should_swallow = cb(ev);
        if should_swallow {
            return std::ptr::null_mut();
        }
    }

    event
}

impl InputCapturer for MacInputCapturer {
    fn check_permissions(&self) -> bool {
        unsafe { AXIsProcessTrustedWithOptions(std::ptr::null()) }
    }

    fn request_permissions(&self) {
        unsafe {
            let key = CFString::new("AXTrustedCheckOptionPrompt");
            let dict = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), CFBoolean::true_value().as_CFType())]);
            AXIsProcessTrustedWithOptions(dict.as_CFTypeRef());
        }
    }

    fn start_capture(&mut self, callback: EventFilterCallback) -> Result<(), PlatformError> {
        if !self.check_permissions() {
            self.request_permissions();
            return Err(PlatformError::PermissionDenied(
                "macOS Accessibility / Input Monitoring permission not granted. Please allow in System Settings."
                    .into(),
            ));
        }

        let cb_arc = Arc::new(callback);
        self.callback = Some(cb_arc.clone());

        let events_mask: u64 = (1 << K_CG_EVENT_MOUSE_MOVED)
            | (1 << K_CG_EVENT_LEFT_MOUSE_DOWN)
            | (1 << K_CG_EVENT_LEFT_MOUSE_UP)
            | (1 << K_CG_EVENT_RIGHT_MOUSE_DOWN)
            | (1 << K_CG_EVENT_RIGHT_MOUSE_UP)
            | (1 << K_CG_EVENT_LEFT_MOUSE_DRAGGED)
            | (1 << K_CG_EVENT_RIGHT_MOUSE_DRAGGED)
            | (1 << K_CG_EVENT_OTHER_MOUSE_DOWN)
            | (1 << K_CG_EVENT_OTHER_MOUSE_UP)
            | (1 << K_CG_EVENT_SCROLL_WHEEL)
            | (1 << K_CG_EVENT_KEY_DOWN)
            | (1 << K_CG_EVENT_KEY_UP)
            | (1 << K_CG_EVENT_FLAGS_CHANGED);

        let user_info_addr = Arc::into_raw(cb_arc) as usize;

        let (tx, rx) = std::sync::mpsc::channel();

        thread::spawn(move || unsafe {
            let user_info = user_info_addr as *mut c_void;
            let tap = CGEventTapCreate(
                K_CG_SESSION_EVENT_TAP,
                K_CG_HEAD_INSERT_EVENT_TAP,
                K_CG_EVENT_TAP_OPTION_DEFAULT,
                events_mask,
                event_tap_callback,
                user_info,
            );

            if tap.is_null() {
                let _ = tx.send(Err(PlatformError::HookFailed(
                    "CGEventTapCreate failed. Ensure Accessibility permission is granted.".into(),
                )));
                return;
            }

            let mach_port = CFMachPort::wrap_under_create_rule(tap);
            let loop_source = mach_port.create_runloop_source(0).unwrap();
            let current_loop = CFRunLoop::get_current();
            current_loop.add_source(&loop_source, kCFRunLoopCommonModes);

            CGEventTapEnable(tap, true);

            let _ = tx.send(Ok(current_loop));

            CFRunLoop::run_current();
        });

        match rx.recv() {
            Ok(Ok(rl)) => {
                self.run_loop = Some(rl);
                Ok(())
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(PlatformError::HookFailed("Failed to spawn event tap thread".into())),
        }
    }

    fn stop_capture(&mut self) -> Result<(), PlatformError> {
        if let Some(ref rl) = self.run_loop {
            rl.stop();
            self.run_loop = None;
        }
        self.callback = None;
        Ok(())
    }
}
