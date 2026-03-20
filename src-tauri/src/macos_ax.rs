use core::ffi::c_void;
use std::ptr;
use std::ptr::NonNull;

use objc2_app_kit::NSRunningApplication;
use objc2_application_services::{AXError, AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{CFBoolean, CFNumber, CFRetained, CFString, CFType, CGPoint, CGSize};

const TEXT_ROLES: [&str; 3] = ["AXTextField", "AXTextArea", "AXTextView"];
const BLOCKED_TERMS: [&str; 11] = [
    "address",
    "url",
    "navigation",
    "omnibox",
    "search",
    "buscar",
    "password",
    "contraseña",
    "contrasena",
    "email",
    "correo",
];
const BROWSER_BUNDLES: [&str; 6] = [
    "com.apple.Safari",
    "com.google.Chrome",
    "com.brave.Browser",
    "com.microsoft.edgemac",
    "org.mozilla.firefox",
    "company.thebrowser.Browser",
];

#[derive(Debug, Clone, Default)]
pub struct AxProbeDiagnostics {
    pub pid: Option<i32>,
    pub bundle_id: Option<String>,
    pub role: Option<String>,
    pub subrole: Option<String>,
    pub dom_input_type: Option<String>,
    pub native_error: Option<String>,
}

impl AxProbeDiagnostics {
    fn push_error(&mut self, context: &str, error: AXError) {
        if self.native_error.is_none() {
            self.native_error = Some(format!("{context}:{error:?}"));
        }
    }

    fn push_custom_error(&mut self, context: &str) {
        if self.native_error.is_none() {
            self.native_error = Some(context.to_string());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxProbeSnapshot {
    pub bundle_id: Option<String>,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxProbeSkipReason {
    InternalBundle,
    MissingFocusedElement,
    RoleNotTextOrEditable(String),
    BlockedSearchSubrole,
    BlockedDomInputType(String),
    BlockedBrowserMetadata(String),
    MissingGeometry,
    TinyGeometry { w: i32, h: i32 },
    BlockedSecureRole(String),
    BlockedPasswordMetadata,
}

impl AxProbeSkipReason {
    pub fn as_reason(&self) -> String {
        match self {
            Self::InternalBundle => "internal_bundle".to_string(),
            Self::MissingFocusedElement => "missing_focused_element".to_string(),
            Self::RoleNotTextOrEditable(role) => format!("role_not_text_or_editable:{role}"),
            Self::BlockedSearchSubrole => "blocked_search_subrole".to_string(),
            Self::BlockedDomInputType(value) => format!("blocked_dom_input_type:{value}"),
            Self::BlockedBrowserMetadata(term) => format!("blocked_browser_metadata:{term}"),
            Self::MissingGeometry => "missing_geometry".to_string(),
            Self::TinyGeometry { w, h } => format!("tiny_geometry:{w}x{h}"),
            Self::BlockedSecureRole(role) => format!("blocked_secure_role:{role}"),
            Self::BlockedPasswordMetadata => "blocked_password_metadata".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AxProbeDecision {
    Show(AxProbeSnapshot),
    Hide(AxProbeSkipReason),
}

#[derive(Debug, Clone)]
pub struct AxProbeOutput {
    pub decision: AxProbeDecision,
    pub fallback_eligible: bool,
    pub diagnostics: AxProbeDiagnostics,
}

#[derive(Debug, Clone)]
struct AxClassificationCandidate {
    bundle_id: Option<String>,
    role: Option<String>,
    subrole: Option<String>,
    editable: bool,
    dom_input_type: Option<String>,
    metadata_text: String,
    geometry: Option<(i32, i32, i32, i32)>,
}

pub fn probe_focused_anchor_snapshot() -> AxProbeOutput {
    let mut diagnostics = AxProbeDiagnostics::default();
    let system_wide = unsafe { AXUIElement::new_system_wide() };

    let Some(focused_element) = resolve_focused_element(&system_wide, &mut diagnostics) else {
        return skip_output(AxProbeSkipReason::MissingFocusedElement, true, diagnostics);
    };

    diagnostics.pid = read_element_pid(&focused_element, &mut diagnostics);
    diagnostics.bundle_id = diagnostics.pid.and_then(bundle_id_for_pid);

    let role = read_optional_string_attribute(&focused_element, "AXRole", &mut diagnostics);
    diagnostics.role = role.clone();

    let editable = read_optional_bool_attribute(&focused_element, "AXEditable", &mut diagnostics)
        .unwrap_or(false);

    let subrole = read_optional_string_attribute(&focused_element, "AXSubrole", &mut diagnostics);
    diagnostics.subrole = subrole.clone();

    let dom_input_type =
        read_optional_string_attribute(&focused_element, "AXDOMInputType", &mut diagnostics)
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty());
    diagnostics.dom_input_type = dom_input_type.clone();

    let metadata_text = collect_metadata_text(&focused_element, &mut diagnostics);

    let geometry = match (
        read_position(&focused_element, &mut diagnostics),
        read_size(&focused_element, &mut diagnostics),
    ) {
        (Some(position), Some(size)) => Some((
            position.x.round() as i32,
            position.y.round() as i32,
            size.width.round() as i32,
            size.height.round() as i32,
        )),
        _ => None,
    };

    let candidate = AxClassificationCandidate {
        bundle_id: diagnostics.bundle_id.clone(),
        role,
        subrole,
        editable,
        dom_input_type,
        metadata_text,
        geometry,
    };

    let decision = classify_candidate(&candidate);

    match decision {
        AxProbeDecision::Show(snapshot) => AxProbeOutput {
            decision: AxProbeDecision::Show(snapshot),
            fallback_eligible: false,
            diagnostics,
        },
        AxProbeDecision::Hide(reason) => {
            let fallback_eligible = matches!(
                reason,
                AxProbeSkipReason::MissingFocusedElement | AxProbeSkipReason::MissingGeometry
            );
            AxProbeOutput {
                decision: AxProbeDecision::Hide(reason),
                fallback_eligible,
                diagnostics,
            }
        }
    }
}

fn classify_candidate(candidate: &AxClassificationCandidate) -> AxProbeDecision {
    if candidate
        .bundle_id
        .as_deref()
        .map(is_internal_bundle_id)
        .unwrap_or(false)
    {
        return AxProbeDecision::Hide(AxProbeSkipReason::InternalBundle);
    }

    let role = candidate
        .role
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("unknown");

    if !is_text_role(Some(role)) && !candidate.editable {
        return AxProbeDecision::Hide(AxProbeSkipReason::RoleNotTextOrEditable(role.to_string()));
    }

    if candidate
        .subrole
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("AXSearchField"))
    {
        return AxProbeDecision::Hide(AxProbeSkipReason::BlockedSearchSubrole);
    }

    if let Some(dom_input_type) = candidate.dom_input_type.as_deref() {
        if matches!(dom_input_type, "search" | "password" | "email") {
            return AxProbeDecision::Hide(AxProbeSkipReason::BlockedDomInputType(
                dom_input_type.to_string(),
            ));
        }
    }

    if should_apply_browser_metadata_block(candidate.bundle_id.as_deref()) {
        if let Some(term) = contains_blocked_browser_term(&candidate.metadata_text) {
            return AxProbeDecision::Hide(AxProbeSkipReason::BlockedBrowserMetadata(term));
        }
    }

    let Some((x, y, w, h)) = candidate.geometry else {
        return AxProbeDecision::Hide(AxProbeSkipReason::MissingGeometry);
    };

    if w < 2 || h < 2 {
        return AxProbeDecision::Hide(AxProbeSkipReason::TinyGeometry { w, h });
    }

    if role.to_lowercase().contains("secure") {
        return AxProbeDecision::Hide(AxProbeSkipReason::BlockedSecureRole(role.to_string()));
    }

    if candidate.metadata_text.contains("password") {
        return AxProbeDecision::Hide(AxProbeSkipReason::BlockedPasswordMetadata);
    }

    AxProbeDecision::Show(AxProbeSnapshot {
        bundle_id: candidate.bundle_id.clone(),
        x,
        y,
        w,
        h,
    })
}

fn skip_output(
    reason: AxProbeSkipReason,
    fallback_eligible: bool,
    diagnostics: AxProbeDiagnostics,
) -> AxProbeOutput {
    AxProbeOutput {
        decision: AxProbeDecision::Hide(reason),
        fallback_eligible,
        diagnostics,
    }
}

fn is_internal_bundle_id(bundle_id: &str) -> bool {
    let clean = bundle_id.trim().to_lowercase();
    clean.is_empty() || clean.contains("whisloai") || clean.contains("com.whisloai.app")
}

fn is_text_role(role: Option<&str>) -> bool {
    let Some(role) = role else {
        return false;
    };
    TEXT_ROLES
        .iter()
        .any(|value| value.eq_ignore_ascii_case(role.trim()))
}

fn should_apply_browser_metadata_block(bundle_id: Option<&str>) -> bool {
    let Some(bundle_id) = bundle_id else {
        return false;
    };

    BROWSER_BUNDLES
        .iter()
        .any(|value| value.eq_ignore_ascii_case(bundle_id))
}

fn contains_blocked_browser_term(metadata_text: &str) -> Option<String> {
    if metadata_text.trim().is_empty() {
        return None;
    }

    BLOCKED_TERMS
        .iter()
        .find(|term| metadata_text.contains(**term))
        .map(|term| (*term).to_string())
}

fn collect_metadata_text(element: &AXUIElement, diagnostics: &mut AxProbeDiagnostics) -> String {
    let mut chunks = Vec::new();
    for attribute_name in [
        "AXTitle",
        "AXDescription",
        "AXHelp",
        "AXPlaceholderValue",
        "AXIdentifier",
        "AXRoleDescription",
    ] {
        if let Some(value) = read_optional_string_attribute(element, attribute_name, diagnostics) {
            if !value.trim().is_empty() {
                chunks.push(value.trim().to_lowercase());
            }
        }
    }

    chunks.join(" ")
}

fn resolve_focused_element(
    system_wide: &AXUIElement,
    diagnostics: &mut AxProbeDiagnostics,
) -> Option<CFRetained<AXUIElement>> {
    if let Some(value) =
        copy_attribute_as_ax_element(system_wide, "AXFocusedUIElement", diagnostics)
    {
        return Some(value);
    }

    let focused_application =
        copy_attribute_as_ax_element(system_wide, "AXFocusedApplication", diagnostics)?;

    if let Some(value) =
        copy_attribute_as_ax_element(&focused_application, "AXFocusedUIElement", diagnostics)
    {
        return Some(value);
    }

    let focused_window =
        copy_attribute_as_ax_element(&focused_application, "AXFocusedWindow", diagnostics)?;
    copy_attribute_as_ax_element(&focused_window, "AXFocusedUIElement", diagnostics)
}

fn copy_attribute_as_ax_element(
    element: &AXUIElement,
    attribute_name: &str,
    diagnostics: &mut AxProbeDiagnostics,
) -> Option<CFRetained<AXUIElement>> {
    let value = match copy_attribute_value(element, attribute_name) {
        Ok(Some(value)) => value,
        Ok(None) => return None,
        Err(error) => {
            diagnostics.push_error(attribute_name, error);
            return None;
        }
    };

    match value.downcast::<AXUIElement>() {
        Ok(value) => Some(value),
        Err(_) => {
            diagnostics.push_custom_error(&format!("{attribute_name}:not_ax_ui_element"));
            None
        }
    }
}

fn copy_attribute_value(
    element: &AXUIElement,
    attribute: &str,
) -> Result<Option<CFRetained<CFType>>, AXError> {
    let attribute_name = CFString::from_str(attribute);
    let mut value_ptr: *const CFType = ptr::null();
    let status = unsafe {
        element.copy_attribute_value(attribute_name.as_ref(), NonNull::from(&mut value_ptr))
    };

    match status {
        AXError::Success => {
            let Some(value_ptr) = NonNull::new(value_ptr as *mut CFType) else {
                return Ok(None);
            };
            let retained = unsafe { CFRetained::from_raw(value_ptr) };
            Ok(Some(retained))
        }
        AXError::NoValue | AXError::AttributeUnsupported => Ok(None),
        other => Err(other),
    }
}

fn read_optional_string_attribute(
    element: &AXUIElement,
    attribute_name: &str,
    diagnostics: &mut AxProbeDiagnostics,
) -> Option<String> {
    match copy_attribute_value(element, attribute_name) {
        Ok(Some(value)) => cf_type_to_string(value.as_ref()),
        Ok(None) => None,
        Err(error) => {
            diagnostics.push_error(attribute_name, error);
            None
        }
    }
}

fn read_optional_bool_attribute(
    element: &AXUIElement,
    attribute_name: &str,
    diagnostics: &mut AxProbeDiagnostics,
) -> Option<bool> {
    match copy_attribute_value(element, attribute_name) {
        Ok(Some(value)) => cf_type_to_bool(value.as_ref()),
        Ok(None) => None,
        Err(error) => {
            diagnostics.push_error(attribute_name, error);
            None
        }
    }
}

fn read_element_pid(element: &AXUIElement, diagnostics: &mut AxProbeDiagnostics) -> Option<i32> {
    let mut pid = -1_i32;
    let status = unsafe { element.pid(NonNull::from(&mut pid)) };
    if status == AXError::Success {
        if pid > 0 {
            Some(pid)
        } else {
            None
        }
    } else {
        diagnostics.push_error("AXUIElementGetPid", status);
        None
    }
}

fn bundle_id_for_pid(pid: i32) -> Option<String> {
    let application = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    application
        .bundleIdentifier()
        .map(|value| value.to_string())
}

fn read_position(element: &AXUIElement, diagnostics: &mut AxProbeDiagnostics) -> Option<CGPoint> {
    match copy_attribute_value(element, "AXPosition") {
        Ok(Some(value)) => cf_type_to_point(value.as_ref()),
        Ok(None) => None,
        Err(error) => {
            diagnostics.push_error("AXPosition", error);
            None
        }
    }
}

fn read_size(element: &AXUIElement, diagnostics: &mut AxProbeDiagnostics) -> Option<CGSize> {
    match copy_attribute_value(element, "AXSize") {
        Ok(Some(value)) => cf_type_to_size(value.as_ref()),
        Ok(None) => None,
        Err(error) => {
            diagnostics.push_error("AXSize", error);
            None
        }
    }
}

fn cf_type_to_string(value: &CFType) -> Option<String> {
    if let Some(cf_string) = value.downcast_ref::<CFString>() {
        return Some(cf_string.to_string());
    }

    if let Some(cf_bool) = value.downcast_ref::<CFBoolean>() {
        return Some(if cf_bool.as_bool() {
            "true".to_string()
        } else {
            "false".to_string()
        });
    }

    if let Some(cf_number) = value.downcast_ref::<CFNumber>() {
        if let Some(number) = cf_number.as_i64() {
            return Some(number.to_string());
        }
        if let Some(number) = cf_number.as_f64() {
            return Some(number.to_string());
        }
    }

    None
}

fn cf_type_to_bool(value: &CFType) -> Option<bool> {
    if let Some(cf_bool) = value.downcast_ref::<CFBoolean>() {
        return Some(cf_bool.as_bool());
    }

    if let Some(cf_number) = value.downcast_ref::<CFNumber>() {
        if let Some(number) = cf_number.as_i64() {
            return Some(number != 0);
        }
        if let Some(number) = cf_number.as_i32() {
            return Some(number != 0);
        }
    }

    None
}

fn cf_type_to_point(value: &CFType) -> Option<CGPoint> {
    let ax_value = value.downcast_ref::<AXValue>()?;
    if unsafe { ax_value.r#type() } != AXValueType::CGPoint {
        return None;
    }

    let mut point = CGPoint::default();
    let ok = unsafe {
        ax_value.value(
            AXValueType::CGPoint,
            NonNull::from(&mut point).cast::<c_void>(),
        )
    };

    if ok {
        Some(point)
    } else {
        None
    }
}

fn cf_type_to_size(value: &CFType) -> Option<CGSize> {
    let ax_value = value.downcast_ref::<AXValue>()?;
    if unsafe { ax_value.r#type() } != AXValueType::CGSize {
        return None;
    }

    let mut size = CGSize::default();
    let ok = unsafe {
        ax_value.value(
            AXValueType::CGSize,
            NonNull::from(&mut size).cast::<c_void>(),
        )
    };

    if ok {
        Some(size)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        classify_candidate, AxClassificationCandidate, AxProbeDecision, AxProbeSkipReason,
    };

    fn candidate(
        role: &str,
        editable: bool,
        geometry: Option<(i32, i32, i32, i32)>,
    ) -> AxClassificationCandidate {
        AxClassificationCandidate {
            bundle_id: Some("com.tinyspeck.slackmacgap".to_string()),
            role: Some(role.to_string()),
            subrole: None,
            editable,
            dom_input_type: None,
            metadata_text: String::new(),
            geometry,
        }
    }

    #[test]
    fn classify_candidate_shows_when_editable_even_with_non_text_role() {
        let output = classify_candidate(&candidate("AXGroup", true, Some((10, 20, 180, 38))));
        assert!(matches!(output, AxProbeDecision::Show(_)));
    }

    #[test]
    fn classify_candidate_blocks_non_text_non_editable_roles() {
        let output = classify_candidate(&candidate("AXRow", false, Some((10, 20, 180, 38))));
        assert_eq!(
            output,
            AxProbeDecision::Hide(AxProbeSkipReason::RoleNotTextOrEditable(
                "AXRow".to_string()
            ))
        );
    }

    #[test]
    fn classify_candidate_hides_when_geometry_is_missing() {
        let output = classify_candidate(&candidate("AXTextArea", true, None));
        assert_eq!(
            output,
            AxProbeDecision::Hide(AxProbeSkipReason::MissingGeometry)
        );
    }

    #[test]
    fn classify_candidate_blocks_browser_metadata_for_sensitive_terms() {
        let mut candidate = candidate("AXTextField", true, Some((1, 2, 120, 24)));
        candidate.bundle_id = Some("com.google.Chrome".to_string());
        candidate.metadata_text = "navigation address search".to_string();
        let output = classify_candidate(&candidate);
        assert_eq!(
            output,
            AxProbeDecision::Hide(AxProbeSkipReason::BlockedBrowserMetadata(
                "address".to_string()
            ))
        );
    }
}
