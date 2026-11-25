use crate::error::{ProbeError, Result};
use crate::filter::{filter_node, TraversalBudget, DEFAULT_RAW_VISIT_LIMIT};
use crate::model::UiNode;
use crate::uia::browser::BrowserDomExtractor;
use crate::uia::cache::{build_traversal_cache_request, read_cached_node_data};
use crate::uia::{validate_hwnd, UiaSession};
use std::collections::HashSet;
use windows::Win32::UI::Accessibility::{IUIAutomationCacheRequest, IUIAutomationElement};

/// Options controlling a UIA tree extraction pass.
#[derive(Debug, Clone, Copy)]
pub struct ExtractOptions {
    pub depth: usize,
    pub max_nodes: usize,
    pub dom: bool,
    pub compact: bool,
}

/// Result of a UIA tree extraction pass.
#[derive(Debug, Clone)]
pub struct ExtractResult {
    pub nodes: Vec<UiNode>,
    pub truncated: bool,
}

/// Extracts filtered, coordinate-normalized interactive nodes from `hwnd`
/// using `IUIAutomationCacheRequest` batched reads.
pub fn extract_window_nodes(
    session: &UiaSession,
    hwnd: isize,
    opts: &ExtractOptions,
    browser: &dyn BrowserDomExtractor,
) -> Result<ExtractResult> {
    let raw_hwnd = validate_hwnd(hwnd)?;
    let cache_req = build_traversal_cache_request(&session.automation)?;

    // SAFETY: `ElementFromHandle` and `BuildUpdatedCache` are called on a validated HWND
    // with COM initialized in `UiaSession`.
    let root = unsafe {
        let raw_root = session
            .automation
            .ElementFromHandle(raw_hwnd)
            .map_err(|e| {
                ProbeError::UiaFailure(format!("ElementFromHandle({hwnd}) failed: {e}"))
            })?;
        raw_root
            .BuildUpdatedCache(&cache_req)
            .map_err(|e| ProbeError::UiaFailure(format!("BuildUpdatedCache(root) failed: {e}")))?
    };

    let mut budget = TraversalBudget::new(opts.max_nodes, DEFAULT_RAW_VISIT_LIMIT);
    let mut nodes = Vec::new();
    let mut seen_runtime_ids: HashSet<Vec<i32>> = HashSet::new();

    walk_element(
        &root,
        hwnd,
        0,
        "0",
        opts,
        browser,
        &cache_req,
        &mut budget,
        &mut seen_runtime_ids,
        &mut nodes,
    );

    Ok(ExtractResult {
        nodes,
        truncated: budget.truncated,
    })
}

#[allow(clippy::too_many_arguments)]
fn walk_element(
    elem: &IUIAutomationElement,
    hwnd: isize,
    depth: usize,
    path: &str,
    opts: &ExtractOptions,
    browser: &dyn BrowserDomExtractor,
    cache_req: &IUIAutomationCacheRequest,
    budget: &mut TraversalBudget,
    seen_runtime_ids: &mut HashSet<Vec<i32>>,
    out: &mut Vec<UiNode>,
) {
    if !budget.try_visit_raw() {
        return;
    }

    let Ok(data) = read_cached_node_data(elem) else {
        // Stale/dead element; skip gracefully without aborting the dump.
        return;
    };

    // When --dom is disabled, do not descend into embedded browser DOM subtrees.
    let is_dom_root = browser.is_dom_root(&data);
    if is_dom_root && !opts.dom {
        return;
    }

    if let Some(node) = filter_node(hwnd, &data, opts.compact, path) {
        let is_dup = !data.runtime_id.is_empty() && !seen_runtime_ids.insert(data.runtime_id);
        if !is_dup {
            if budget.can_emit() {
                out.push(node);
                budget.record_emit();
            } else {
                budget.mark_truncated();
                return;
            }
        }
    }

    if depth >= opts.depth || !budget.can_emit() {
        return;
    }

    // SAFETY: `GetCachedChildren` reads children pre-fetched by `TreeScope_Children`.
    let Ok(children) = (unsafe { elem.GetCachedChildren() }) else {
        return;
    };
    let Ok(len) = (unsafe { children.Length() }) else {
        return;
    };

    for i in 0..len {
        if !budget.can_emit() {
            budget.mark_truncated();
            break;
        }
        let Ok(child) = (unsafe { children.GetElement(i) }) else {
            continue;
        };
        // Only issue a BuildUpdatedCache call for children that may have descendants.
        let child_for_walk = if depth + 1 < opts.depth {
            match unsafe { child.BuildUpdatedCache(cache_req) } {
                Ok(updated) => updated,
                Err(_) => continue,
            }
        } else {
            child
        };

        let child_path = format!("{path}/{i}");
        walk_element(
            &child_for_walk,
            hwnd,
            depth + 1,
            &child_path,
            opts,
            browser,
            cache_req,
            budget,
            seen_runtime_ids,
            out,
        );
    }
}
