use crate::{InputInjector, PlatformError};
use kvm_core::protocol::{InputEvent, MouseButton};

pub struct WinInputInjector {
    current_x: f32,
    current_y: f32,
}

impl Default for WinInputInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl WinInputInjector {
    pub fn new() -> Self {
        Self {
            current_x: 0.0,
            current_y: 0.0,
        }
    }
}

impl InputInjector for WinInputInjector {
    fn inject_event(&mut self, event: &InputEvent) -> Result<(), PlatformError> {
        #[cfg(target_os = "windows")]
        {
            use std::mem::size_of;
            use windows::Win32::UI::Input::KeyboardAndMouse::{
                SendInput, INPUT, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
                KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSEEVENTF_ABSOLUTE,
                MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN,
                MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
                MOUSEEVENTF_WHEEL, MOUSEINPUT, VIRTUAL_KEY,
            };

            let mut input = INPUT::default();

            match event {
                InputEvent::MouseMoveDelta(delta) => {
                    self.current_x += delta.dx;
                    self.current_y += delta.dy;
                    input.r#type = INPUT_MOUSE;
                    input.Anonymous.mi = MOUSEINPUT {
                        dx: delta.dx as i32,
                        dy: delta.dy as i32,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE,
                        time: 0,
                        dwExtraInfo: 0,
                    };
                }
                InputEvent::MouseMoveAbsolute { x, y } => {
                    self.current_x = *x;
                    self.current_y = *y;
                    // Normalized to 0..65535 for SendInput absolute
                    let norm_x = (*x * 65535.0 / 1920.0) as i32;
                    let norm_y = (*y * 65535.0 / 1080.0) as i32;
                    input.r#type = INPUT_MOUSE;
                    input.Anonymous.mi = MOUSEINPUT {
                        dx: norm_x,
                        dy: norm_y,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                        time: 0,
                        dwExtraInfo: 0,
                    };
                }
                InputEvent::MouseDown(button) => {
                    input.r#type = INPUT_MOUSE;
                    let flag = match button {
                        MouseButton::Left => MOUSEEVENTF_LEFTDOWN,
                        MouseButton::Right => MOUSEEVENTF_RIGHTDOWN,
                        MouseButton::Middle | MouseButton::Other(_) => MOUSEEVENTF_MIDDLEDOWN,
                    };
                    input.Anonymous.mi = MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: flag,
                        time: 0,
                        dwExtraInfo: 0,
                    };
                }
                InputEvent::MouseUp(button) => {
                    input.r#type = INPUT_MOUSE;
                    let flag = match button {
                        MouseButton::Left => MOUSEEVENTF_LEFTUP,
                        MouseButton::Right => MOUSEEVENTF_RIGHTUP,
                        MouseButton::Middle | MouseButton::Other(_) => MOUSEEVENTF_MIDDLEUP,
                    };
                    input.Anonymous.mi = MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: flag,
                        time: 0,
                        dwExtraInfo: 0,
                    };
                }
                InputEvent::Scroll { delta_y, .. } => {
                    input.r#type = INPUT_MOUSE;
                    input.Anonymous.mi = MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: ((*delta_y as i32) * 120) as u32, // 120 = WHEEL_DELTA
                        dwFlags: MOUSEEVENTF_WHEEL,
                        time: 0,
                        dwExtraInfo: 0,
                    };
                }
                InputEvent::KeyDown { scancode, keycode, .. } => {
                    input.r#type = INPUT_KEYBOARD;
                    input.Anonymous.ki = KEYBDINPUT {
                        wVk: VIRTUAL_KEY(*keycode as u16),
                        wScan: *scancode as u16,
                        dwFlags: if *scancode != 0 {
                            KEYEVENTF_SCANCODE
                        } else {
                            KEYBD_EVENT_FLAGS(0)
                        },
                        time: 0,
                        dwExtraInfo: 0,
                    };
                }
                InputEvent::KeyUp { scancode, keycode, .. } => {
                    input.r#type = INPUT_KEYBOARD;
                    input.Anonymous.ki = KEYBDINPUT {
                        wVk: VIRTUAL_KEY(*keycode as u16),
                        wScan: *scancode as u16,
                        dwFlags: KEYEVENTF_KEYUP | if *scancode != 0 {
                            KEYEVENTF_SCANCODE
                        } else {
                            KEYBD_EVENT_FLAGS(0)
                        },
                        time: 0,
                        dwExtraInfo: 0,
                    };
                }
            }

            unsafe {
                SendInput(&[input], size_of::<INPUT>() as i32);
            }
            Ok(())
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = event;
            Ok(())
        }
    }

    fn hide_cursor(&mut self) -> Result<(), PlatformError> {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::ShowCursor;
            unsafe {
                ShowCursor(false);
            }
        }
        Ok(())
    }

    fn show_cursor(&mut self) -> Result<(), PlatformError> {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::ShowCursor;
            unsafe {
                ShowCursor(true);
            }
        }
        Ok(())
    }

    fn warp_cursor(&mut self, x: f32, y: f32) -> Result<(), PlatformError> {
        self.current_x = x;
        self.current_y = y;
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;
            unsafe {
                let _ = SetCursorPos(x as i32, y as i32);
            }
        }
        Ok(())
    }
}
