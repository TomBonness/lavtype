use crate::hotkey::Shortcut;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const SCHEMA_VERSION: u32 = 1;
const FILE_NAME: &str = "lavtype.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Engine {
    #[serde(rename = "apple-speech")]
    AppleSpeech,
    #[serde(rename = "parakeet")]
    Parakeet,
}

impl Default for Engine {
    #[cfg(target_os = "macos")]
    fn default() -> Self {
        Self::AppleSpeech
    }
    #[cfg(not(target_os = "macos"))]
    fn default() -> Self {
        Self::Parakeet
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub schema_version: u32,
    pub engine: Engine,
    pub lowercase: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<Shortcut>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            engine: Engine::default(),
            lowercase: false,
            shortcut: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("could not determine the Lavtype configuration directory")]
    ConfigDirectoryUnavailable,
    #[error("could not read settings: {0}")]
    Read(#[source] io::Error),
    #[error("invalid settings TOML: {0}")]
    Parse(String),
    #[error("unsupported settings schema version {0}; expected {SCHEMA_VERSION}")]
    SchemaVersion(u32),
    #[error("Apple Speech is not supported on Linux")]
    UnsupportedEngine,
    #[error("could not write settings: {0}")]
    Write(#[source] io::Error),
    #[error("could not encode settings: {0}")]
    Encode(String),
}

#[derive(Debug)]
pub struct LoadOutcome {
    pub settings: Settings,
    /// A malformed/unsupported file is backed up and defaults are returned. This field keeps
    /// the actionable parse/version error visible to the tray rather than silently discarding it.
    pub error: Option<SettingsError>,
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn for_current_user() -> Result<Self, SettingsError> {
        let dirs = ProjectDirs::from("io.github", "lavtype", "lavtype")
            .ok_or(SettingsError::ConfigDirectoryUnavailable)?;
        Ok(Self::new(dirs.config_dir().join(FILE_NAME)))
    }

    pub fn load(&self) -> Result<LoadOutcome, SettingsError> {
        if !self.path.exists() {
            return Ok(LoadOutcome {
                settings: Settings::default(),
                error: None,
            });
        }
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(LoadOutcome {
                    settings: Settings::default(),
                    error: None,
                });
            }
            Err(error) => return Err(SettingsError::Read(error)),
        };
        match parse_settings(&bytes) {
            Ok(settings) => Ok(LoadOutcome {
                settings,
                error: None,
            }),
            Err(
                error @ (SettingsError::Parse(_)
                | SettingsError::SchemaVersion(_)
                | SettingsError::UnsupportedEngine),
            ) => {
                let backup = backup_path(&self.path);
                fs::rename(&self.path, backup).map_err(SettingsError::Read)?;
                Ok(LoadOutcome {
                    settings: Settings::default(),
                    error: Some(error),
                })
            }
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, settings: &Settings) -> Result<(), SettingsError> {
        save_to_path(&self.path, settings)
    }
}

pub fn config_path() -> Result<PathBuf, SettingsError> {
    SettingsStore::for_current_user().map(|store| store.path)
}

pub fn load() -> Result<LoadOutcome, SettingsError> {
    SettingsStore::for_current_user()?.load()
}

pub fn save(settings: &Settings) -> Result<(), SettingsError> {
    SettingsStore::for_current_user()?.save(settings)
}

fn parse_settings(bytes: &[u8]) -> Result<Settings, SettingsError> {
    let mut settings: Settings =
        toml::from_slice(bytes).map_err(|error| SettingsError::Parse(error.to_string()))?;
    if settings.schema_version != SCHEMA_VERSION {
        return Err(SettingsError::SchemaVersion(settings.schema_version));
    }
    #[cfg(not(target_os = "macos"))]
    if settings.engine == Engine::AppleSpeech {
        return Err(SettingsError::UnsupportedEngine);
    }
    settings.schema_version = SCHEMA_VERSION;
    Ok(settings)
}

fn save_to_path(path: &Path, settings: &Settings) -> Result<(), SettingsError> {
    let mut settings = settings.clone();
    settings.schema_version = SCHEMA_VERSION;
    #[cfg(not(target_os = "macos"))]
    if settings.engine == Engine::AppleSpeech {
        return Err(SettingsError::UnsupportedEngine);
    }
    let encoded = toml::to_string_pretty(&settings)
        .map_err(|error| SettingsError::Encode(error.to_string()))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(SettingsError::Write)?;
    }
    let tmp = temporary_path(path);
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp)
            .map_err(SettingsError::Write)?;
        file.write_all(encoded.as_bytes())
            .map_err(SettingsError::Write)?;
        file.flush().map_err(SettingsError::Write)?;
        file.sync_all().map_err(SettingsError::Write)?;
        fs::rename(&tmp, path).map_err(SettingsError::Write)?;
        sync_parent(path).map_err(SettingsError::Write)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut tmp = path.to_path_buf();
    let suffix = format!(".tmp-{}-{}", std::process::id(), timestamp());
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(FILE_NAME);
    tmp.set_file_name(format!("{name}{suffix}"));
    tmp
}

fn backup_path(path: &Path) -> PathBuf {
    let stem = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(FILE_NAME);
    let mut candidate = path.with_file_name(format!("{stem}.broken-{}", timestamp()));
    let mut n = 1u32;
    while candidate.exists() {
        candidate = path.with_file_name(format!("{stem}.broken-{}-{n}", timestamp()));
        n += 1;
    }
    candidate
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn sync_parent(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        #[cfg(unix)]
        {
            File::open(parent)?.sync_all()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn round_trip_preserves_exact_shape() {
        let dir = tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join(FILE_NAME));
        let settings = Settings {
            schema_version: 1,
            engine: Engine::Parakeet,
            lowercase: true,
            shortcut: Some(Shortcut::new(
                crate::hotkey::KeyName::Space,
                crate::hotkey::Modifiers::CONTROL | crate::hotkey::Modifiers::SHIFT,
            )),
        };
        store.save(&settings).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.settings, settings);
        let text = fs::read_to_string(store.path()).unwrap();
        assert!(text.contains("schema_version = 1"));
        assert!(text.contains("engine = \"parakeet\""));
        assert!(text.contains("[shortcut]"));
    }

    #[test]
    fn corrupt_file_is_backed_up_and_defaults_are_loaded() {
        let dir = tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        fs::write(&path, "schema_version = nope\n").unwrap();
        let store = SettingsStore::new(path.clone());
        let result = store.load().unwrap();
        assert_eq!(result.settings, Settings::default());
        assert!(matches!(result.error, Some(SettingsError::Parse(_))));
        assert!(!path.exists());
        let backups: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            backups
                .iter()
                .any(|name| name.starts_with("lavtype.toml.broken-"))
        );
    }
}
