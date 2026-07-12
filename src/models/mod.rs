//! Local speech-model installation and readiness.

mod parakeet;

pub use parakeet::{
    DownloadProgress, InstallError, PARAKEET_MANIFEST, ParakeetInstallState, ParakeetInstaller,
    ParakeetManifest, create_parakeet_recognizer,
};
