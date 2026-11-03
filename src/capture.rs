//! Monitor enumeration and screenshot capture.
//!
//! Wraps the `xcap` crate to list monitors and grab screenshots, returning
//! either raw `RgbaImage` data or PNG-encoded bytes.

use image::RgbaImage;
use std::io::Cursor;

/// Metadata for a single connected monitor.
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub name: String,
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
}

/// Lists all connected monitors with their metadata.
///
/// Returns an empty `Vec` if enumeration fails (e.g. no monitors attached in a
/// headless environment).
pub fn list_monitors() -> Vec<MonitorInfo> {
    let monitors = match xcap::Monitor::all() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to enumerate monitors: {e}");
            return Vec::new();
        }
    };

    monitors
        .into_iter()
        .map(|m| MonitorInfo {
            name: m.name().to_string(),
            id: m.id(),
            width: m.width(),
            height: m.height(),
            x: m.x(),
            y: m.y(),
        })
        .collect()
}

/// Captures a screenshot of the monitor at the given index.
///
/// Returns the raw RGBA image. The index corresponds to the position in the
/// list returned by [`list_monitors`].
pub fn capture_monitor(index: usize) -> Result<RgbaImage, String> {
    let monitors = xcap::Monitor::all().map_err(|e| format!("Monitor enumeration failed: {e}"))?;

    let monitor = monitors
        .into_iter()
        .nth(index)
        .ok_or_else(|| format!("Monitor index {index} out of range"))?;

    monitor
        .capture_image()
        .map_err(|e| format!("Screen capture failed: {e}"))
}

/// Captures a screenshot of the monitor at `index` and encodes it as PNG bytes.
pub fn capture_monitor_png(index: usize) -> Result<Vec<u8>, String> {
    let image = capture_monitor(index)?;
    encode_rgba_to_png(&image)
}

/// Encodes an `RgbaImage` into PNG bytes in memory.
fn encode_rgba_to_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut buf = Cursor::new(Vec::new());
    image
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("PNG encoding failed: {e}"))?;
    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_monitors_does_not_panic() {
        // Just ensure the call completes without panicking.
        let monitors = list_monitors();
        for m in &monitors {
            assert!(m.width > 0);
            assert!(m.height > 0);
        }
    }
}
