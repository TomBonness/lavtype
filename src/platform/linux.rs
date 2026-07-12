//! Linux platform checks.
//!
//! Lavtype intentionally supports X11 only.  Global hotkeys and the GTK tray
//! must not be initialized until this check succeeds; this also rejects an
//! XWayland display when the desktop session itself is Wayland.

use std::env;

/// Status shown in the tray when this process is not running in a supported
/// Linux session.  Keep this wording stable: it is also used by diagnostics.
pub const LINUX_X11_ERROR: &str = "lavtype currently requires an X11 session on Linux";

/// Guidance shown by the Linux tray/about UI when no AppIndicator host is
/// present.  The application remains tray-only; it does not create a second
/// window as a fallback.
pub const APPINDICATOR_HOST_GUIDANCE: &str = "Lavtype requires an X11 desktop with a GTK AppIndicator host (for example, an AppIndicator/status-notifier extension).";

/// Short name for callers constructing tray help text.
pub const APPINDICATOR_GUIDANCE: &str = APPINDICATOR_HOST_GUIDANCE;

/// Return the host requirement for a tray/about menu item.
pub const fn appindicator_host_guidance() -> &'static str {
    APPINDICATOR_HOST_GUIDANCE
}

/// Validate a session using explicit environment values.
///
/// Keeping this pure function separate makes the startup policy deterministic
/// and lets callers test the Wayland/XWayland edge case without mutating the
/// process environment. `WAYLAND_DISPLAY` being present is rejected even
/// when `DISPLAY` is also present (the latter is how XWayland sessions often
/// look).
pub fn validate_linux_session_values(
    session_type: Option<&str>,
    wayland_display: Option<&str>,
    display: Option<&str>,
) -> Result<(), &'static str> {
    let is_wayland_session =
        session_type.is_some_and(|value| value.eq_ignore_ascii_case("wayland"));
    let has_wayland_display = wayland_display.is_some();
    let has_display = display.is_some_and(|value| !value.trim().is_empty());

    if is_wayland_session || has_wayland_display || !has_display {
        Err(LINUX_X11_ERROR)
    } else {
        Ok(())
    }
}

/// Validate the current process environment before initializing input/audio.
pub fn validate_linux_session() -> Result<(), &'static str> {
    validate_linux_session_values(
        env::var("XDG_SESSION_TYPE").ok().as_deref(),
        env::var("WAYLAND_DISPLAY").ok().as_deref(),
        env::var("DISPLAY").ok().as_deref(),
    )
}

/// Startup spelling used by the coordinator.  This intentionally performs no
/// initialization and is an alias of [`validate_linux_session`].
pub fn require_x11_session() -> Result<(), &'static str> {
    validate_linux_session()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wayland_even_with_xwayland() {
        let result = validate_linux_session_values(Some("wayland"), Some("wayland-0"), Some(":0"));
        assert_eq!(result, Err(LINUX_X11_ERROR));
    }

    #[test]
    fn rejects_wayland_display_with_x11_session() {
        let result = validate_linux_session_values(Some("x11"), Some("wayland-0"), Some(":0"));
        assert_eq!(result, Err(LINUX_X11_ERROR));
    }

    #[test]
    fn rejects_missing_or_empty_display() {
        assert_eq!(
            validate_linux_session_values(Some("x11"), None, None),
            Err(LINUX_X11_ERROR)
        );
        assert_eq!(
            validate_linux_session_values(Some("x11"), None, Some("  ")),
            Err(LINUX_X11_ERROR)
        );
    }

    #[test]
    fn accepts_x11_display() {
        assert_eq!(
            validate_linux_session_values(Some("x11"), None, Some(":0")),
            Ok(())
        );
    }
}
