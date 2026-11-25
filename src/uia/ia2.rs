use crate::coords::normalize_bbox;
use crate::error::{ProbeError, Result};
use crate::filter::{TraversalBudget, DEFAULT_RAW_VISIT_LIMIT};
use crate::hash::short_node_id;
use crate::model::UiNode;
use crate::uia::validate_hwnd;
use windows::core::{Interface, VARIANT};
use windows::Win32::System::Com::IDispatch;
use windows::Win32::System::Variant::{VT_DISPATCH, VT_I4};
use windows::Win32::UI::Accessibility::{
    AccessibleChildren, AccessibleObjectFromWindow, IAccessible,
};
use windows::Win32::UI::WindowsAndMessaging::OBJID_CLIENT;

const CHILDID_SELF: i32 = 0;

// MSAA Role constants (oleacc.h)
const ROLE_SYSTEM_DOCUMENT: i32 = 0x0F;
const ROLE_SYSTEM_LINK: i32 = 0x1E;
const ROLE_SYSTEM_LISTITEM: i32 = 0x22;
const ROLE_SYSTEM_OUTLINEITEM: i32 = 0x24;
const ROLE_SYSTEM_PAGETAB: i32 = 0x25;
const ROLE_SYSTEM_TEXT: i32 = 0x2A;
const ROLE_SYSTEM_PUSHBUTTON: i32 = 0x2B;
const ROLE_SYSTEM_CHECKBUTTON: i32 = 0x2C;
const ROLE_SYSTEM_RADIOBUTTON: i32 = 0x2D;
const ROLE_SYSTEM_COMBOBOX: i32 = 0x2E;
const ROLE_SYSTEM_DROPLIST: i32 = 0x2F;
const ROLE_SYSTEM_SLIDER: i32 = 0x33;
const ROLE_SYSTEM_MENUITEM: i32 = 0x0C;
const ROLE_SYSTEM_BUTTONDROPDOWN: i32 = 0x38;
const ROLE_SYSTEM_SPLITBUTTON: i32 = 0x3E;

// MSAA State flags (oleacc.h)
const STATE_SYSTEM_UNAVAILABLE: i32 = 0x0000_0001;
const STATE_SYSTEM_SELECTED: i32 = 0x0000_0002;
const STATE_SYSTEM_FOCUSED: i32 = 0x0000_0004;
const STATE_SYSTEM_CHECKED: i32 = 0x0000_0010;
const STATE_SYSTEM_EXPANDED: i32 = 0x0000_0200;
const STATE_SYSTEM_READONLY: i32 = 0x0000_0040;
const STATE_SYSTEM_INVISIBLE: i32 = 0x0000_8000;
const STATE_SYSTEM_OFFSCREEN: i32 = 0x0001_0000;
const STATE_SYSTEM_PROTECTED: i32 = 0x2000_0000;

/// Returns `true` if `process` is a known Gecko/Firefox-based browser.
pub fn is_gecko_process(process: &str) -> bool {
    matches!(
        process.to_ascii_lowercase().as_str(),
        "firefox.exe" | "librewolf.exe" | "waterfox.exe" | "zen.exe" | "floorp.exe"
    )
}

/// Maps MSAA `ROLE_SYSTEM_*` constants to the UiaProbe 12-role whitelist.
pub fn msaa_role_to_whitelist(role: i32) -> Option<&'static str> {
    match role {
        ROLE_SYSTEM_PUSHBUTTON | ROLE_SYSTEM_BUTTONDROPDOWN => Some("Button"),
        ROLE_SYSTEM_SPLITBUTTON => Some("SplitButton"),
        ROLE_SYSTEM_TEXT => Some("Edit"),
        ROLE_SYSTEM_CHECKBUTTON => Some("CheckBox"),
        ROLE_SYSTEM_RADIOBUTTON => Some("RadioButton"),
        ROLE_SYSTEM_COMBOBOX | ROLE_SYSTEM_DROPLIST => Some("ComboBox"),
        ROLE_SYSTEM_MENUITEM => Some("MenuItem"),
        ROLE_SYSTEM_PAGETAB => Some("TabItem"),
        ROLE_SYSTEM_LINK => Some("Hyperlink"),
        ROLE_SYSTEM_OUTLINEITEM => Some("TreeItem"),
        ROLE_SYSTEM_LISTITEM => Some("ListItem"),
        ROLE_SYSTEM_SLIDER => Some("Slider"),
        _ => None,
    }
}

/// Extracts interactive web DOM nodes from a Gecko/Firefox window via `oleacc.dll` (`IAccessible`).
pub fn extract_gecko_ia2_nodes(
    hwnd: isize,
    max_depth: usize,
    max_nodes: usize,
    compact: bool,
) -> Result<(Vec<UiNode>, bool)> {
    let raw_hwnd = validate_hwnd(hwnd)?;
    let mut ptr: *mut core::ffi::c_void = core::ptr::null_mut();

    // SAFETY: `AccessibleObjectFromWindow` initializes `ptr` with an `IAccessible` COM pointer.
    let root: IAccessible = unsafe {
        AccessibleObjectFromWindow(raw_hwnd, OBJID_CLIENT.0 as u32, &IAccessible::IID, &mut ptr)
            .map_err(|e| {
                ProbeError::UiaFailure(format!("AccessibleObjectFromWindow failed: {e}"))
            })?;
        if ptr.is_null() {
            return Ok((Vec::new(), false));
        }
        IAccessible::from_raw(ptr)
    };

    let mut budget = TraversalBudget::new(max_nodes, DEFAULT_RAW_VISIT_LIMIT);
    let mut nodes = Vec::new();
    walk_ia2(
        &root,
        CHILDID_SELF,
        hwnd,
        0,
        max_depth,
        false,
        "ia2",
        compact,
        &mut budget,
        &mut nodes,
    );

    Ok((nodes, budget.truncated))
}

#[allow(clippy::too_many_arguments)]
fn walk_ia2(
    acc: &IAccessible,
    child_id: i32,
    hwnd: isize,
    depth: usize,
    max_depth: usize,
    in_document: bool,
    path: &str,
    compact: bool,
    budget: &mut TraversalBudget,
    out: &mut Vec<UiNode>,
) {
    if !budget.try_visit_raw() {
        return;
    }

    let var_child = VARIANT::from(child_id);
    // SAFETY: Querying MSAA properties on a valid `IAccessible` COM object.
    let role = unsafe { acc.get_accRole(&var_child) }
        .ok()
        .and_then(|v| variant_to_i32(&v))
        .unwrap_or(0);

    let state = unsafe { acc.get_accState(&var_child) }
        .ok()
        .and_then(|v| variant_to_i32(&v))
        .unwrap_or(0);

    if (state & (STATE_SYSTEM_INVISIBLE | STATE_SYSTEM_OFFSCREEN)) != 0 {
        return;
    }

    let now_in_doc = in_document || role == ROLE_SYSTEM_DOCUMENT;

    if now_in_doc {
        if let Some(mapped_role) = msaa_role_to_whitelist(role) {
            let mut left = 0i32;
            let mut top = 0i32;
            let mut width = 0i32;
            let mut height = 0i32;
            // SAFETY: `accLocation` writes screen coordinates into the 4 output pointers.
            if unsafe { acc.accLocation(&mut left, &mut top, &mut width, &mut height, &var_child) }
                .is_ok()
            {
                if let Some((bbox, anchor)) = normalize_bbox(
                    left as f64,
                    top as f64,
                    (left + width) as f64,
                    (top + height) as f64,
                ) {
                    let name = if (state & STATE_SYSTEM_PROTECTED) != 0 {
                        "***".to_string()
                    } else {
                        // SAFETY: `get_accName` returns an optional BSTR.
                        unsafe { acc.get_accName(&var_child) }
                            .map(|b| b.to_string().trim().to_string())
                            .unwrap_or_default()
                    };

                    let states = collect_msaa_states(state, compact);
                    let id = short_node_id(hwnd, &[], mapped_role, path);
                    if budget.can_emit() {
                        out.push(UiNode {
                            id,
                            role: mapped_role.to_string(),
                            name,
                            bbox,
                            anchor,
                            states,
                        });
                        budget.record_emit();
                    } else {
                        budget.mark_truncated();
                        return;
                    }
                }
            }
        }
    }

    // Simple child IDs (non-zero) have no further MSAA descendants.
    if child_id != CHILDID_SELF || depth >= max_depth || !budget.can_emit() {
        return;
    }

    // SAFETY: Reading child count and fetching children via `AccessibleChildren`.
    let count = unsafe { acc.accChildCount() }.unwrap_or(0);
    if count <= 0 {
        return;
    }
    let fetch_count = (count as usize).min(512);
    let mut children = vec![VARIANT::default(); fetch_count];
    let mut obtained = 0i32;
    if unsafe { AccessibleChildren(acc, 0, &mut children, &mut obtained) }.is_err() {
        return;
    }

    for (idx, var) in children
        .into_iter()
        .take(obtained.max(0) as usize)
        .enumerate()
    {
        if !budget.can_emit() {
            budget.mark_truncated();
            break;
        }
        let child_path = format!("{path}/{idx}");
        let raw = var.as_raw();
        // SAFETY: Reading the discriminant `vt` of an initialized `VARIANT`.
        let vt = unsafe { raw.Anonymous.Anonymous.vt };
        if vt == VT_DISPATCH.0 {
            // SAFETY: `pdispVal` is a valid `IDispatch` when `vt == VT_DISPATCH`.
            let disp_ptr = unsafe { raw.Anonymous.Anonymous.Anonymous.pdispVal };
            if !disp_ptr.is_null() {
                let disp = unsafe { IDispatch::from_raw_borrowed(&disp_ptr) };
                if let Some(Ok(child_acc)) = disp.map(|d| d.cast::<IAccessible>()) {
                    walk_ia2(
                        &child_acc,
                        CHILDID_SELF,
                        hwnd,
                        depth + 1,
                        max_depth,
                        now_in_doc,
                        &child_path,
                        compact,
                        budget,
                        out,
                    );
                }
            }
        } else if vt == VT_I4.0 {
            // SAFETY: `lVal` holds the integer child ID when `vt == VT_I4`.
            let cid = unsafe { raw.Anonymous.Anonymous.Anonymous.lVal };
            if cid != CHILDID_SELF {
                walk_ia2(
                    acc,
                    cid,
                    hwnd,
                    depth + 1,
                    max_depth,
                    now_in_doc,
                    &child_path,
                    compact,
                    budget,
                    out,
                );
            }
        }
    }
}

fn variant_to_i32(var: &VARIANT) -> Option<i32> {
    let raw = var.as_raw();
    // SAFETY: Reading `vt` and `lVal` on an initialized `VARIANT` when `vt == VT_I4`.
    unsafe {
        if raw.Anonymous.Anonymous.vt == VT_I4.0 {
            Some(raw.Anonymous.Anonymous.Anonymous.lVal)
        } else {
            None
        }
    }
}

fn collect_msaa_states(state: i32, compact: bool) -> Vec<String> {
    let mut out = Vec::new();
    let enabled = (state & STATE_SYSTEM_UNAVAILABLE) == 0;
    if enabled && !compact {
        out.push("enabled".to_string());
    }
    if (state & STATE_SYSTEM_FOCUSED) != 0 {
        out.push("focused".to_string());
    }
    if (state & STATE_SYSTEM_CHECKED) != 0 {
        out.push("checked".to_string());
    }
    if (state & STATE_SYSTEM_SELECTED) != 0 {
        out.push("selected".to_string());
    }
    if (state & STATE_SYSTEM_EXPANDED) != 0 {
        out.push("expanded".to_string());
    }
    if (state & STATE_SYSTEM_READONLY) != 0 {
        out.push("readonly".to_string());
    }
    out
}
