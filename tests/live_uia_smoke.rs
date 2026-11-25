use uiaprobe::cli::parse_hwnd;
use uiaprobe::coords::init_dpi_awareness;
use uiaprobe::uia::browser::{BrowserDomExtractor, ChromiumDomExtractor};
use uiaprobe::uia::cache::CachedNodeData;
use uiaprobe::uia::standard::{extract_window_nodes, ExtractOptions};
use uiaprobe::uia::{foreground_hwnd, read_window_info, UiaSession};

#[test]
fn parse_hwnd_supports_decimal_and_hex() {
    assert_eq!(parse_hwnd("197842").unwrap(), 197842);
    assert_eq!(parse_hwnd("0x304d2").unwrap(), 0x304d2);
    assert_eq!(parse_hwnd("0X304D2").unwrap(), 0x304d2);
    assert!(parse_hwnd("0").is_err());
    assert!(parse_hwnd("not_a_hwnd").is_err());
}

#[test]
fn chromium_dom_extractor_detects_root_web_area() {
    let extractor = ChromiumDomExtractor;
    let mut node = CachedNodeData {
        control_type: 50030,
        name: "Example".to_string(),
        automation_id: "RootWebArea".to_string(),
        runtime_id: vec![],
        rect: (0.0, 0.0, 100.0, 100.0),
        is_enabled: true,
        is_offscreen: false,
        has_focus: false,
        is_password: false,
        is_checked: false,
        is_selected: false,
        is_expanded: false,
        is_readonly: false,
    };
    assert!(extractor.is_dom_root(&node));
    node.automation_id = "Toolbar".to_string();
    assert!(!extractor.is_dom_root(&node));
}

#[test]
fn live_active_window_dump_smoke() {
    init_dpi_awareness();
    let Ok(hwnd) = foreground_hwnd() else {
        // Non-interactive/headless CI session without a foreground window.
        return;
    };
    let info = read_window_info(hwnd).expect("foreground window metadata should be readable");
    assert_ne!(info.hwnd, 0);
    assert!(info.dpi_scale >= 1.0);

    let session = UiaSession::new().expect("COM + IUIAutomation should initialize");
    let opts = ExtractOptions {
        depth: 4,
        max_nodes: 50,
        dom: false,
        compact: true,
    };
    let res = extract_window_nodes(&session, hwnd, &opts, &ChromiumDomExtractor)
        .expect("live UIA traversal should succeed");
    assert!(res.nodes.len() <= 50);
}
