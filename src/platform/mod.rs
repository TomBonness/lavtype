//! Small platform boundary used by the coordinator.
//!
//! Platform-specific code is deliberately kept behind this module.  In
//! particular, Linux session validation happens before creating either a
//! global-hotkey manager or an audio stream.

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub use linux::{
    APPINDICATOR_GUIDANCE, APPINDICATOR_HOST_GUIDANCE, LINUX_X11_ERROR, appindicator_host_guidance,
    require_x11_session, validate_linux_session,
};

#[cfg(target_os = "macos")]
pub use macos::{
    ACCESSIBILITY_PERMISSION_ERROR, APPLE_SPEECH_UNAVAILABLE_ERROR, MICROPHONE_USAGE_DESCRIPTION,
    Permission, SPEECH_USAGE_DESCRIPTION, accessibility_permission, configure_menu_bar_only,
    microphone_permission, open_privacy_settings, permission_summary, request_accessibility,
    request_microphone, request_speech, speech_permission,
};
