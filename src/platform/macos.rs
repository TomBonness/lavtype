//! macOS privacy and on-device speech permission helpers.
//!
//! Objective-C calls live exclusively in this file.  The coordinator only
//! deals in the small [`Permission`] value and `Result<bool, &str>` request
//! APIs, so a denied permission never tears down the tray/event loop.

use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_application_services::AXIsProcessTrustedWithOptions;
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
use objc2_speech::{SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus};

/// Exact `NSMicrophoneUsageDescription` value required by the application.
pub const MICROPHONE_USAGE_DESCRIPTION: &str =
    "Lavtype records while you hold the dictation shortcut.";

/// Exact `NSSpeechRecognitionUsageDescription` value required by the
/// application.
pub const SPEECH_USAGE_DESCRIPTION: &str =
    "Lavtype uses Apple Speech to turn your recording into text.";

/// Actionable error used when Apple cannot provide a local recognizer for the
/// current system language.  Network recognition is never used as a fallback.
pub const APPLE_SPEECH_UNAVAILABLE_ERROR: &str = "On-device Apple Speech is unavailable for the current system language; choose Parakeet or install macOS dictation support.";

/// Actionable error returned when text injection is attempted without the
/// Accessibility grant.
pub const ACCESSIBILITY_PERMISSION_ERROR: &str = "Accessibility permission is required to type into the focused application; enable Lavtype in System Settings > Privacy & Security > Accessibility.";

/// A normalized TCC permission state shared by microphone and Speech.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    NotDetermined,
    Denied,
    Restricted,
    Authorized,
}

impl Permission {
    pub const fn is_authorized(self) -> bool {
        matches!(self, Self::Authorized)
    }
}

/// Read the current microphone TCC state through AVFoundation.
pub fn microphone_permission() -> Permission {
    let Some(media_type) = (unsafe { AVMediaTypeAudio.as_ref() }) else {
        // AVMediaTypeAudio is a framework constant and should always exist;
        // treating an unavailable constant as restricted is safer than asking
        // AVFoundation with an invalid media type.
        return Permission::Restricted;
    };
    let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(media_type) };
    map_av_status(status)
}

/// Request microphone access.  The completion callback is bridged immediately
/// into Rust-owned state; no UI work is performed from Apple's callback queue.
pub fn request_microphone() -> Result<bool, &'static str> {
    let Some(media_type) = (unsafe { AVMediaTypeAudio.as_ref() }) else {
        return Err("AVFoundation does not expose an audio media type");
    };
    let (sender, receiver) = mpsc::sync_channel(1);
    let callback: RcBlock<dyn Fn(Bool)> = RcBlock::new(move |granted: Bool| {
        let _ = sender.send(granted.as_bool());
    });
    unsafe {
        AVCaptureDevice::requestAccessForMediaType_completionHandler(media_type, &callback);
    }
    receiver
        .recv_timeout(Duration::from_secs(60))
        .map_err(|_| "Timed out while waiting for microphone permission")
}

/// Read the current Speech TCC state.
pub fn speech_permission() -> Permission {
    let status = unsafe { SFSpeechRecognizer::authorizationStatus() };
    map_speech_status(status)
}

/// Request Speech access.  Apple invokes the completion block on an arbitrary
/// queue, so this function only returns a copied Rust bool to its caller.
pub fn request_speech() -> Result<bool, &'static str> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let callback: RcBlock<dyn Fn(SFSpeechRecognizerAuthorizationStatus)> =
        RcBlock::new(move |status: SFSpeechRecognizerAuthorizationStatus| {
            let _ = sender.send(map_speech_status(status).is_authorized());
        });
    unsafe {
        SFSpeechRecognizer::requestAuthorization(&callback);
    }
    receiver
        .recv_timeout(Duration::from_secs(60))
        .map_err(|_| "Timed out while waiting for Speech permission")
}

/// Check whether this process can send keyboard events to the focused app.
///
/// Passing `None` asks the system for the current trust state without
/// presenting a prompt.  Calling this API rather than Input Monitoring APIs is
/// intentional: registered global shortcuts do not require Input Monitoring.
pub fn accessibility_permission() -> bool {
    unsafe { AXIsProcessTrustedWithOptions(None) }
}

/// Read Accessibility and, where possible, trigger Apple's standard trust
/// check.  The system settings prompt is asynchronous; callers should keep
/// the menu alive and call [`accessibility_permission`] again when returning
/// to the app.
pub fn request_accessibility() -> bool {
    unsafe { AXIsProcessTrustedWithOptions(None) }
}

/// Return the three values needed by a Permissions menu in one call.
pub fn permission_summary() -> (Permission, Permission, bool) {
    (
        microphone_permission(),
        speech_permission(),
        accessibility_permission(),
    )
}

fn map_av_status(status: AVAuthorizationStatus) -> Permission {
    if status == AVAuthorizationStatus::Authorized {
        Permission::Authorized
    } else if status == AVAuthorizationStatus::Denied {
        Permission::Denied
    } else if status == AVAuthorizationStatus::Restricted {
        Permission::Restricted
    } else {
        Permission::NotDetermined
    }
}

fn map_speech_status(status: SFSpeechRecognizerAuthorizationStatus) -> Permission {
    if status == SFSpeechRecognizerAuthorizationStatus::Authorized {
        Permission::Authorized
    } else if status == SFSpeechRecognizerAuthorizationStatus::Denied {
        Permission::Denied
    } else if status == SFSpeechRecognizerAuthorizationStatus::Restricted {
        Permission::Restricted
    } else {
        Permission::NotDetermined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_strings_are_stable() {
        assert_eq!(
            MICROPHONE_USAGE_DESCRIPTION,
            "Lavtype records while you hold the dictation shortcut."
        );
        assert_eq!(
            SPEECH_USAGE_DESCRIPTION,
            "Lavtype uses Apple Speech to turn your recording into text."
        );
    }

    #[test]
    fn permission_is_authorized_only_for_authorized_state() {
        assert!(Permission::Authorized.is_authorized());
        assert!(!Permission::Denied.is_authorized());
        assert!(!Permission::Restricted.is_authorized());
        assert!(!Permission::NotDetermined.is_authorized());
    }
}
