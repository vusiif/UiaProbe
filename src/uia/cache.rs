use crate::error::{ProbeError, Result};
use windows::core::Interface;
use windows::Win32::System::Com::SAFEARRAY;
use windows::Win32::System::Ole::{
    SafeArrayAccessData, SafeArrayGetLBound, SafeArrayGetUBound, SafeArrayUnaccessData,
};
use windows::Win32::System::Variant::{VT_ARRAY, VT_I4};
use windows::Win32::UI::Accessibility::{
    ExpandCollapseState_Expanded, ExpandCollapseState_PartiallyExpanded, IUIAutomation,
    IUIAutomationCacheRequest, IUIAutomationElement, IUIAutomationExpandCollapsePattern,
    IUIAutomationSelectionItemPattern, IUIAutomationTogglePattern, IUIAutomationValuePattern,
    ToggleState_On, TreeScope, TreeScope_Children, TreeScope_Element, UIA_AutomationIdPropertyId,
    UIA_BoundingRectanglePropertyId, UIA_ControlTypePropertyId, UIA_ExpandCollapsePatternId,
    UIA_HasKeyboardFocusPropertyId, UIA_IsEnabledPropertyId, UIA_IsOffscreenPropertyId,
    UIA_IsPasswordPropertyId, UIA_NamePropertyId, UIA_RuntimeIdPropertyId,
    UIA_SelectionItemPatternId, UIA_TogglePatternId, UIA_ValuePatternId,
};

/// Snapshot of a single UIA element read exclusively from an `IUIAutomationCacheRequest`.
#[derive(Debug, Clone)]
pub struct CachedNodeData {
    pub control_type: i32,
    pub name: String,
    pub automation_id: String,
    pub runtime_id: Vec<i32>,
    pub rect: (f64, f64, f64, f64),
    pub is_enabled: bool,
    pub is_offscreen: bool,
    pub has_focus: bool,
    pub is_password: bool,
    pub is_checked: bool,
    pub is_selected: bool,
    pub is_expanded: bool,
    pub is_readonly: bool,
}

/// Builds an `IUIAutomationCacheRequest` configured for `Element | Children` batched traversal.
pub fn build_traversal_cache_request(
    automation: &IUIAutomation,
) -> Result<IUIAutomationCacheRequest> {
    // SAFETY: Calling `IUIAutomation::CreateCacheRequest` and configuring property/pattern IDs.
    unsafe {
        let req = automation
            .CreateCacheRequest()
            .map_err(|e| ProbeError::UiaFailure(format!("CreateCacheRequest failed: {e}")))?;

        req.SetTreeScope(TreeScope(TreeScope_Element.0 | TreeScope_Children.0))
            .map_err(|e| ProbeError::UiaFailure(format!("SetTreeScope failed: {e}")))?;

        let properties = [
            UIA_NamePropertyId,
            UIA_AutomationIdPropertyId,
            UIA_ControlTypePropertyId,
            UIA_RuntimeIdPropertyId,
            UIA_BoundingRectanglePropertyId,
            UIA_IsEnabledPropertyId,
            UIA_IsOffscreenPropertyId,
            UIA_HasKeyboardFocusPropertyId,
            UIA_IsPasswordPropertyId,
        ];
        for prop in properties {
            req.AddProperty(prop).map_err(|e| {
                ProbeError::UiaFailure(format!("AddProperty({prop:?}) failed: {e}"))
            })?;
        }

        let patterns = [
            UIA_TogglePatternId,
            UIA_SelectionItemPatternId,
            UIA_ExpandCollapsePatternId,
            UIA_ValuePatternId,
        ];
        for pat in patterns {
            req.AddPattern(pat)
                .map_err(|e| ProbeError::UiaFailure(format!("AddPattern({pat:?}) failed: {e}")))?;
        }

        Ok(req)
    }
}

/// Reads all required properties and pattern states from an element's in-process cache.
///
/// Issues zero cross-process COM calls.
pub fn read_cached_node_data(elem: &IUIAutomationElement) -> Result<CachedNodeData> {
    // SAFETY: All `Cached*` and `GetCached*` calls read from the in-process UIA cache.
    unsafe {
        let control_type = elem
            .CachedControlType()
            .map(|ct| ct.0)
            .map_err(|e| ProbeError::UiaFailure(format!("CachedControlType failed: {e}")))?;

        let name = elem.CachedName().map(|b| b.to_string()).unwrap_or_default();

        let automation_id = elem
            .CachedAutomationId()
            .map(|b| b.to_string())
            .unwrap_or_default();

        let rect = elem
            .CachedBoundingRectangle()
            .map(|r| (r.left as f64, r.top as f64, r.right as f64, r.bottom as f64))
            .unwrap_or((0.0, 0.0, 0.0, 0.0));

        let is_enabled = elem.CachedIsEnabled().map(|b| b.as_bool()).unwrap_or(false);
        let is_offscreen = elem
            .CachedIsOffscreen()
            .map(|b| b.as_bool())
            .unwrap_or(false);
        let has_focus = elem
            .CachedHasKeyboardFocus()
            .map(|b| b.as_bool())
            .unwrap_or(false);
        let is_password = elem
            .CachedIsPassword()
            .map(|b| b.as_bool())
            .unwrap_or(false);

        let runtime_id = read_cached_runtime_id(elem);

        let is_checked = elem
            .GetCachedPattern(UIA_TogglePatternId)
            .ok()
            .and_then(|unk| unk.cast::<IUIAutomationTogglePattern>().ok())
            .and_then(|pat| pat.CachedToggleState().ok())
            .is_some_and(|st| st == ToggleState_On);

        let is_selected = elem
            .GetCachedPattern(UIA_SelectionItemPatternId)
            .ok()
            .and_then(|unk| unk.cast::<IUIAutomationSelectionItemPattern>().ok())
            .and_then(|pat| pat.CachedIsSelected().ok())
            .is_some_and(|b| b.as_bool());

        let is_expanded = elem
            .GetCachedPattern(UIA_ExpandCollapsePatternId)
            .ok()
            .and_then(|unk| unk.cast::<IUIAutomationExpandCollapsePattern>().ok())
            .and_then(|pat| pat.CachedExpandCollapseState().ok())
            .is_some_and(|st| {
                st == ExpandCollapseState_Expanded || st == ExpandCollapseState_PartiallyExpanded
            });

        let is_readonly = elem
            .GetCachedPattern(UIA_ValuePatternId)
            .ok()
            .and_then(|unk| unk.cast::<IUIAutomationValuePattern>().ok())
            .and_then(|pat| pat.CachedIsReadOnly().ok())
            .is_some_and(|b| b.as_bool());

        Ok(CachedNodeData {
            control_type,
            name,
            automation_id,
            runtime_id,
            rect,
            is_enabled,
            is_offscreen,
            has_focus,
            is_password,
            is_checked,
            is_selected,
            is_expanded,
            is_readonly,
        })
    }
}

unsafe fn read_cached_runtime_id(elem: &IUIAutomationElement) -> Vec<i32> {
    let Ok(variant) = elem.GetCachedPropertyValue(UIA_RuntimeIdPropertyId) else {
        return Vec::new();
    };
    let raw = variant.as_raw();
    let vt = raw.Anonymous.Anonymous.vt;
    if (vt & VT_ARRAY.0) == 0 || (vt & VT_I4.0) == 0 {
        return Vec::new();
    }
    let psa: *mut SAFEARRAY = raw.Anonymous.Anonymous.Anonymous.parray.cast::<SAFEARRAY>();
    if psa.is_null() {
        return Vec::new();
    }
    let Ok(lbound) = SafeArrayGetLBound(psa, 1) else {
        return Vec::new();
    };
    let Ok(ubound) = SafeArrayGetUBound(psa, 1) else {
        return Vec::new();
    };
    if ubound < lbound {
        return Vec::new();
    }
    let count = (ubound - lbound + 1) as usize;
    let mut ptr: *mut core::ffi::c_void = core::ptr::null_mut();
    if SafeArrayAccessData(psa, &mut ptr).is_err() || ptr.is_null() {
        return Vec::new();
    }
    let slice = core::slice::from_raw_parts(ptr as *const i32, count);
    let out = slice.to_vec();
    let _ = SafeArrayUnaccessData(psa);
    out
}
