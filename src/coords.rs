use crate::model::{BBox, Point};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::WindowsAndMessaging::SetProcessDPIAware;

/// Enables Per-Monitor V2 DPI awareness for the current process.
///
/// Must be called at the very start of `main()` before any Win32 or UIA calls
/// so that `BoundingRectangle` and `GetDpiForWindow` return true physical pixels.
pub fn init_dpi_awareness() {
    // SAFETY: Calling Win32 process-wide DPI awareness APIs once at startup.
    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_err() {
            let _ = SetProcessDPIAware();
        }
    }
}

/// Returns the DPI scale factor (e.g. `1.0`, `1.25`, `1.5`) for the given window handle.
pub fn dpi_scale_for_hwnd(hwnd: isize) -> f64 {
    if hwnd == 0 {
        return 1.0;
    }
    // SAFETY: `GetDpiForWindow` safely returns 0 for invalid handles.
    let dpi = unsafe { GetDpiForWindow(HWND(hwnd as *mut core::ffi::c_void)) };
    if dpi == 0 {
        1.0
    } else {
        (dpi as f64) / 96.0
    }
}

/// Normalizes a UIA bounding rectangle `(left, top, right, bottom)` into physical `BBox` and center `Point`.
///
/// Preserves negative coordinates for multi-monitor setups and rejects zero-area or inverted rectangles.
pub fn normalize_bbox(left: f64, top: f64, right: f64, bottom: f64) -> Option<(BBox, Point)> {
    if !left.is_finite() || !top.is_finite() || !right.is_finite() || !bottom.is_finite() {
        return None;
    }
    let x = left.round() as i32;
    let y = top.round() as i32;
    let r = right.round() as i32;
    let b = bottom.round() as i32;
    let w = r.saturating_sub(x);
    let h = b.saturating_sub(y);
    if w <= 0 || h <= 0 {
        return None;
    }
    let anchor = Point {
        x: x.saturating_add(w / 2),
        y: y.saturating_add(h / 2),
    };
    Some((BBox { x, y, w, h }, anchor))
}
