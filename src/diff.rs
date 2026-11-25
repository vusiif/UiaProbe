use crate::model::{UiNode, WindowInfo};
use serde::Serialize;
use std::collections::HashMap;

/// A node that kept the same `id` between two snapshots but changed properties.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NodeDiff {
    pub id: String,
    pub before: UiNode,
    pub after: UiNode,
}

/// Structural delta between a baseline snapshot and the current snapshot.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DiffSummary {
    pub added: Vec<UiNode>,
    pub removed: Vec<UiNode>,
    pub changed: Vec<NodeDiff>,
}

/// JSON payload emitted when `--diff` (or `stdio` `diff: true`) is active.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DiffOutput {
    pub timestamp: u64,
    pub elapsed_ms: u64,
    pub truncated: bool,
    pub window: WindowInfo,
    pub diff: DiffSummary,
}

/// Computes added, removed, and changed nodes between `previous` and `current` snapshots.
pub fn compute_diff(previous: &[UiNode], current: &[UiNode]) -> DiffSummary {
    let mut prev_map: HashMap<&str, &UiNode> = HashMap::with_capacity(previous.len());
    for node in previous {
        prev_map.insert(node.id.as_str(), node);
    }

    let mut added = Vec::new();
    let mut changed = Vec::new();

    for curr in current {
        match prev_map.remove(curr.id.as_str()) {
            None => added.push(curr.clone()),
            Some(prev) if prev != curr => {
                changed.push(NodeDiff {
                    id: curr.id.clone(),
                    before: prev.clone(),
                    after: curr.clone(),
                });
            }
            Some(_) => {}
        }
    }

    let mut removed: Vec<UiNode> = prev_map.into_values().cloned().collect();
    removed.sort_by(|a, b| a.id.cmp(&b.id));

    DiffSummary {
        added,
        removed,
        changed,
    }
}
