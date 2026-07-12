//! Speech transcription engines and their shared error contract.

use std::path::Path;

use thiserror::Error;

#[cfg(target_os = "macos")]
pub mod apple;
pub mod parakeet;

#[cfg(target_os = "macos")]
pub use apple::AppleSpeechTranscriber;
pub use parakeet::ParakeetTranscriber;

/// Failures reported by a speech engine. Messages are intentionally actionable
/// because they are shown verbatim in the tray status row.
#[derive(Debug, Error)]
pub enum TranscriptionError {
    #[error("Parakeet is not installed; choose Download model")]
    ModelNotInstalled,
    #[error("Parakeet recognizer could not be created: {0}")]
    ModelLoad(String),
    #[error("Parakeet returned no recognition result")]
    MissingResult,
    #[error("Parakeet recognition failed: {0}")]
    Recognition(String),
    #[error("Apple Speech permission is denied")]
    ApplePermissionDenied,
    #[error(
        "On-device Apple Speech is unavailable for the current system language; choose Parakeet or install macOS dictation support."
    )]
    AppleUnavailable,
    #[error("Apple Speech recognition failed: {0}")]
    AppleRecognition(String),
    #[error("Apple Speech recognition timed out")]
    AppleTimeout,
    #[error("invalid sample rate {0}; transcription requires 16000 Hz")]
    InvalidSampleRate(u32),
    #[error("could not write temporary recognition audio: {0}")]
    AudioFile(String),
}

/// Dispatch-independent contract useful to workers and tests. The app's
/// `SpeechTranscriber` seam has the same signature and is implemented by both
/// production engines.
pub trait Transcriber {
    fn transcribe(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<String, TranscriptionError>;
}

/// Apply the one output policy shared by all engines.
pub fn normalize_output(text: &str, lowercase: bool) -> String {
    let trimmed = text.trim();
    if lowercase {
        trimmed.to_lowercase()
    } else {
        trimmed.to_owned()
    }
}

/// Returns true when an installed model directory contains the exact files
/// needed by the Parakeet runtime.
pub(crate) fn model_files_present(path: &Path) -> bool {
    [
        "encoder.int8.onnx",
        "decoder.int8.onnx",
        "joiner.int8.onnx",
        "tokens.txt",
    ]
    .iter()
    .all(|name| path.join(name).is_file())
}
