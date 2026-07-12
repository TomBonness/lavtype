//! Download, verify, and configure the pinned English Parakeet model.

use bzip2::read::BzDecoder;
use directories::ProjectDirs;
use fs2::available_space;
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tar::Archive;

pub const MODEL_DIRECTORY_NAME: &str = "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8";
pub const REQUIRED_FREE_SPACE: u64 = 1_288_490_188;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParakeetManifest {
    pub model: &'static str,
    pub asset: &'static str,
    pub url: &'static str,
    pub length: u64,
    pub sha256: &'static str,
    pub installed_files: &'static [&'static str],
}

pub const PARAKEET_MANIFEST: ParakeetManifest = ParakeetManifest {
    model: "nvidia/parakeet-tdt-0.6b-v2",
    asset: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2",
    url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2",
    length: 482_468_385,
    sha256: "157c157bc51155e03e37d2466522a3a737dd9c72bb25f36eb18912964161e1ad",
    installed_files: &[
        "encoder.int8.onnx",
        "decoder.int8.onnx",
        "joiner.int8.onnx",
        "tokens.txt",
    ],
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParakeetInstallState {
    Missing,
    Downloading,
    Ready,
    Error(String),
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("Lavtype data directory is unavailable")]
    DataDirectoryUnavailable,
    #[error("insufficient free space: need {required} bytes, have {available}")]
    InsufficientSpace { required: u64, available: u64 },
    #[error("HTTP response has status {0}")]
    HttpStatus(u16),
    #[error("download content length mismatch: expected {expected}, got {actual}")]
    ContentLength { expected: u64, actual: u64 },
    #[error("download digest mismatch: expected {expected}, got {actual}")]
    DigestMismatch { expected: String, actual: String },
    #[error("archive is invalid: {0}")]
    Archive(String),
    #[error("installed model is invalid: {0}")]
    InvalidModel(String),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub struct ParakeetInstaller {
    data_dir: PathBuf,
    model_dir: PathBuf,
    state: ParakeetInstallState,
}

impl ParakeetInstaller {
    /// Construct an installer at the platform data directory.
    pub fn new() -> Result<Self, InstallError> {
        let dirs = ProjectDirs::from("io.github", "lavtype", "lavtype")
            .ok_or(InstallError::DataDirectoryUnavailable)?;
        Self::from_data_dir(dirs.data_dir())
    }

    /// Construct an installer under `data_dir`; primarily useful for isolated tests.
    pub fn from_data_dir(data_dir: impl AsRef<Path>) -> Result<Self, InstallError> {
        let data_dir = data_dir.as_ref().join("models");
        fs::create_dir_all(&data_dir)?;
        let model_dir = data_dir.join(MODEL_DIRECTORY_NAME);
        cleanup_stale(&data_dir, &model_dir)?;
        let state = state_for_model(&model_dir);
        Ok(Self {
            data_dir,
            model_dir,
            state,
        })
    }

    pub fn model_dir(&self) -> &Path {
        &self.model_dir
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn state(&self) -> ParakeetInstallState {
        self.state.clone()
    }

    pub fn install_state(&self) -> ParakeetInstallState {
        self.state()
    }

    /// Revalidate an existing installation without downloading anything.
    pub fn refresh(&mut self) -> ParakeetInstallState {
        self.state = state_for_model(&self.model_dir);
        self.state.clone()
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.state, ParakeetInstallState::Ready)
            && validate_model_dir(&self.model_dir, &PARAKEET_MANIFEST).is_ok()
    }

    /// Download and atomically install the pinned archive.
    pub fn download(&mut self) -> Result<(), InstallError> {
        self.state = ParakeetInstallState::Downloading;
        let result = self.download_inner();
        if result.is_err() {
            let partial = self
                .data_dir
                .join(format!("{MODEL_DIRECTORY_NAME}.partial"));
            let _ = fs::remove_file(&partial);
            let _ = fs::remove_dir_all(&partial);
        }
        if let Err(error) = &result {
            self.state = ParakeetInstallState::Error(error.to_string());
        } else {
            self.state = ParakeetInstallState::Ready;
        }
        result
    }

    pub fn install(&mut self) -> Result<(), InstallError> {
        self.download()
    }

    /// Install an already downloaded archive after checking its exact manifest.
    pub fn install_archive(&mut self, archive_path: impl AsRef<Path>) -> Result<(), InstallError> {
        self.state = ParakeetInstallState::Downloading;
        let result = install_archive_file(
            archive_path.as_ref(),
            &self.data_dir,
            &self.model_dir,
            &PARAKEET_MANIFEST,
        );
        if let Err(error) = &result {
            self.state = ParakeetInstallState::Error(error.to_string());
        } else {
            self.state = ParakeetInstallState::Ready;
        }
        result
    }

    fn download_inner(&self) -> Result<(), InstallError> {
        let available = available_space(&self.data_dir)?;
        if available < REQUIRED_FREE_SPACE {
            return Err(InstallError::InsufficientSpace {
                required: REQUIRED_FREE_SPACE,
                available,
            });
        }
        let partial = self
            .data_dir
            .join(format!("{MODEL_DIRECTORY_NAME}.partial"));
        let _ = fs::remove_file(&partial);
        let response = Client::builder()
            .build()?
            .get(PARAKEET_MANIFEST.url)
            .send()?;
        if !response.status().is_success() {
            return Err(InstallError::HttpStatus(response.status().as_u16()));
        }
        let remote_length = response.content_length().ok_or_else(|| {
            InstallError::Archive("download did not provide Content-Length".to_string())
        })?;
        if remote_length != PARAKEET_MANIFEST.length {
            return Err(InstallError::ContentLength {
                expected: PARAKEET_MANIFEST.length,
                actual: remote_length,
            });
        }
        let mut response = response;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)?;
        let mut hasher = Sha256::new();
        let mut copied = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = response.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            output.write_all(&buffer[..read])?;
            hasher.update(&buffer[..read]);
            copied += read as u64;
        }
        output.flush()?;
        output.sync_all()?;
        if copied != PARAKEET_MANIFEST.length {
            let _ = fs::remove_file(&partial);
            return Err(InstallError::ContentLength {
                expected: PARAKEET_MANIFEST.length,
                actual: copied,
            });
        }
        let digest = hex_digest(hasher.finalize());
        if digest != PARAKEET_MANIFEST.sha256 {
            let _ = fs::remove_file(&partial);
            return Err(InstallError::DigestMismatch {
                expected: PARAKEET_MANIFEST.sha256.to_string(),
                actual: digest,
            });
        }
        let result = install_archive_file(
            &partial,
            &self.data_dir,
            &self.model_dir,
            &PARAKEET_MANIFEST,
        );
        let _ = fs::remove_file(&partial);
        result
    }
}

fn hex_digest(digest: impl AsRef<[u8]>) -> String {
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn cleanup_stale(parent: &Path, model_dir: &Path) -> Result<(), InstallError> {
    let partial = parent.join(format!("{MODEL_DIRECTORY_NAME}.partial"));
    let _ = fs::remove_file(&partial);
    let _ = fs::remove_dir_all(&partial);
    let mut backups = Vec::new();
    for entry in fs::read_dir(parent)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name.starts_with(".parakeet-install-") {
            let _ = fs::remove_dir_all(path);
        } else if name.starts_with(&format!(".{MODEL_DIRECTORY_NAME}.old-")) {
            backups.push(path);
        }
    }
    if !model_dir.exists()
        && let Some(backup) = backups.pop()
    {
        fs::rename(backup, model_dir)?;
    }
    for backup in backups {
        let _ = fs::remove_dir_all(backup);
    }
    Ok(())
}

fn state_for_model(model_dir: &Path) -> ParakeetInstallState {
    match validate_model_dir(model_dir, &PARAKEET_MANIFEST) {
        Ok(()) => ParakeetInstallState::Ready,
        Err(error) if fs::symlink_metadata(model_dir).is_ok() => {
            ParakeetInstallState::Error(error.to_string())
        }
        Err(_) => ParakeetInstallState::Missing,
    }
}

fn validate_model_dir(model_dir: &Path, manifest: &ParakeetManifest) -> Result<(), InstallError> {
    let metadata = fs::symlink_metadata(model_dir)
        .map_err(|_| InstallError::InvalidModel("model directory is missing".to_string()))?;
    if !metadata.is_dir() {
        return Err(InstallError::InvalidModel(
            "model path is not a directory".to_string(),
        ));
    }
    let expected: HashSet<String> = manifest
        .installed_files
        .iter()
        .map(|file| (*file).to_owned())
        .collect();
    let mut found = HashSet::new();
    for entry in fs::read_dir(model_dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| InstallError::InvalidModel("non-UTF-8 file name".to_string()))?
            .to_owned();
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.is_file() || !expected.contains(&name) {
            return Err(InstallError::InvalidModel(format!(
                "unexpected model entry {name}"
            )));
        }
        found.insert(name);
    }
    if found != expected {
        return Err(InstallError::InvalidModel(
            "one or more model files are missing".to_string(),
        ));
    }
    Ok(())
}

fn install_archive_file(
    archive_path: &Path,
    parent: &Path,
    model_dir: &Path,
    manifest: &ParakeetManifest,
) -> Result<(), InstallError> {
    let metadata = fs::metadata(archive_path)?;
    if metadata.len() != manifest.length {
        return Err(InstallError::ContentLength {
            expected: manifest.length,
            actual: metadata.len(),
        });
    }
    let mut input = File::open(archive_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hex_digest(hasher.finalize());
    if digest != manifest.sha256 {
        return Err(InstallError::DigestMismatch {
            expected: manifest.sha256.to_string(),
            actual: digest,
        });
    }
    extract_and_install(archive_path, parent, model_dir, manifest)
}

fn extract_and_install(
    archive_path: &Path,
    parent: &Path,
    model_dir: &Path,
    manifest: &ParakeetManifest,
) -> Result<(), InstallError> {
    let temp = tempfile::Builder::new()
        .prefix(".parakeet-install-")
        .tempdir_in(parent)?;
    let input = File::open(archive_path)?;
    let decoder = BzDecoder::new(input);
    let mut archive = Archive::new(decoder);
    let mut seen = HashSet::new();
    for item in archive.entries()? {
        let entry = item.map_err(|error| InstallError::Archive(error.to_string()))?;
        let path = entry
            .path()
            .map_err(|error| InstallError::Archive(error.to_string()))?
            .into_owned();
        let mut components = path.components();
        let first = match components.next() {
            Some(Component::Normal(value)) if value == MODEL_DIRECTORY_NAME => value,
            _ => {
                return Err(InstallError::Archive(
                    "entry is outside expected top-level directory".to_string(),
                ));
            }
        };
        if components.next().is_none() && !entry.header().entry_type().is_dir() {
            return Err(InstallError::Archive(
                "top-level entry is not a directory".to_string(),
            ));
        }
        if path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(InstallError::Archive(
                "absolute or parent archive path".to_string(),
            ));
        }
        if !entry.header().entry_type().is_file() && !entry.header().entry_type().is_dir() {
            return Err(InstallError::Archive(
                "archive contains a link or special entry".to_string(),
            ));
        }
        if !seen.insert(path.clone()) {
            return Err(InstallError::Archive(
                "archive contains duplicate entries".to_string(),
            ));
        }
        let _ = first;
    }
    let input = File::open(archive_path)?;
    let decoder = BzDecoder::new(input);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(temp.path())
        .map_err(|error| InstallError::Archive(error.to_string()))?;
    let extracted = temp.path().join(MODEL_DIRECTORY_NAME);
    validate_model_dir(&extracted, manifest)?;
    atomic_replace(&extracted, model_dir, parent)
}

fn atomic_replace(new_model: &Path, model_dir: &Path, parent: &Path) -> Result<(), InstallError> {
    let backup = parent.join(format!(".{MODEL_DIRECTORY_NAME}.old-{}", unix_seconds()));
    let had_old = model_dir.exists() || fs::symlink_metadata(model_dir).is_ok();
    if had_old {
        fs::rename(model_dir, &backup)?;
    }
    if let Err(error) = fs::rename(new_model, model_dir) {
        if had_old {
            let _ = fs::rename(&backup, model_dir);
        }
        return Err(error.into());
    }
    if had_old {
        let _ = fs::remove_dir_all(backup);
    }
    Ok(())
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Construct a cached sherpa recognizer for the pinned four-file model.
pub fn create_parakeet_recognizer(
    model_dir: impl AsRef<Path>,
) -> Result<OfflineRecognizer, InstallError> {
    let model_dir = model_dir.as_ref();
    validate_model_dir(model_dir, &PARAKEET_MANIFEST)?;
    let mut config = OfflineRecognizerConfig::default();
    config.model_config.transducer = OfflineTransducerModelConfig {
        encoder: Some(
            model_dir
                .join("encoder.int8.onnx")
                .to_string_lossy()
                .into_owned(),
        ),
        decoder: Some(
            model_dir
                .join("decoder.int8.onnx")
                .to_string_lossy()
                .into_owned(),
        ),
        joiner: Some(
            model_dir
                .join("joiner.int8.onnx")
                .to_string_lossy()
                .into_owned(),
        ),
    };
    config.model_config.tokens = Some(model_dir.join("tokens.txt").to_string_lossy().into_owned());
    config.model_config.model_type = Some("nemo_transducer".to_string());
    config.model_config.provider = Some("cpu".to_string());
    config.model_config.num_threads = 2;
    config.decoding_method = Some("greedy_search".to_string());
    OfflineRecognizer::create(&config).ok_or_else(|| {
        InstallError::InvalidModel("sherpa-onnx could not create recognizer".to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::LazyLock;
    use tar::Builder;

    fn archive_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut raw = Vec::new();
        {
            let mut builder = Builder::new(&mut raw);
            builder.append_dir(MODEL_DIRECTORY_NAME, ".").unwrap();
            for (name, bytes) in entries {
                let path = format!("{MODEL_DIRECTORY_NAME}/{name}");
                let mut header = tar::Header::new_gnu();
                header.set_size(bytes.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder
                    .append_data(&mut header, path, Cursor::new(*bytes))
                    .unwrap();
            }
            builder.finish().unwrap();
        }
        let mut compressed = Vec::new();
        {
            let mut encoder =
                bzip2::write::BzEncoder::new(&mut compressed, bzip2::Compression::best());
            encoder.write_all(&raw).unwrap();
            encoder.finish().unwrap();
        }
        compressed
    }

    fn link_archive_bytes() -> Vec<u8> {
        let mut raw = Vec::new();
        {
            let mut builder = Builder::new(&mut raw);
            builder.append_dir(MODEL_DIRECTORY_NAME, ".").unwrap();
            let mut header = tar::Header::new_gnu();
            header
                .set_path(format!("{MODEL_DIRECTORY_NAME}/tokens.txt"))
                .unwrap();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_link_name("outside").unwrap();
            header.set_cksum();
            builder
                .append(&header, Cursor::new(Vec::<u8>::new()))
                .unwrap();
            builder.finish().unwrap();
        }
        let mut compressed = Vec::new();
        let mut encoder = bzip2::write::BzEncoder::new(&mut compressed, bzip2::Compression::best());
        encoder.write_all(&raw).unwrap();
        encoder.finish().unwrap();
        compressed
    }

    fn traversal_archive_bytes() -> Vec<u8> {
        let mut raw = Vec::new();
        {
            let mut builder = Builder::new(&mut raw);
            builder.append_dir(MODEL_DIRECTORY_NAME, ".").unwrap();
            let mut header = tar::Header::new_gnu();
            header
                .set_path(format!("{MODEL_DIRECTORY_NAME}/safe"))
                .unwrap();
            header.set_size(3);
            header.set_mode(0o644);
            let path = format!("{MODEL_DIRECTORY_NAME}/../outside");
            header.as_mut_bytes()[..100].fill(0);
            header.as_mut_bytes()[..path.len()].copy_from_slice(path.as_bytes());
            header.set_cksum();
            builder
                .append(&header, Cursor::new(b"bad".to_vec()))
                .unwrap();
            builder.finish().unwrap();
        }
        let mut compressed = Vec::new();
        let mut encoder = bzip2::write::BzEncoder::new(&mut compressed, bzip2::Compression::best());
        encoder.write_all(&raw).unwrap();
        encoder.finish().unwrap();
        compressed
    }

    #[test]
    fn manifest_is_pinned() {
        assert_eq!(PARAKEET_MANIFEST.length, 482_468_385);
        assert_eq!(PARAKEET_MANIFEST.installed_files.len(), 4);
        assert_eq!(
            MODEL_DIRECTORY_NAME,
            "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8"
        );
    }

    #[test]
    fn installed_dir_requires_exact_files() {
        let root = tempfile::tempdir().unwrap();
        let model = root.path().join(MODEL_DIRECTORY_NAME);
        fs::create_dir(&model).unwrap();
        for name in PARAKEET_MANIFEST.installed_files {
            fs::write(model.join(name), b"x").unwrap();
        }
        assert!(validate_model_dir(&model, &PARAKEET_MANIFEST).is_ok());
        fs::write(model.join("extra"), b"x").unwrap();
        assert!(validate_model_dir(&model, &PARAKEET_MANIFEST).is_err());
    }
    fn test_manifest(length: u64, sha256: &'static str) -> ParakeetManifest {
        static FILES: &[&str] = &[
            "encoder.int8.onnx",
            "decoder.int8.onnx",
            "joiner.int8.onnx",
            "tokens.txt",
        ];
        ParakeetManifest {
            model: "test",
            asset: "test.tar.bz2",
            url: "https://invalid.test/test",
            length,
            sha256,
            installed_files: FILES,
        }
    }

    fn digest(bytes: &[u8]) -> String {
        hex_digest(Sha256::digest(bytes))
    }

    #[test]
    fn archive_install_checks_digest_length_and_atomic_readiness() {
        let entries = [
            ("encoder.int8.onnx", b"encoder".as_slice()),
            ("decoder.int8.onnx", b"decoder".as_slice()),
            ("joiner.int8.onnx", b"joiner".as_slice()),
            ("tokens.txt", b"tokens".as_slice()),
        ];
        let bytes = archive_bytes(&entries);
        let root = tempfile::tempdir().unwrap();
        let archive = root.path().join("archive.tar.bz2");
        fs::write(&archive, &bytes).unwrap();
        static TEST_DIGEST: LazyLock<String> = LazyLock::new(|| {
            let entries = [
                ("encoder.int8.onnx", b"encoder".as_slice()),
                ("decoder.int8.onnx", b"decoder".as_slice()),
                ("joiner.int8.onnx", b"joiner".as_slice()),
                ("tokens.txt", b"tokens".as_slice()),
            ];
            digest(&archive_bytes(&entries))
        });
        let digest = TEST_DIGEST.as_str();
        let manifest = test_manifest(bytes.len() as u64, digest);
        let model = root.path().join(MODEL_DIRECTORY_NAME);
        install_archive_file(&archive, root.path(), &model, &manifest).unwrap();
        assert!(validate_model_dir(&model, &manifest).is_ok());

        let short_manifest = test_manifest(bytes.len() as u64 + 1, digest);
        assert!(matches!(
            install_archive_file(&archive, root.path(), &model, &short_manifest),
            Err(InstallError::ContentLength { .. })
        ));
        let bad_digest = test_manifest(
            bytes.len() as u64,
            "0000000000000000000000000000000000000000000000000000000000000000",
        );
        assert!(matches!(
            install_archive_file(&archive, root.path(), &model, &bad_digest),
            Err(InstallError::DigestMismatch { .. })
        ));
    }

    #[test]
    fn archive_rejects_traversal_and_missing_files() {
        let root = tempfile::tempdir().unwrap();
        let model = root.path().join(MODEL_DIRECTORY_NAME);
        let traversal = traversal_archive_bytes();
        let traversal_path = root.path().join("traversal.tar.bz2");
        fs::write(&traversal_path, traversal).unwrap();
        assert!(matches!(
            extract_and_install(&traversal_path, root.path(), &model, &PARAKEET_MANIFEST),
            Err(InstallError::Archive(_))
        ));

        let link = link_archive_bytes();
        let link_path = root.path().join("link.tar.bz2");
        fs::write(&link_path, link).unwrap();
        assert!(matches!(
            extract_and_install(&link_path, root.path(), &model, &PARAKEET_MANIFEST),
            Err(InstallError::Archive(_))
        ));

        let missing = archive_bytes(&[
            ("encoder.int8.onnx", b"encoder".as_slice()),
            ("decoder.int8.onnx", b"decoder".as_slice()),
            ("joiner.int8.onnx", b"joiner".as_slice()),
        ]);
        let missing_path = root.path().join("missing.tar.bz2");
        fs::write(&missing_path, missing).unwrap();
        assert!(matches!(
            extract_and_install(&missing_path, root.path(), &model, &PARAKEET_MANIFEST),
            Err(InstallError::InvalidModel(_))
        ));
    }
}
