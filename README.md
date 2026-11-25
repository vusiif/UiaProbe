# UiaProbe

A lightweight, read-only, non-intrusive Windows GUI structure perception CLI tool written in Rust.

`UiaProbe` (`uiaprobe.exe`) traverses Windows UI Automation (UIA) control trees and browser DOMs, filters interactive elements, normalizes multi-monitor/DPI coordinates to physical screen pixels (primary monitor top-left `(0, 0)`), and outputs structured JSON to `stdout`.

## Features

- **Per-Monitor V2 DPI Awareness**: Calls `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` at process startup so bounding boxes and anchors are always in true physical pixels (negative coordinates preserved on multi-monitor setups).
- **Batched COM Reads**: Uses `IUIAutomationCacheRequest` (`TreeScope_Element | TreeScope_Children`) to prefetch properties and patterns in-process, avoiding per-property cross-process COM round-trips.
- **Browser DOM Extraction (`--dom`)**:
  - Chromium / Edge / Electron / WebView2 via `RootWebArea` detection.
  - Firefox / Gecko fallback via `oleacc.dll` (`IAccessible` / MSAA).
- **Noise Reduction & Budget Control**:
  - 12-role declarative whitelist: `Button`, `Edit`, `CheckBox`, `RadioButton`, `ComboBox`, `MenuItem`, `TabItem`, `Hyperlink`, `TreeItem`, `ListItem`, `Slider`, `SplitButton`.
  - Extracts `enabled`, `focused`, `checked`, `selected`, `expanded`, `readonly` (`--compact` omits default `enabled`).
  - Redacts password fields (`***`), prunes offscreen/`0x0` elements, and enforces `--depth` (default `16`), `--max-nodes` (default `300`), and an internal `3000` raw-visit circuit breaker.
- **Snapshot Diff (`--diff`) & Resident `stdio` Mode**:
  - Compare against a baseline JSON snapshot (`--diff prev.json`).
  - Run `uiaprobe stdio` for persistent JSON-line IPC reusing a single `IUIAutomation` session.

## Build

```powershell
cargo build --release
```

The standalone binary is emitted at `target\release\uiaprobe.exe`.

## Usage

### `dump`

```powershell
# Active foreground window (pretty JSON)
uiaprobe dump --target active --json

# Specific HWND (decimal or 0x hex) with browser DOM extraction
uiaprobe dump --hwnd 0x304d2 --dom --json

# Compact single-line JSON with custom depth & node budget
uiaprobe dump --target active --depth 8 --max-nodes 100 --compact

# Diff against a previous dump
uiaprobe dump --target active --diff prev.json --json
```

### `stdio` (Resident Interactive Mode)

Send newline-delimited JSON commands on `stdin` and read single-line JSON responses on `stdout`:

```json
{"cmd":"ping"}
{"cmd":"dump","dom":true,"depth":16,"max_nodes":300,"compact":true,"diff":true}
{"cmd":"quit"}
```

## Output Schema

`stdout` emits only valid JSON; all diagnostics and warnings go to `stderr`.

```json
{
  "timestamp": 1774340000000,
  "elapsed_ms": 18,
  "truncated": false,
  "window": {
    "hwnd": 197842,
    "title": "Google Chrome",
    "process": "chrome.exe",
    "dpi_scale": 1.25
  },
  "nodes": [
    {
      "id": "n_a1b2",
      "role": "Button",
      "name": "Submit",
      "bbox": { "x": 1420, "y": 310, "w": 88, "h": 32 },
      "anchor": { "x": 1464, "y": 326 },
      "states": ["enabled"]
    }
  ]
}
```
