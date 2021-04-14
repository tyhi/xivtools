mod citystate;
mod classjob;
mod remote;
pub mod ui;
mod venture;
pub use citystate::CityState;
pub use classjob::ClassJob;
pub use process::Process;
pub use remote::craft;
pub use remote::retainer;
use std::fmt;
pub use venture::Venture;

use anyhow::{anyhow, Error, Result};
use bindings::Windows::Win32::{
    SystemServices::{BOOL, FALSE, PWSTR},
    WindowsAndMessaging::{EnumWindows, GetWindowTextW, HWND, LPARAM},
};

pub const JOB_CNT: usize = 8;
pub const JOBS: [&str; JOB_CNT] = ["CRP", "BSM", "ARM", "GSM", "LTW", "WVR", "ALC", "CUL"];

// The main handle passed back to library methods. The contents are kept
// private to avoid leaking any winapi dependencies to callers.
#[derive(Copy, Clone)]
pub struct XivHandle {
    hwnd: HWND,                    // The handle passed back by the winapi
    pub use_slow_navigation: bool, // Add more delay to XIV navigation
}

impl fmt::Debug for XivHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Xivhandle {{ {} }}", self.hwnd.0 as u64)
    }
}

#[cfg(windows)]
pub fn init() -> Result<XivHandle, Error> {
    let mut arg = HWND::NULL;
    unsafe {
        // TODO: Figure out Rust error handling rather than just panicking inside a lib
        // method.
        match EnumWindows(Some(enum_callback), LPARAM(&mut arg as *mut HWND as isize)) {
            FALSE => Ok(XivHandle {
                hwnd: arg as HWND,
                use_slow_navigation: false,
            }),
            _ => Err(anyhow!(
                "Unable to find XIV window! Is Final Fantasy XIV running?"
            )),
        }
    }
}

// This callback is called for every window the user32 EnumWindows call finds
// while walking the window list. It's used to find the XIV window by title.
//
// To be more foolproof checking process name might be better.
extern "system" fn enum_callback(win_hwnd: HWND, arg: LPARAM) -> BOOL {
    unsafe {
        let mut title = [0; 256];
        let xiv_hwnd = arg.0 as *mut HWND;

        let len = GetWindowTextW(win_hwnd, PWSTR(title.as_mut_ptr()), title.len() as i32);
        let title = String::from_utf16_lossy(&title[..len as usize]);
        log::debug!("found {}: {:?}, arg {:?}", title, win_hwnd, xiv_hwnd);
        if title.contains("FINAL FANTASY XIV") {
            log::info!("Found FFXIV.");
            *xiv_hwnd = win_hwnd;
            return false.into();
        }
        true.into()
    }
}
