use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PermissionTarget {
    Microphone,
    Accessibility,
    Automation,
}

impl PermissionTarget {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_lowercase().as_str() {
            "microphone" => Some(Self::Microphone),
            "accessibility" => Some(Self::Accessibility),
            "automation" => Some(Self::Automation),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlatformCapabilities {
    pub(crate) platform: &'static str,
    pub(crate) needs_accessibility: bool,
    pub(crate) needs_automation: bool,
    pub(crate) supports_permission_settings: bool,
    pub(crate) supports_contextual_anchor: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) struct PlatformFocusedAnchorSnapshot {
    pub(crate) anchor_x: i32,
    pub(crate) anchor_y: i32,
    pub(crate) focus_x: i32,
    pub(crate) focus_y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) struct PlatformFocusedAnchorProbe {
    pub(crate) snapshot: Option<PlatformFocusedAnchorSnapshot>,
    pub(crate) reason: String,
    pub(crate) bundle_id: Option<String>,
    pub(crate) source: &'static str,
    pub(crate) pid: Option<i32>,
    pub(crate) role: Option<String>,
}

pub(crate) trait PlatformBackend: Send + Sync {
    fn capabilities(&self) -> PlatformCapabilities;
    fn open_permission_settings(&self, target: PermissionTarget) -> Result<(), String>;
    fn open_external_url(&self, url: &str) -> Result<(), String>;
    fn ensure_accessibility_permission(&self) -> Result<(), String>;
    fn ensure_system_events_permission(&self) -> Result<(), String>;
    fn simulate_modifier_shortcut(&self, character: char) -> Result<(), String>;
    fn refocus_point(
        &self,
        x: i32,
        y: i32,
        click_stabilize_ms: u64,
        post_restore_ms: u64,
    ) -> Result<(), String>;
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod unsupported;
#[cfg(target_os = "windows")]
mod windows;

static PLATFORM_BACKEND: OnceLock<Box<dyn PlatformBackend>> = OnceLock::new();

pub(crate) fn backend() -> &'static dyn PlatformBackend {
    PLATFORM_BACKEND
        .get_or_init(|| {
            #[cfg(target_os = "macos")]
            {
                Box::new(macos::MacosBackend)
            }
            #[cfg(target_os = "windows")]
            {
                Box::new(windows::WindowsBackend)
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                Box::new(unsupported::UnsupportedBackend)
            }
        })
        .as_ref()
}

pub(crate) fn capabilities() -> PlatformCapabilities {
    backend().capabilities()
}

#[cfg(target_os = "windows")]
pub(crate) fn focused_anchor_probe() -> PlatformFocusedAnchorProbe {
    windows::focused_anchor_probe()
}

#[cfg(not(target_os = "windows"))]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn focused_anchor_probe() -> PlatformFocusedAnchorProbe {
    PlatformFocusedAnchorProbe {
        snapshot: None,
        reason: "contextual_not_supported".to_string(),
        bundle_id: None,
        source: "unsupported",
        pid: None,
        role: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{capabilities, focused_anchor_probe, PermissionTarget};

    #[test]
    fn permission_target_parse_supports_expected_values() {
        assert_eq!(
            PermissionTarget::parse("microphone"),
            Some(PermissionTarget::Microphone)
        );
        assert_eq!(
            PermissionTarget::parse("accessibility"),
            Some(PermissionTarget::Accessibility)
        );
        assert_eq!(
            PermissionTarget::parse("automation"),
            Some(PermissionTarget::Automation)
        );
        assert_eq!(PermissionTarget::parse("unknown"), None);
    }

    #[test]
    fn capabilities_match_compiled_platform() {
        let caps = capabilities();

        if cfg!(target_os = "macos") {
            assert_eq!(caps.platform, "macos");
            assert!(caps.needs_accessibility);
            assert!(caps.needs_automation);
            assert!(caps.supports_permission_settings);
            assert!(caps.supports_contextual_anchor);
        } else if cfg!(target_os = "windows") {
            assert_eq!(caps.platform, "windows");
            assert!(!caps.needs_accessibility);
            assert!(!caps.needs_automation);
            assert!(caps.supports_permission_settings);
            assert!(caps.supports_contextual_anchor);
        } else if cfg!(target_os = "linux") {
            assert_eq!(caps.platform, "linux");
            assert!(!caps.needs_accessibility);
            assert!(!caps.needs_automation);
            assert!(!caps.supports_permission_settings);
            assert!(!caps.supports_contextual_anchor);
        } else {
            assert_eq!(caps.platform, "unknown");
        }
    }

    #[test]
    fn focused_anchor_probe_matches_platform_support() {
        let probe = focused_anchor_probe();

        if cfg!(target_os = "windows") {
            assert_ne!(probe.reason, "contextual_not_supported");
        } else {
            assert_eq!(probe.reason, "contextual_not_supported");
            assert!(probe.snapshot.is_none());
        }
    }
}
