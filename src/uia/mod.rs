pub mod browser;
pub mod cache;
pub mod ia2;
pub mod standard;

use crate::coords::dpi_scale_for_hwnd;
use crate::error::{ProbeError, Result};
use crate::model::WindowInfo;
use std::path::Path;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND, RPC_E_CHANGED_MODE};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindow,
};

/// RAII guard for COM initialization on the current thread.
pub struct ComGuard {
    should_uninit: bool,
}

impl ComGuard {
    /// Initializes COM in multithreaded mode (`COINIT_MULTITHREADED`).
    pub fn init() -> Result<Self> {
        // SAFETY: Initializing COM for the current thread.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_ok() {
            Ok(Self {
                should_uninit: true,
            })
        } else if hr == RPC_E_CHANGED_MODE {
            Ok(Self {
                should_uninit: false,
            })
        } else {
            Err(ProbeError::ComInit(format!(
                "CoInitializeEx failed: {hr:?}"
            )))
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            // SAFETY: Matches a successful `CoInitializeEx` on this thread.
            unsafe { CoUninitialize() };
        }
    }
}

/// Active UIA session holding the `IUIAutomation` root COM object.
///
/// Field order is critical: `automation` must be declared before `_com` so that
/// `IUIAutomation::Release` runs before `CoUninitialize`.
pub struct UiaSession {
    pub automation: IUIAutomation,
    _com: ComGuard,
}

impl UiaSession {
    /// Creates a new COM + `IUIAutomation` session.
    pub fn new() -> Result<Self> {
        let com = ComGuard::init()?;
        // SAFETY: COM is initialized on this thread; instantiating the standard CUIAutomation coclass.
        let automation: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.map_err(
                |e| ProbeError::ComInit(format!("CoCreateInstance(CUIAutomation) failed: {e}")),
            )?;
        Ok(Self {
            automation,
            _com: com,
        })
    }
}

/// Resolves the foreground window handle, returning an error if none is active.
pub fn foreground_hwnd() -> Result<isize> {
    // SAFETY: `GetForegroundWindow` is safe to call from any thread.
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return Err(ProbeError::InvalidWindow(
            "no active foreground window".to_string(),
        ));
    }
    Ok(hwnd.0 as isize)
}

/// Validates that `hwnd` refers to an existing Win32 window.
pub fn validate_hwnd(hwnd: isize) -> Result<HWND> {
    if hwnd == 0 {
        return Err(ProbeError::InvalidWindow("null window handle".to_string()));
    }
    let raw = HWND(hwnd as *mut core::ffi::c_void);
    // SAFETY: `IsWindow` safely validates an arbitrary HWND.
    if unsafe { !IsWindow(raw).as_bool() } {
        return Err(ProbeError::InvalidWindow(format!(
            "window handle {hwnd} does not exist"
        )));
    }
    Ok(raw)
}

/// Reads window metadata (`hwnd`, `title`, `process`, `dpi_scale`) using Win32 APIs only.
pub fn read_window_info(hwnd: isize) -> Result<WindowInfo> {
    let raw = validate_hwnd(hwnd)?;
    let title = read_window_title(raw);
    let process = read_process_basename(raw).unwrap_or_else(|| "unknown.exe".to_string());
    let dpi_scale = dpi_scale_for_hwnd(hwnd);

    Ok(WindowInfo {
        hwnd,
        title,
        process,
        dpi_scale,
    })
}

fn read_window_title(hwnd: HWND) -> String {
    // SAFETY: `GetWindowTextLengthW` and `GetWindowTextW` operate on a validated HWND.
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len as usize) + 1];
        let copied = GetWindowTextW(hwnd, &mut buf);
        if copied <= 0 {
            String::new()
        } else {
            String::from_utf16_lossy(&buf[..copied as usize])
        }
    }
}

fn read_process_basename(hwnd: HWND) -> Option<String> {
    let mut pid: u32 = 0;
    // SAFETY: `GetWindowThreadProcessId` writes the owning PID to `pid`.
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == 0 {
        return None;
    }
    // SAFETY: Opening the process with limited query rights and closing the handle before returning.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = vec![0u16; 512];
        let mut len = buf.len() as u32;
        let res = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);
        res.ok()?;
        let full = String::from_utf16_lossy(&buf[..len as usize]);
        Path::new(&full)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
    }
}
