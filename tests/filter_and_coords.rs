use uiaprobe::coords::normalize_bbox;
use uiaprobe::filter::{filter_node, whitelist_role, TraversalBudget};
use uiaprobe::hash::short_node_id;
use uiaprobe::model::{BBox, DumpOutput, Point, UiNode, WindowInfo};
use uiaprobe::uia::cache::CachedNodeData;

fn sample_cached(control_type: i32, name: &str) -> CachedNodeData {
    CachedNodeData {
        control_type,
        name: name.to_string(),
        automation_id: String::new(),
        runtime_id: vec![42, 1, 7],
        rect: (100.0, 200.0, 180.0, 240.0),
        is_enabled: true,
        is_offscreen: false,
        has_focus: false,
        is_password: false,
        is_checked: false,
        is_selected: false,
        is_expanded: false,
        is_readonly: false,
    }
}

#[test]
fn normalize_bbox_rejects_zero_or_inverted_rects() {
    assert_eq!(normalize_bbox(10.0, 20.0, 10.0, 50.0), None);
    assert_eq!(normalize_bbox(10.0, 20.0, 50.0, 20.0), None);
    assert_eq!(normalize_bbox(100.0, 200.0, 50.0, 220.0), None);
    assert_eq!(normalize_bbox(f64::NAN, 0.0, 10.0, 10.0), None);
}

#[test]
fn normalize_bbox_preserves_negative_multimonitor_coords() {
    let (bbox, anchor) = normalize_bbox(-1920.0, -100.0, -1720.0, -40.0).unwrap();
    assert_eq!(
        bbox,
        BBox {
            x: -1920,
            y: -100,
            w: 200,
            h: 60
        }
    );
    assert_eq!(anchor, Point { x: -1820, y: -70 });
}

#[test]
fn short_node_id_is_deterministic_and_distinguishes_roles() {
    let a = short_node_id(197842, &[42, 1, 99], "Button", "");
    let b = short_node_id(197842, &[42, 1, 99], "Button", "");
    let c = short_node_id(197842, &[42, 1, 99], "Edit", "");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert!(a.starts_with("n_"));
    assert_eq!(a.len(), 6);
}

#[test]
fn dump_output_serializes_expected_schema() {
    let out = DumpOutput {
        timestamp: 1_774_340_000_000,
        elapsed_ms: 18,
        truncated: false,
        window: WindowInfo {
            hwnd: 197842,
            title: "Google Chrome".to_string(),
            process: "chrome.exe".to_string(),
            dpi_scale: 1.25,
        },
        nodes: vec![UiNode {
            id: "n_a1b2".to_string(),
            role: "Button".to_string(),
            name: "Submit".to_string(),
            bbox: BBox {
                x: 1420,
                y: 310,
                w: 88,
                h: 32,
            },
            anchor: Point { x: 1464, y: 326 },
            states: vec!["enabled".to_string()],
        }],
    };
    let v: serde_json::Value = serde_json::to_value(&out).unwrap();
    assert_eq!(v["timestamp"], 1_774_340_000_000u64);
    assert_eq!(v["elapsed_ms"], 18);
    assert_eq!(v["truncated"], false);
    assert_eq!(v["window"]["dpi_scale"], 1.25);
    assert_eq!(v["nodes"][0]["anchor"]["x"], 1464);
}

#[test]
fn whitelist_covers_all_12_roles_and_rejects_containers() {
    let expected = [
        (50000, "Button"),
        (50002, "CheckBox"),
        (50003, "ComboBox"),
        (50004, "Edit"),
        (50005, "Hyperlink"),
        (50007, "ListItem"),
        (50011, "MenuItem"),
        (50013, "RadioButton"),
        (50015, "Slider"),
        (50019, "TabItem"),
        (50024, "TreeItem"),
        (50031, "SplitButton"),
    ];
    for (id, role) in expected {
        assert_eq!(whitelist_role(id), Some(role));
    }
    assert_eq!(whitelist_role(50033), None); // Pane
    assert_eq!(whitelist_role(50026), None); // Group
}

#[test]
fn filter_node_handles_compact_states_offscreen_and_passwords() {
    let mut data = sample_cached(50000, "  Save  ");
    data.has_focus = true;
    data.is_checked = true;

    let normal = filter_node(1, &data, false, "0").unwrap();
    assert_eq!(normal.name, "Save");
    assert_eq!(normal.states, vec!["enabled", "focused", "checked"]);

    let compact = filter_node(1, &data, true, "0").unwrap();
    assert_eq!(compact.states, vec!["focused", "checked"]);

    let mut pw = sample_cached(50004, "secret123");
    pw.is_password = true;
    pw.is_readonly = true;
    let pw_node = filter_node(1, &pw, true, "0/1").unwrap();
    assert_eq!(pw_node.name, "***");
    assert_eq!(pw_node.states, vec!["readonly"]);

    let mut offscreen = sample_cached(50000, "Hidden");
    offscreen.is_offscreen = true;
    assert_eq!(filter_node(1, &offscreen, false, "0/2"), None);
}

#[test]
fn traversal_budget_enforces_raw_and_output_caps() {
    let mut budget = TraversalBudget::new(2, 5);
    assert!(budget.try_visit_raw());
    assert!(budget.can_emit());
    budget.record_emit();
    assert!(budget.try_visit_raw());
    budget.record_emit();
    assert!(!budget.can_emit());
    assert!(!budget.try_visit_raw());
    assert!(budget.truncated);

    let mut raw_budget = TraversalBudget::new(100, 2);
    assert!(raw_budget.try_visit_raw());
    assert!(raw_budget.try_visit_raw());
    assert!(!raw_budget.try_visit_raw());
    assert!(raw_budget.truncated);
}
