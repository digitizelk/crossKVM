use std::ffi::c_void;
use core_foundation::base::CFTypeRef;
use core_foundation::mach_port::CFMachPortRef;

pub type CGEventRef = *mut c_void;
pub type CGEventSourceRef = *mut c_void;
pub type CGDirectDisplayID = u32;

pub const K_CG_NULL_DIRECT_DISPLAY: CGDirectDisplayID = 0;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CGSize {
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

pub type CGEventTapLocation = u32;
pub const K_CG_HID_EVENT_TAP: CGEventTapLocation = 0;
pub const K_CG_SESSION_EVENT_TAP: CGEventTapLocation = 1;

pub type CGEventTapPlacement = u32;
pub const K_CG_HEAD_INSERT_EVENT_TAP: CGEventTapPlacement = 0;

pub type CGEventTapOptions = u32;
pub const K_CG_EVENT_TAP_OPTION_DEFAULT: CGEventTapOptions = 0;
pub const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: CGEventTapOptions = 1;

pub type CGEventType = u32;
pub const K_CG_EVENT_NULL: CGEventType = 0;
pub const K_CG_EVENT_LEFT_MOUSE_DOWN: CGEventType = 1;
pub const K_CG_EVENT_LEFT_MOUSE_UP: CGEventType = 2;
pub const K_CG_EVENT_RIGHT_MOUSE_DOWN: CGEventType = 3;
pub const K_CG_EVENT_RIGHT_MOUSE_UP: CGEventType = 4;
pub const K_CG_EVENT_MOUSE_MOVED: CGEventType = 5;
pub const K_CG_EVENT_LEFT_MOUSE_DRAGGED: CGEventType = 6;
pub const K_CG_EVENT_RIGHT_MOUSE_DRAGGED: CGEventType = 7;
pub const K_CG_EVENT_KEY_DOWN: CGEventType = 10;
pub const K_CG_EVENT_KEY_UP: CGEventType = 11;
pub const K_CG_EVENT_FLAGS_CHANGED: CGEventType = 12;
pub const K_CG_EVENT_SCROLL_WHEEL: CGEventType = 22;
pub const K_CG_EVENT_OTHER_MOUSE_DOWN: CGEventType = 25;
pub const K_CG_EVENT_OTHER_MOUSE_UP: CGEventType = 26;
pub const K_CG_EVENT_OTHER_MOUSE_DRAGGED: CGEventType = 27;

pub const K_CG_EVENT_FLAG_MASK_SHIFT: u64 = 0x00020000;
pub const K_CG_EVENT_FLAG_MASK_CONTROL: u64 = 0x00040000;
pub const K_CG_EVENT_FLAG_MASK_ALTERNATE: u64 = 0x00080000;
pub const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 0x00100000;
pub const K_CG_EVENT_FLAG_MASK_ALPHA_LOCK: u64 = 0x00010000;

pub type CGMouseButton = u32;
pub const K_CG_MOUSE_BUTTON_LEFT: CGMouseButton = 0;
pub const K_CG_MOUSE_BUTTON_RIGHT: CGMouseButton = 1;
pub const K_CG_MOUSE_BUTTON_CENTER: CGMouseButton = 2;

pub type CGScrollEventUnit = u32;
pub const K_CG_SCROLL_EVENT_UNIT_PIXEL: CGScrollEventUnit = 0;
pub const K_CG_SCROLL_EVENT_UNIT_LINE: CGScrollEventUnit = 1;

pub type CGEventTapCallBack = extern "C" fn(
    proxy: *mut c_void,
    event_type: CGEventType,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub fn CGEventTapCreate(
        tap: CGEventTapLocation,
        place: CGEventTapPlacement,
        options: CGEventTapOptions,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;

    pub fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);

    pub fn CGEventPost(tap: CGEventTapLocation, event: CGEventRef);

    pub fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
    pub fn CGEventGetFlags(event: CGEventRef) -> u64;
    pub fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    pub fn CGEventGetDoubleValueField(event: CGEventRef, field: u32) -> f64;

    pub fn CGEventCreateMouseEvent(
        source: CGEventSourceRef,
        mouse_type: CGEventType,
        mouse_cursor_position: CGPoint,
        mouse_button: CGMouseButton,
    ) -> CGEventRef;

    pub fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        virtual_key: u16,
        key_down: bool,
    ) -> CGEventRef;

    pub fn CGEventCreateScrollWheelEvent(
        source: CGEventSourceRef,
        units: CGScrollEventUnit,
        wheel_count: u32,
        wheel1: i32,
        wheel2: i32,
    ) -> CGEventRef;

    pub fn CGEventSetFlags(event: CGEventRef, flags: u64);

    pub fn CGWarpMouseCursorPosition(new_cursor_position: CGPoint) -> i32;
    pub fn CGAssociateMouseAndMouseCursorPosition(connected: bool) -> i32;
    pub fn CGDisplayHideCursor(display: CGDirectDisplayID) -> i32;
    pub fn CGDisplayShowCursor(display: CGDirectDisplayID) -> i32;

    pub fn CGMainDisplayID() -> CGDirectDisplayID;
    pub fn CGDisplayBounds(display: CGDirectDisplayID) -> CGRect;
    pub fn CGGetActiveDisplayList(
        max_displays: u32,
        active_displays: *mut CGDirectDisplayID,
        display_count: *mut u32,
    ) -> i32;

    pub fn AXIsProcessTrustedWithOptions(options: CFTypeRef) -> bool;
}

pub const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
pub const K_CG_SCROLL_WHEEL_EVENT_DELTA_AXIS_1: u32 = 11;
pub const K_CG_SCROLL_WHEEL_EVENT_DELTA_AXIS_2: u32 = 12;
pub const K_CG_MOUSE_EVENT_DELTA_X: u32 = 4;
pub const K_CG_MOUSE_EVENT_DELTA_Y: u32 = 5;
