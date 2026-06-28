//! Platform window-handle extraction for native text input backends.
//!
//! Raw handles are contained in this native-player module and are never written
//! into trace or Sans I/O data.

#[cfg(target_os = "windows")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(target_os = "windows")]
use std::ffi::c_void;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;

#[derive(Clone, Copy, Debug)]
pub(crate) struct NativeTextInputWindowContext {
    #[cfg(target_os = "windows")]
    hwnd: Option<HWND>,
}

impl NativeTextInputWindowContext {
    pub(crate) fn from_winit_window(window: &dyn winit::window::Window) -> Self {
        #[cfg(target_os = "windows")]
        {
            let hwnd = window
                .window_handle()
                .ok()
                .and_then(|handle| match handle.as_raw() {
                    RawWindowHandle::Win32(handle) => Some(HWND(handle.hwnd.get() as *mut c_void)),
                    _ => None,
                });
            Self { hwnd }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = window;
            Self {}
        }
    }

    #[cfg(test)]
    pub(crate) const fn unavailable_for_tests() -> Self {
        #[cfg(target_os = "windows")]
        {
            Self { hwnd: None }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self {}
        }
    }

    #[cfg(target_os = "windows")]
    pub(crate) const fn hwnd(&self) -> Option<HWND> {
        self.hwnd
    }
}
