fn main() {
    windows::build!(
        Windows::Win32::WindowsAndMessaging::{EnumWindows, GetWindowTextW, HWND, LPARAM, PostMessageA, WPARAM},
        Windows::Win32::WindowsAndMessaging::{VK_NUMPAD8, VK_NUMPAD2, VK_NUMPAD4, VK_NUMPAD6, VK_NUMPAD0, VK_NUMPAD9, VK_NUMPAD7, VK_DECIMAL, VK_RETURN, VK_ESCAPE, VK_BACK, VK_HOME, WM_KEYUP, WM_KEYDOWN, WM_CHAR},
        Windows::Win32::SystemServices::{BOOL, PWSTR, OpenProcess, PROCESS_ACCESS_RIGHTS, HANDLE, TRUE, FALSE},
        Windows::Win32::ProcessStatus::{K32EnumProcesses, K32GetModuleBaseNameA, K32EnumProcessModulesEx, K32GetModuleInformation, MODULEINFO},
        Windows::Win32::Debug::{ReadProcessMemory, GetLastError},
    );
}
