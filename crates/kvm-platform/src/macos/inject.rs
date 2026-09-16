use crate::macos::ffi::*;
use crate::{InputInjector, PlatformError};
use kvm_core::protocol::{InputEvent, KeyModifiers, MouseButton};
use std::ptr;

pub struct MacInputInjector {
    current_x: f64,
    current_y: f64,
}

impl Default for MacInputInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl MacInputInjector {
    pub fn new() -> Self {
        Self {
            current_x: 0.0,
            current_y: 0.0,
        }
    }

    fn modifiers_to_cgflags(mods: &KeyModifiers) -> u64 {
        let mut flags = 0u64;
        if mods.shift {
            flags |= K_CG_EVENT_FLAG_MASK_SHIFT;
        }
        if mods.control {
            flags |= K_CG_EVENT_FLAG_MASK_CONTROL;
        }
        if mods.alt_option {
            flags |= K_CG_EVENT_FLAG_MASK_ALTERNATE;
        }
        if mods.meta_command {
            flags |= K_CG_EVENT_FLAG_MASK_COMMAND;
        }
        if mods.caps_lock {
            flags |= K_CG_EVENT_FLAG_MASK_ALPHA_LOCK;
        }
        flags
    }
}

impl InputInjector for MacInputInjector {
    fn inject_event(&mut self, event: &InputEvent) -> Result<(), PlatformError> {
        unsafe {
            match event {
                InputEvent::MouseMoveDelta(delta) => {
                    self.current_x += delta.dx as f64;
                    self.current_y += delta.dy as f64;
                    let point = CGPoint {
                        x: self.current_x,
                        y: self.current_y,
                    };
                    CGWarpMouseCursorPosition(point);
                    let ev = CGEventCreateMouseEvent(
                        ptr::null_mut(),
                        K_CG_EVENT_MOUSE_MOVED,
                        point,
                        K_CG_MOUSE_BUTTON_LEFT,
                    );
                    if !ev.is_null() {
                        CGEventPost(K_CG_HID_EVENT_TAP, ev);
                        core_foundation::base::CFRelease(ev as _);
                    }
                }
                InputEvent::MouseMoveAbsolute { x, y } => {
                    self.current_x = *x as f64;
                    self.current_y = *y as f64;
                    let point = CGPoint {
                        x: self.current_x,
                        y: self.current_y,
                    };
                    CGWarpMouseCursorPosition(point);
                    let ev = CGEventCreateMouseEvent(
                        ptr::null_mut(),
                        K_CG_EVENT_MOUSE_MOVED,
                        point,
                        K_CG_MOUSE_BUTTON_LEFT,
                    );
                    if !ev.is_null() {
                        CGEventPost(K_CG_HID_EVENT_TAP, ev);
                        core_foundation::base::CFRelease(ev as _);
                    }
                }
                InputEvent::MouseDown(button) => {
                    let point = CGPoint {
                        x: self.current_x,
                        y: self.current_y,
                    };
                    let (event_type, cg_button) = match button {
                        MouseButton::Left => (K_CG_EVENT_LEFT_MOUSE_DOWN, K_CG_MOUSE_BUTTON_LEFT),
                        MouseButton::Right => (K_CG_EVENT_RIGHT_MOUSE_DOWN, K_CG_MOUSE_BUTTON_RIGHT),
                        MouseButton::Middle | MouseButton::Other(_) => {
                            (K_CG_EVENT_OTHER_MOUSE_DOWN, K_CG_MOUSE_BUTTON_CENTER)
                        }
                    };
                    let ev = CGEventCreateMouseEvent(ptr::null_mut(), event_type, point, cg_button);
                    if !ev.is_null() {
                        CGEventPost(K_CG_HID_EVENT_TAP, ev);
                        core_foundation::base::CFRelease(ev as _);
                    }
                }
                InputEvent::MouseUp(button) => {
                    let point = CGPoint {
                        x: self.current_x,
                        y: self.current_y,
                    };
                    let (event_type, cg_button) = match button {
                        MouseButton::Left => (K_CG_EVENT_LEFT_MOUSE_UP, K_CG_MOUSE_BUTTON_LEFT),
                        MouseButton::Right => (K_CG_EVENT_RIGHT_MOUSE_UP, K_CG_MOUSE_BUTTON_RIGHT),
                        MouseButton::Middle | MouseButton::Other(_) => {
                            (K_CG_EVENT_OTHER_MOUSE_UP, K_CG_MOUSE_BUTTON_CENTER)
                        }
                    };
                    let ev = CGEventCreateMouseEvent(ptr::null_mut(), event_type, point, cg_button);
                    if !ev.is_null() {
                        CGEventPost(K_CG_HID_EVENT_TAP, ev);
                        core_foundation::base::CFRelease(ev as _);
                    }
                }
                InputEvent::Scroll { delta_x, delta_y } => {
                    let ev = CGEventCreateScrollWheelEvent(
                        ptr::null_mut(),
                        K_CG_SCROLL_EVENT_UNIT_PIXEL,
                        2,
                        *delta_y as i32,
                        *delta_x as i32,
                    );
                    if !ev.is_null() {
                        CGEventPost(K_CG_HID_EVENT_TAP, ev);
                        core_foundation::base::CFRelease(ev as _);
                    }
                }
                InputEvent::KeyDown {
                    keycode,
                    modifiers,
                    ..
                } => {
                    let ev =
                        CGEventCreateKeyboardEvent(ptr::null_mut(), *keycode as u16, true);
                    if !ev.is_null() {
                        CGEventSetFlags(ev, Self::modifiers_to_cgflags(modifiers));
                        CGEventPost(K_CG_HID_EVENT_TAP, ev);
                        core_foundation::base::CFRelease(ev as _);
                    }
                }
                InputEvent::KeyUp {
                    keycode,
                    modifiers,
                    ..
                } => {
                    let ev =
                        CGEventCreateKeyboardEvent(ptr::null_mut(), *keycode as u16, false);
                    if !ev.is_null() {
                        CGEventSetFlags(ev, Self::modifiers_to_cgflags(modifiers));
                        CGEventPost(K_CG_HID_EVENT_TAP, ev);
                        core_foundation::base::CFRelease(ev as _);
                    }
                }
            }
        }
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), PlatformError> {
        unsafe {
            CGDisplayHideCursor(K_CG_NULL_DIRECT_DISPLAY);
        }
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), PlatformError> {
        unsafe {
            CGDisplayShowCursor(K_CG_NULL_DIRECT_DISPLAY);
        }
        Ok(())
    }

    fn warp_cursor(&mut self, x: f32, y: f32) -> Result<(), PlatformError> {
        self.current_x = x as f64;
        self.current_y = y as f64;
        unsafe {
            CGWarpMouseCursorPosition(CGPoint {
                x: self.current_x,
                y: self.current_y,
            });
        }
        Ok(())
    }
}
