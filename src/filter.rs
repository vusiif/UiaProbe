use crate::coords::normalize_bbox;
use crate::hash::short_node_id;
use crate::model::UiNode;
use crate::uia::cache::CachedNodeData;

/// Internal hard circuit-breaker limit on total raw UIA elements visited.
pub const DEFAULT_RAW_VISIT_LIMIT: usize = 3000;

/// Declarative whitelist mapping from Win32 `UIA_ControlTypeId` to UiaProbe role names.
///
/// Covers the 12 interactive roles defined in `SPEC.md`:
/// `Button`, `Edit`, `CheckBox`, `RadioButton`, `ComboBox`, `MenuItem`,
/// `TabItem`, `Hyperlink`, `TreeItem`, `ListItem`, `Slider`, `SplitButton`.
pub fn whitelist_role(control_type: i32) -> Option<&'static str> {
    match control_type {
        50000 => Some("Button"),
        50002 => Some("CheckBox"),
        50003 => Some("ComboBox"),
        50004 => Some("Edit"),
        50005 => Some("Hyperlink"),
        50007 => Some("ListItem"),
        50011 => Some("MenuItem"),
        50013 => Some("RadioButton"),
        50015 => Some("Slider"),
        50019 => Some("TabItem"),
        50024 => Some("TreeItem"),
        50031 => Some("SplitButton"),
        _ => None,
    }
}

/// Dual-cap traversal budget enforcing both output node count (`--max-nodes`)
/// and raw visited element circuit breaking (`3000`).
#[derive(Debug, Clone)]
pub struct TraversalBudget {
    max_nodes: usize,
    raw_limit: usize,
    emitted: usize,
    raw_visited: usize,
    pub truncated: bool,
}

impl TraversalBudget {
    pub fn new(max_nodes: usize, raw_limit: usize) -> Self {
        Self {
            max_nodes: max_nodes.max(1),
            raw_limit: raw_limit.max(1),
            emitted: 0,
            raw_visited: 0,
            truncated: false,
        }
    }

    /// Registers one raw visited element. Returns `false` if the raw circuit breaker tripped.
    pub fn try_visit_raw(&mut self) -> bool {
        if self.raw_visited >= self.raw_limit || self.emitted >= self.max_nodes {
            self.truncated = true;
            return false;
        }
        self.raw_visited += 1;
        true
    }

    /// Returns `true` if another filtered node can be emitted within `max_nodes`.
    pub fn can_emit(&self) -> bool {
        self.emitted < self.max_nodes
    }

    /// Registers one emitted node. Marks `truncated` if the cap is now reached.
    pub fn record_emit(&mut self) {
        self.emitted += 1;
    }

    /// Marks traversal as truncated when an output-eligible node is dropped due to cap.
    pub fn mark_truncated(&mut self) {
        self.truncated = true;
    }
}

/// Evaluates a cached UIA node against the declarative role whitelist, visibility,
/// bounding box, and compact state rules.
pub fn filter_node(
    hwnd: isize,
    data: &CachedNodeData,
    compact: bool,
    fallback_path: &str,
) -> Option<UiNode> {
    if data.is_offscreen {
        return None;
    }
    let role = whitelist_role(data.control_type)?;
    let (bbox, anchor) = normalize_bbox(data.rect.0, data.rect.1, data.rect.2, data.rect.3)?;

    let name = if data.is_password {
        "***".to_string()
    } else {
        data.name.trim().to_string()
    };

    let states = collect_states(data, compact);
    let id = short_node_id(hwnd, &data.runtime_id, role, fallback_path);

    Some(UiNode {
        id,
        role: role.to_string(),
        name,
        bbox,
        anchor,
        states,
    })
}

fn collect_states(data: &CachedNodeData, compact: bool) -> Vec<String> {
    let mut states = Vec::new();
    if data.is_enabled && !compact {
        states.push("enabled".to_string());
    }
    if data.has_focus {
        states.push("focused".to_string());
    }
    if data.is_checked {
        states.push("checked".to_string());
    }
    if data.is_selected {
        states.push("selected".to_string());
    }
    if data.is_expanded {
        states.push("expanded".to_string());
    }
    if data.is_readonly {
        states.push("readonly".to_string());
    }
    states
}
