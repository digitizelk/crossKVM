use crate::{EventFilterCallback, InputCapturer, PlatformError};
use kvm_core::protocol::{InputEvent, KeyModifiers, MouseButton, MouseDeltaPacket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

static SEQ_COUNTER: AtomicU64 = AtomicU64::new(1);

pub struct WinInputCapturer {
    callback: Option<Arc<EventFilterCallback>>,
    is_running: Arc<AtomicBool>,
}

impl Default for WinInputCapturer {
    fn default() -> Self {
        Self::new()
    }
}

impl WinInputCapturer {
    pub fn new() -> Self {
        Self {
            callback: None,
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl InputCapturer for WinInputCapturer {
    fn check_permissions(&self) -> bool {
        // Windows allows low-level hooks by default.
        // For elevated windows, requires running as administrator or uiAccess="true".
        true
    }

    fn request_permissions(&self) {
        // No explicit TCC dialog required on Windows like macOS
    }

    fn start_capture(&mut self, callback: EventFilterCallback) -> Result<(), PlatformError> {
        let cb_arc = Arc::new(callback);
        self.callback = Some(cb_arc.clone());
        self.is_running.store(true, Ordering::SeqCst);

        #[cfg(target_os = "windows")]
        {
            use std::sync::Mutex;
            use std::thread;
            use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
            use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
            use windows::Win32::UI::WindowsAndMessaging::{
                CallNextHookEx, DispatchMessageW, GetMessageW,
                SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
                MSLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYUP,
                WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE,
                WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYUP,
            };

            static GLOBAL_CB: Mutex<Option<Arc<EventFilterCallback>>> = Mutex::new(None);
            static LAST_PT: Mutex<POINT> = Mutex::new(POINT { x: 0, y: 0 });

            *GLOBAL_CB.lock().unwrap() = Some(cb_arc.clone());

            unsafe extern "system" fn mouse_hook_proc(
                code: i32,
                wparam: WPARAM,
                lparam: LPARAM,
            ) -> LRESULT {
                if code >= 0 {
                    let ms = &*(lparam.0 as *const MSLLHOOKSTRUCT);
                    let mut last = LAST_PT.lock().unwrap();
                    let dx = (ms.pt.x - last.x) as f32;
                    let dy = (ms.pt.y - last.y) as f32;
                    last.x = ms.pt.x;
                    last.y = ms.pt.y;
                    drop(last);

                    let event: Option<InputEvent> = match wparam.0 as u32 {
                        WM_MOUSEMOVE => {
                            let seq = SEQ_COUNTER.fetch_add(1, Ordering::Relaxed);
                            Some(InputEvent::MouseMoveDelta(MouseDeltaPacket {
                                seq,
                                dx,
                                dy,
                                current_x: ms.pt.x as f32,
                                current_y: ms.pt.y as f32,
                            }))
                        }
                        WM_LBUTTONDOWN => Some(InputEvent::MouseDown(MouseButton::Left)),
                        WM_LBUTTONUP => Some(InputEvent::MouseUp(MouseButton::Left)),
                        WM_RBUTTONDOWN => Some(InputEvent::MouseDown(MouseButton::Right)),
                        WM_RBUTTONUP => Some(InputEvent::MouseUp(MouseButton::Right)),
                        WM_MBUTTONDOWN => Some(InputEvent::MouseDown(MouseButton::Middle)),
                        WM_MBUTTONUP => Some(InputEvent::MouseUp(MouseButton::Middle)),
                        WM_MOUSEWHEEL => {
                            let delta = ((ms.mouseData >> 16) as i16) as f32 / 120.0;
                            Some(InputEvent::Scroll {
                                delta_x: 0.0,
                                delta_y: delta,
                            })
                        }
                        _ => None,
                    };

                    if let Some(ref ev) = event {
                        if let Some(ref cb) = *GLOBAL_CB.lock().unwrap() {
                            if cb(ev) {
                                // Swallow event locally!
                                return LRESULT(1);
                            }
                        }
                    }
                }
                CallNextHookEx(HHOOK(std::ptr::null_mut()), code, wparam, lparam)
            }

            unsafe extern "system" fn keyboard_hook_proc(
                code: i32,
                wparam: WPARAM,
                lparam: LPARAM,
            ) -> LRESULT {
                if code >= 0 {
                    let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
                    let is_up = wparam.0 as u32 == WM_KEYUP || wparam.0 as u32 == WM_SYSKEYUP;

                    let shift = (GetKeyState(0x10) as u16 & 0x8000) != 0;
                    let control = (GetKeyState(0x11) as u16 & 0x8000) != 0;
                    let alt = (GetKeyState(0x12) as u16 & 0x8000) != 0;
                    let meta = (GetKeyState(0x5B) as u16 & 0x8000) != 0
                        || (GetKeyState(0x5C) as u16 & 0x8000) != 0;

                    let modifiers = KeyModifiers {
                        shift,
                        control,
                        alt_option: alt,
                        meta_command: meta,
                        caps_lock: (GetKeyState(0x14) as u16 & 1) != 0,
                    };

                    let event = if is_up {
                        InputEvent::KeyUp {
                            scancode: kb.scanCode,
                            keycode: kb.vkCode,
                            modifiers,
                        }
                    } else {
                        InputEvent::KeyDown {
                            scancode: kb.scanCode,
                            keycode: kb.vkCode,
                            modifiers,
                        }
                    };

                    if let Some(ref cb) = *GLOBAL_CB.lock().unwrap() {
                        if cb(&event) {
                            // Swallow keyboard input locally!
                            return LRESULT(1);
                        }
                    }
                }
                CallNextHookEx(HHOOK(std::ptr::null_mut()), code, wparam, lparam)
            }

            let is_running = self.is_running.clone();

            thread::spawn(move || unsafe {
                let mouse_hook = SetWindowsHookExW(
                    WH_MOUSE_LL,
                    Some(mouse_hook_proc),
                    HINSTANCE(std::ptr::null_mut()),
                    0,
                );
                let kbd_hook = SetWindowsHookExW(
                    WH_KEYBOARD_LL,
                    Some(keyboard_hook_proc),
                    HINSTANCE(std::ptr::null_mut()),
                    0,
                );

                let mut msg = MSG::default();
                while is_running.load(Ordering::SeqCst) && GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }

                if let Ok(h) = mouse_hook {
                    let _ = UnhookWindowsHookEx(h);
                }
                if let Ok(h) = kbd_hook {
                    let _ = UnhookWindowsHookEx(h);
                }
            });
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = cb_arc;
        }

        Ok(())
    }

    fn stop_capture(&mut self) -> Result<(), PlatformError> {
        self.is_running.store(false, Ordering::SeqCst);
        self.callback = None;
        Ok(())
    }
}
