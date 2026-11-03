//! Stealth window management.
//!
//! Makes the overlay invisible to screen-capture APIs, toggles click-through
//! mode, and provides helpers for showing/hiding the window.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowLongPtrW, SetWindowDisplayAffinity, SetWindowLongPtrW, ShowWindow,
    GWL_EXSTYLE, SW_HIDE, SW_SHOW, WINDOW_DISPLAY_AFFINITY, WS_EX_LAYERED, WS_EX_TRANSPARENT,
};

/// The display-affinity flag that excludes a window from screen capture.
/// The window remains visible on the physical monitor but is hidden from
/// `BitBlt`, `PrintWindow`, and similar capture APIs.
const WDA_EXCLUDEFROMCAPTURE: WINDOW_DISPLAY_AFFINITY = WINDOW_DISPLAY_AFFINITY(0x11);

/// Applies `WDA_EXCLUDEFROMCAPTURE` to hide the window from screen capture
/// APIs. The window remains visible on the physical monitor.
pub fn apply_capture_exclusion(hwnd: HWND) -> Result<(), String> {
    // SAFETY: `hwnd` must be a valid window handle owned by this process.
    // `SetWindowDisplayAffinity` is safe to call with a valid HWND and a
    // recognised affinity constant.
    unsafe {
        SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)
            .map_err(|e| format!("SetWindowDisplayAffinity failed: {e}"))
    }
}

/// Removes `WDA_EXCLUDEFROMCAPTURE`, making the window visible to screen capture again.
pub fn remove_capture_exclusion(hwnd: HWND) -> Result<(), String> {
    unsafe {
        SetWindowDisplayAffinity(hwnd, WINDOW_DISPLAY_AFFINITY(0))
            .map_err(|e| format!("SetWindowDisplayAffinity failed: {e}"))
    }
}

/// Sets the window to click-through mode.
///
/// When `enabled` is `true`, `WS_EX_TRANSPARENT | WS_EX_LAYERED` are added to
/// the extended style so that mouse events pass through to windows behind.
///
/// When `enabled` is `false`, `WS_EX_TRANSPARENT` is removed (keeping
/// `WS_EX_LAYERED` so the overlay can still use per-pixel alpha).
pub fn set_click_through(hwnd: HWND, enabled: bool) -> Result<(), String> {
    // SAFETY: `GetWindowLongPtrW` / `SetWindowLongPtrW` are safe to call with
    // a valid HWND and `GWL_EXSTYLE`.
    unsafe {
        let current = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        // GetWindowLongPtrW returns 0 on error, but 0 is also a valid (empty)
        // extended style. We rely on callers passing a valid HWND.

        let new_style = if enabled {
            current | WS_EX_TRANSPARENT.0 as isize | WS_EX_LAYERED.0 as isize
        } else {
            // Remove TRANSPARENT but keep LAYERED.
            (current | WS_EX_LAYERED.0 as isize) & !(WS_EX_TRANSPARENT.0 as isize)
        };

        if SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style) == 0 {
            // A return of 0 *might* be an error, or the previous value was 0.
            // Check GetLastError via the windows crate.
            let err = windows::core::Error::from_win32();
            if err.code().is_err() {
                return Err(format!("SetWindowLongPtrW failed: {err}"));
            }
        }

        Ok(())
    }
}

/// Toggles window visibility using `ShowWindow`.
pub fn set_window_visible(hwnd: HWND, visible: bool) {
    let cmd = if visible { SW_SHOW } else { SW_HIDE };
    // SAFETY: `ShowWindow` is safe to call with any HWND. If the HWND is
    // invalid the call is simply a no-op.
    unsafe {
        let _ = ShowWindow(hwnd, cmd);
    }
}

/// Finds the `HWND` of a top-level window by its exact title.
///
/// Returns `None` if no matching window is found.
pub fn find_window_by_title(title: &str) -> Option<HWND> {
    let wide_title: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    let title_pcwstr = windows::core::PCWSTR(wide_title.as_ptr());

    // SAFETY: `FindWindowW` is always safe to call. A null first parameter
    // means "any class".
    let hwnd = unsafe { FindWindowW(None, title_pcwstr) };

    match hwnd {
        Ok(h) if !h.is_invalid() => Some(h),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_nonexistent_window_returns_none() {
        let result = find_window_by_title("__uniteach_nonexistent_test_window__");
        assert!(result.is_none());
    }
}
