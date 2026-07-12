use std::cell::RefCell;

use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{AboutMetadata, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
};

use crate::app::OperationState;
use crate::models::DownloadProgress;
pub const ID_SHORTCUT: &str = "shortcut";
pub const ID_ENGINE_APPLE: &str = "engine-apple";
pub const ID_ENGINE_PARAKEET: &str = "engine-parakeet";
pub const ID_LOWERCASE: &str = "lowercase";
pub const ID_PERMISSIONS: &str = "permissions";
pub const ID_DOWNLOAD: &str = "download";
pub const ID_QUIT: &str = "quit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayVisualState {
    Idle,
    Recording,
    Busy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TraySnapshot {
    state: OperationState,
    error: Option<String>,
    parakeet_ready: bool,
    has_shortcut: bool,
    download_progress: Option<DownloadProgress>,
}

fn should_publish(last: Option<&TraySnapshot>, next: &TraySnapshot) -> bool {
    last != Some(next)
}

pub struct Tray {
    pub icon: TrayIcon,
    pub menu: Menu,
    pub status: MenuItem,
    pub shortcut: MenuItem,
    #[cfg(target_os = "macos")]
    pub apple: CheckMenuItem,
    pub parakeet: CheckMenuItem,
    pub lowercase: CheckMenuItem,
    #[cfg(target_os = "macos")]
    pub permissions: MenuItem,
    pub download: MenuItem,
    pub quit: MenuItem,
    last_published: RefCell<Option<TraySnapshot>>,
}

fn model_menu_label(
    state: OperationState,
    progress: Option<DownloadProgress>,
    ready: bool,
) -> String {
    match (state, progress, ready) {
        (OperationState::Downloading, Some(progress), _) => {
            format!("Downloading model… {}%", progress.percent())
        }
        (OperationState::Downloading, None, _) => "Downloading model…".to_string(),
        (_, _, true) => "Parakeet model installed".to_string(),
        _ => "Download model (460 MiB)".to_string(),
    }
}
impl Tray {
    pub fn new(engine_apple: bool, lowercase: bool, parakeet_ready: bool) -> Result<Self, String> {
        let menu = Menu::new();
        let status = MenuItem::with_id("status", "Set a shortcut to start", false, None);
        let shortcut = MenuItem::with_id(ID_SHORTCUT, "Set Push-to-Talk Shortcut…", true, None);
        #[cfg(target_os = "macos")]
        let apple = CheckMenuItem::with_id(
            ID_ENGINE_APPLE,
            "Apple Speech (on-device)",
            true,
            engine_apple,
            None,
        );
        let parakeet = CheckMenuItem::with_id(
            ID_ENGINE_PARAKEET,
            "Parakeet (local, English)",
            true,
            !engine_apple,
            None,
        );
        let lowercase =
            CheckMenuItem::with_id(ID_LOWERCASE, "Lowercase output", true, lowercase, None);
        #[cfg(target_os = "macos")]
        let permissions = MenuItem::with_id(ID_PERMISSIONS, "Permissions…", true, None);
        let download = MenuItem::with_id(
            ID_DOWNLOAD,
            "Download model (460 MiB)",
            !parakeet_ready,
            None,
        );
        let quit = MenuItem::with_id(ID_QUIT, "Quit", true, None);
        menu.append(&status).map_err(|e| e.to_string())?;
        menu.append(&shortcut).map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        menu.append(&apple).map_err(|e| e.to_string())?;
        menu.append(&parakeet).map_err(|e| e.to_string())?;
        menu.append(&lowercase).map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        menu.append(&permissions).map_err(|e| e.to_string())?;
        menu.append(&download).map_err(|e| e.to_string())?;
        let about = AboutMetadata {
            name: Some("Lavtype".into()),
            version: Some(env!("CARGO_PKG_VERSION").into()),
            short_version: Some(env!("CARGO_PKG_VERSION").into()),
            authors: Some(vec!["Lavtype contributors".into()]),
            copyright: Some("Copyright (c) 2026 Lavtype contributors".into()),
            license: Some("MIT".into()),
            website: Some("https://github.com/lavtype/lavtype".into()),
            comments: Some("Tray-only push-to-talk transcription".into()),
            ..Default::default()
        };
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| e.to_string())?;
        menu.append(&PredefinedMenuItem::about(Some("Lavtype"), Some(about)))
            .map_err(|e| e.to_string())?;
        menu.append(&quit).map_err(|e| e.to_string())?;
        let icon = make_icon(TrayVisualState::Idle)?;
        #[allow(unused_mut)]
        let mut builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_icon(icon);
        #[cfg(target_os = "macos")]
        {
            builder = builder.with_icon_as_template(true);
        }
        let icon = builder.build().map_err(|e| e.to_string())?;
        let tray = Self {
            icon,
            menu,
            status,
            shortcut,
            #[cfg(target_os = "macos")]
            apple,
            parakeet,
            lowercase,
            #[cfg(target_os = "macos")]
            permissions,
            download,
            quit,
            last_published: RefCell::new(None),
        };
        tray.update(OperationState::Idle, None, parakeet_ready, false, None);
        Ok(tray)
    }

    pub fn set_engine(&self, apple_selected: bool) {
        #[cfg(target_os = "macos")]
        self.apple.set_checked(apple_selected);
        self.parakeet.set_checked(!apple_selected);
    }

    pub fn update(
        &self,
        state: OperationState,
        error: Option<&str>,
        parakeet_ready: bool,
        has_shortcut: bool,
        download_progress: Option<DownloadProgress>,
    ) {
        let snapshot = TraySnapshot {
            state,
            error: error.map(str::to_owned),
            parakeet_ready,
            has_shortcut,
            download_progress,
        };
        if !should_publish(self.last_published.borrow().as_ref(), &snapshot) {
            return;
        }
        let download_label = model_menu_label(state, download_progress, parakeet_ready);
        let status = match (state, error) {
            (_, Some(error)) if !error.is_empty() => error.to_string(),
            (OperationState::Recording, _) => "Recording…".to_string(),
            (OperationState::Transcribing, _) => "Transcribing…".to_string(),
            (OperationState::Downloading, _) => download_label.clone(),
            (OperationState::CapturingShortcut, _) => "Press a shortcut…".to_string(),
            (OperationState::Idle, _) if has_shortcut => "Ready".to_string(),
            _ => "Set a shortcut to start".to_string(),
        };
        self.status.set_text(status);
        self.download.set_text(download_label);
        let idle = state == OperationState::Idle;
        self.shortcut.set_enabled(idle);
        self.download.set_enabled(idle && !parakeet_ready);
        self.lowercase.set_enabled(idle);
        self.parakeet.set_enabled(idle);
        #[cfg(target_os = "macos")]
        self.apple.set_enabled(idle);
        self.quit.set_enabled(true);
        #[cfg(target_os = "macos")]
        self.permissions.set_enabled(idle);
        let visual = match state {
            OperationState::Recording => TrayVisualState::Recording,
            OperationState::Transcribing
            | OperationState::Downloading
            | OperationState::CapturingShortcut => TrayVisualState::Busy,
            OperationState::Idle => TrayVisualState::Idle,
        };
        if let Ok(icon) = make_icon(visual) {
            let _ = self
                .icon
                .set_icon_with_as_template(Some(icon), cfg!(target_os = "macos"));
        }
        *self.last_published.borrow_mut() = Some(snapshot);
    }
}

fn make_icon(state: TrayVisualState) -> Result<Icon, String> {
    // Small microphone glyph: transparent, dark pixels, with state marker.
    let mut rgba = vec![0_u8; 32 * 32 * 4];
    for y in 5..27 {
        for x in 11..21 {
            if (x == 11 || x == 20) && !(8..24).contains(&y) {
                continue;
            }
            let i = (y * 32 + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[32, 32, 32, 255]);
        }
    }
    for y in 21..28 {
        for x in 7..25 {
            if y == 21 || x == 7 || x == 24 {
                let i = (y * 32 + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[32, 32, 32, 255]);
            }
        }
    }
    let (r, g, b) = match state {
        TrayVisualState::Idle => (0, 180, 90),
        TrayVisualState::Recording => (220, 30, 30),
        TrayVisualState::Busy => (220, 150, 20),
    };
    for y in 2..8 {
        for x in 24..30 {
            if (x as i32 - 27).pow(2) + (y as i32 - 5).pow(2) <= 9 {
                let i = (y * 32 + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[r, g, b, 255]);
            }
        }
    }
    Icon::from_rgba(rgba, 32, 32).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_menu_distinguishes_missing_downloading_and_installed() {
        assert_eq!(
            model_menu_label(OperationState::Idle, None, false),
            "Download model (460 MiB)"
        );
        assert_eq!(
            model_menu_label(
                OperationState::Downloading,
                Some(DownloadProgress::new(50, 100)),
                false,
            ),
            "Downloading model… 50%"
        );
        assert_eq!(
            model_menu_label(OperationState::Idle, None, true),
            "Parakeet model installed"
        );
    }

    #[test]
    fn identical_tray_snapshot_is_not_republished() {
        let snapshot = TraySnapshot {
            state: OperationState::Idle,
            error: None,
            parakeet_ready: true,
            has_shortcut: true,
            download_progress: None,
        };
        let published = Some(snapshot.clone());
        assert!(!should_publish(published.as_ref(), &snapshot));

        let changed = TraySnapshot {
            state: OperationState::Recording,
            ..snapshot
        };
        assert!(should_publish(published.as_ref(), &changed));
    }
}
