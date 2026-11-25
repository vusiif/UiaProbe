use serde::Serialize;

/// Root JSON payload emitted to stdout by `uiaprobe dump`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DumpOutput {
    pub timestamp: u64,
    pub elapsed_ms: u64,
    pub truncated: bool,
    pub window: WindowInfo,
    pub nodes: Vec<UiNode>,
}

/// Metadata describing the probed target window.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub title: String,
    pub process: String,
    pub dpi_scale: f64,
}

/// A single filtered, interactive UI element in physical screen coordinates.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UiNode {
    pub id: String,
    pub role: String,
    pub name: String,
    pub bbox: BBox,
    pub anchor: Point,
    pub states: Vec<String>,
}

/// Physical pixel bounding box relative to the primary monitor top-left `(0, 0)`.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct BBox {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Physical pixel center point for click/focus targeting.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
