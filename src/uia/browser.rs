use crate::uia::cache::CachedNodeData;

/// Abstraction for browser-specific DOM extraction backends.
///
/// Phase 2 implements `ChromiumDomExtractor` (`RootWebArea` via UIA).
/// Phase 3 will add Firefox/Gecko `IAccessible2` (`oleacc.dll`) fallback.
pub trait BrowserDomExtractor {
    /// Human-readable backend name for diagnostics.
    fn name(&self) -> &'static str;

    /// Returns `true` if `data` represents the root of an embedded web DOM area.
    fn is_dom_root(&self, data: &CachedNodeData) -> bool;
}

/// Chromium / Edge / Electron / WebView2 DOM detector using UIA `RootWebArea`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ChromiumDomExtractor;

impl BrowserDomExtractor for ChromiumDomExtractor {
    fn name(&self) -> &'static str {
        "chromium-uia"
    }

    fn is_dom_root(&self, data: &CachedNodeData) -> bool {
        data.automation_id == "RootWebArea"
    }
}
