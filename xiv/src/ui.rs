#[cfg(windows)]
use bindings::Windows::Win32::WindowsAndMessaging::{PostMessageA, LPARAM, WPARAM};
use std::thread::sleep;
use std::time::Duration;

// This module handles all interactions with the game UI.

// Delay for WM_CHAR events. In testing, even with low fps or
// higher latency this value is still safe because of the game's
// input buffer.
const CHAR_DELAY: f32 = 0.05;
// Delay for window navigation sent via KEYDOWN / KEYUP events.
// These are affected by latency and in testing 200 milliseconds
// seems safe in laggier conditions.
const UI_DELAY: f32 = 0.2;

#[cfg(windows)]
mod constants {
    use bindings::Windows::Win32::WindowsAndMessaging::*;
    pub const KEY_UP: u32 = VK_NUMPAD8;
    pub const KEY_DOWN: u32 = VK_NUMPAD2;
    pub const KEY_LEFT: u32 = VK_NUMPAD4;
    pub const KEY_RIGHT: u32 = VK_NUMPAD6;
    pub const KEY_CONFIRM: u32 = VK_NUMPAD0;
    pub const KEY_FORWARD: u32 = VK_NUMPAD9;
    pub const KEY_BACKWARD: u32 = VK_NUMPAD7;
    pub const KEY_CANCEL: u32 = VK_DECIMAL;
    pub const KEY_ENTER: u32 = VK_RETURN;
    pub const KEY_ESCAPE: u32 = VK_ESCAPE;
    pub const KEY_BACKSPACE: u32 = VK_BACK;
    pub const KEY_SUBCOMMANDS: u32 = VK_HOME;
    pub const MSG_KEY_UP: u32 = WM_KEYUP;
    pub const MSG_KEY_DOWN: u32 = WM_KEYDOWN;
    pub const MSG_KEY_CHAR: u32 = WM_CHAR;
}

#[cfg(not(windows))]
mod constants {
    pub const KEY_UP: i32 = 0;
    pub const KEY_DOWN: i32 = 0;
    pub const KEY_LEFT: i32 = 0;
    pub const KEY_RIGHT: i32 = 0;
    pub const KEY_CONFIRM: i32 = 0;
    pub const KEY_FORWARD: i32 = 0;
    pub const KEY_BACKWARD: i32 = 0;
    pub const KEY_CANCEL: i32 = 0;
    pub const KEY_ENTER: i32 = 0;
    pub const KEY_BACKSPACE: i32 = 0;
    pub const KEY_ESCAPE: i32 = 0;
    pub const KEY_SUBCOMMANDS: i32 = 0;
    pub const MSG_KEY_UP: u32 = 0;
    pub const MSG_KEY_DOWN: u32 = 0;
    pub const MSG_KEY_CHAR: u32 = 0;
}

// Wait |s| seconds, fractions permitted.
pub fn wait(s: f32) {
    let ms = (s * 1000_f32) as u64;
    sleep(Duration::from_millis(ms));
}

pub fn cursor_down(xiv_handle: super::XivHandle) {
    log::debug!("[down]");
    send_key(xiv_handle, constants::KEY_DOWN);
}

pub fn cursor_up(xiv_handle: super::XivHandle) {
    log::debug!("[up]");
    send_key(xiv_handle, constants::KEY_UP);
}

pub fn cursor_left(xiv_handle: super::XivHandle) {
    log::debug!("[left]");
    send_key(xiv_handle, constants::KEY_LEFT);
}

pub fn cursor_right(xiv_handle: super::XivHandle) {
    log::debug!("[right]");
    send_key(xiv_handle, constants::KEY_RIGHT);
}

pub fn cursor_backward(xiv_handle: super::XivHandle) {
    log::debug!("[ui back]");
    send_key(xiv_handle, constants::KEY_BACKWARD)
}

pub fn cursor_forward(xiv_handle: super::XivHandle) {
    log::debug!("[ui forward]");
    send_key(xiv_handle, constants::KEY_FORWARD);
}

pub fn press_backspace(xiv_handle: super::XivHandle) {
    log::debug!("[backspace]");
    send_key(xiv_handle, constants::KEY_BACKSPACE);
}

pub fn press_confirm(xiv_handle: super::XivHandle) {
    log::debug!("[confirm]");
    send_key(xiv_handle, constants::KEY_CONFIRM);
}

pub fn press_cancel(xiv_handle: super::XivHandle) {
    log::debug!("[cancel]");
    send_key(xiv_handle, constants::KEY_CANCEL);
}

pub fn press_enter(xiv_handle: super::XivHandle) {
    log::debug!("[enter]");
    send_key(xiv_handle, constants::KEY_ENTER);
}

pub fn press_escape(xiv_handle: super::XivHandle) {
    log::debug!("[esc]");
    send_key(xiv_handle, constants::KEY_ESCAPE);
}

pub fn press_subcommands(xiv_handle: super::XivHandle) {
    log::debug!("[subcommands]");
    send_key(xiv_handle, constants::KEY_SUBCOMMANDS);
}

pub fn target_nearest_npc(xiv_handle: super::XivHandle) {
    press_enter(xiv_handle);
    send_string(xiv_handle, "/tnpc");
    press_enter(xiv_handle);
}

pub fn send_string(xiv_handle: super::XivHandle, s: &str) {
    log::trace!("sending string: '{}'\n", s);
    for c in s.chars() {
        send_char(xiv_handle, c);
    }
}

pub fn send_action(xiv_handle: super::XivHandle, s: &str, _delay: Option<i64>) {
    send_string(xiv_handle, s);
    wait(0.5);
    press_enter(xiv_handle);
}

// Clear all dialog windows and the text input so we can get
// the game into a state we can trust. If someone kills a craft or
// Talan midway then the UI can be in an inconsistent state, this
// attempts to deal with that. This has been tested in environments
// as low as 11 fps.
pub fn clear_window(xiv_handle: super::XivHandle) {
    log::debug!("clearing the game window");
    // If the text input has focus, try clearing the text to prevent
    // saying junk in a linkshell, /say, etc.
    for _ in 0..32 {
        press_backspace(xiv_handle);
    }
    press_enter(xiv_handle);

    // If we didn't have focus before, we do now and we clear the
    // test this time.
    for _ in 0..32 {
        press_backspace(xiv_handle);
    }
    press_enter(xiv_handle);

    for _ in 0..4 {
        press_escape(xiv_handle);
    }
    press_cancel(xiv_handle);

    // Each press of escape clears out one window, or removes the input focus
    for _ in 0..10 {
        press_cancel(xiv_handle);
    }

    // Cancelling twice will close the System menu if it is open, as well as any
    // remaining text input focus.
    press_cancel(xiv_handle);
    press_cancel(xiv_handle);
}

pub fn send_char(xiv_handle: super::XivHandle, c: char) {
    log::trace!("char: {}", c);
    send_msg(xiv_handle, constants::MSG_KEY_CHAR, c as u32);
    // TODO: Redo this when we have a better timing system
    wait(CHAR_DELAY);
}

pub fn send_key(xiv_handle: super::XivHandle, c: u32) {
    log::trace!("key {:x}", c);
    send_msg(xiv_handle, constants::MSG_KEY_DOWN, c);
    send_msg(xiv_handle, constants::MSG_KEY_UP, c);
    wait(UI_DELAY);
}

// Send a character/key to the XIV window
fn send_msg(_xiv_handle: super::XivHandle, _msg: u32, _key: u32) {
    #[cfg(windows)]
    unsafe {
        PostMessageA(_xiv_handle.hwnd, _msg, WPARAM(_key as usize), LPARAM(0));
    }
}
