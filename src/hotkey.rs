use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers as GlobalModifiers},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[cfg(target_os = "macos")]
use objc2_core_graphics::{CGEventSource, CGEventSourceStateID};
use std::fmt;
use tao::{
    event::{ElementState, WindowEvent},
    keyboard::{KeyCode, ModifiersState},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyName {
    ControlRight,
    Space,
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Backquote,
    Backslash,
    BracketLeft,
    BracketRight,
    Comma,
    Period,
    Slash,
    Semicolon,
    Quote,
    Minus,
    Equal,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

impl KeyName {
    pub fn to_code(self) -> Code {
        use Code::*;
        match self {
            Self::ControlRight => ControlRight,
            Self::Space => Space,
            Self::Enter => Enter,
            Self::Tab => Tab,
            Self::Escape => Escape,
            Self::Backspace => Backspace,
            Self::Delete => Delete,
            Self::Insert => Insert,
            Self::Home => Home,
            Self::End => End,
            Self::PageUp => PageUp,
            Self::PageDown => PageDown,
            Self::ArrowUp => ArrowUp,
            Self::ArrowDown => ArrowDown,
            Self::ArrowLeft => ArrowLeft,
            Self::ArrowRight => ArrowRight,
            Self::Backquote => Backquote,
            Self::Backslash => Backslash,
            Self::BracketLeft => BracketLeft,
            Self::BracketRight => BracketRight,
            Self::Comma => Comma,
            Self::Period => Period,
            Self::Slash => Slash,
            Self::Semicolon => Semicolon,
            Self::Quote => Quote,
            Self::Minus => Minus,
            Self::Equal => Equal,
            Self::Digit0 => Digit0,
            Self::Digit1 => Digit1,
            Self::Digit2 => Digit2,
            Self::Digit3 => Digit3,
            Self::Digit4 => Digit4,
            Self::Digit5 => Digit5,
            Self::Digit6 => Digit6,
            Self::Digit7 => Digit7,
            Self::Digit8 => Digit8,
            Self::Digit9 => Digit9,
            Self::A => KeyA,
            Self::B => KeyB,
            Self::C => KeyC,
            Self::D => KeyD,
            Self::E => KeyE,
            Self::F => KeyF,
            Self::G => KeyG,
            Self::H => KeyH,
            Self::I => KeyI,
            Self::J => KeyJ,
            Self::K => KeyK,
            Self::L => KeyL,
            Self::M => KeyM,
            Self::N => KeyN,
            Self::O => KeyO,
            Self::P => KeyP,
            Self::Q => KeyQ,
            Self::R => KeyR,
            Self::S => KeyS,
            Self::T => KeyT,
            Self::U => KeyU,
            Self::V => KeyV,
            Self::W => KeyW,
            Self::X => KeyX,
            Self::Y => KeyY,
            Self::Z => KeyZ,
            Self::F1 => F1,
            Self::F2 => F2,
            Self::F3 => F3,
            Self::F4 => F4,
            Self::F5 => F5,
            Self::F6 => F6,
            Self::F7 => F7,
            Self::F8 => F8,
            Self::F9 => F9,
            Self::F10 => F10,
            Self::F11 => F11,
            Self::F12 => F12,
        }
    }

    pub fn from_tao(code: KeyCode) -> Option<Self> {
        use KeyCode::*;
        Some(match code {
            ControlRight => Self::ControlRight,
            Space => Self::Space,
            Enter => Self::Enter,
            Tab => Self::Tab,
            Escape => Self::Escape,
            Backspace => Self::Backspace,
            Delete => Self::Delete,
            Insert => Self::Insert,
            Home => Self::Home,
            End => Self::End,
            PageUp => Self::PageUp,
            PageDown => Self::PageDown,
            ArrowUp => Self::ArrowUp,
            ArrowDown => Self::ArrowDown,
            ArrowLeft => Self::ArrowLeft,
            ArrowRight => Self::ArrowRight,
            Backquote => Self::Backquote,
            Backslash => Self::Backslash,
            BracketLeft => Self::BracketLeft,
            BracketRight => Self::BracketRight,
            Comma => Self::Comma,
            Period => Self::Period,
            Slash => Self::Slash,
            Semicolon => Self::Semicolon,
            Quote => Self::Quote,
            Minus => Self::Minus,
            Equal => Self::Equal,
            Digit0 => Self::Digit0,
            Digit1 => Self::Digit1,
            Digit2 => Self::Digit2,
            Digit3 => Self::Digit3,
            Digit4 => Self::Digit4,
            Digit5 => Self::Digit5,
            Digit6 => Self::Digit6,
            Digit7 => Self::Digit7,
            Digit8 => Self::Digit8,
            Digit9 => Self::Digit9,
            KeyA => Self::A,
            KeyB => Self::B,
            KeyC => Self::C,
            KeyD => Self::D,
            KeyE => Self::E,
            KeyF => Self::F,
            KeyG => Self::G,
            KeyH => Self::H,
            KeyI => Self::I,
            KeyJ => Self::J,
            KeyK => Self::K,
            KeyL => Self::L,
            KeyM => Self::M,
            KeyN => Self::N,
            KeyO => Self::O,
            KeyP => Self::P,
            KeyQ => Self::Q,
            KeyR => Self::R,
            KeyS => Self::S,
            KeyT => Self::T,
            KeyU => Self::U,
            KeyV => Self::V,
            KeyW => Self::W,
            KeyX => Self::X,
            KeyY => Self::Y,
            KeyZ => Self::Z,
            F1 => Self::F1,
            F2 => Self::F2,
            F3 => Self::F3,
            F4 => Self::F4,
            F5 => Self::F5,
            F6 => Self::F6,
            F7 => Self::F7,
            F8 => Self::F8,
            F9 => Self::F9,
            F10 => Self::F10,
            F11 => Self::F11,
            F12 => Self::F12,
            _ => return None,
        })
    }
}

impl fmt::Display for KeyName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::ControlRight => "RightControl",
            Self::Space => "Space",
            Self::Enter => "Enter",
            Self::Tab => "Tab",
            Self::Escape => "Escape",
            Self::Backspace => "Backspace",
            Self::Delete => "Delete",
            Self::Insert => "Insert",
            Self::Home => "Home",
            Self::End => "End",
            Self::PageUp => "PageUp",
            Self::PageDown => "PageDown",
            Self::ArrowUp => "ArrowUp",
            Self::ArrowDown => "ArrowDown",
            Self::ArrowLeft => "ArrowLeft",
            Self::ArrowRight => "ArrowRight",
            Self::Backquote => "Backquote",
            Self::Backslash => "Backslash",
            Self::BracketLeft => "BracketLeft",
            Self::BracketRight => "BracketRight",
            Self::Comma => "Comma",
            Self::Period => "Period",
            Self::Slash => "Slash",
            Self::Semicolon => "Semicolon",
            Self::Quote => "Quote",
            Self::Minus => "Minus",
            Self::Equal => "Equal",
            Self::Digit0 => "Digit0",
            Self::Digit1 => "Digit1",
            Self::Digit2 => "Digit2",
            Self::Digit3 => "Digit3",
            Self::Digit4 => "Digit4",
            Self::Digit5 => "Digit5",
            Self::Digit6 => "Digit6",
            Self::Digit7 => "Digit7",
            Self::Digit8 => "Digit8",
            Self::Digit9 => "Digit9",
            Self::A => "KeyA",
            Self::B => "KeyB",
            Self::C => "KeyC",
            Self::D => "KeyD",
            Self::E => "KeyE",
            Self::F => "KeyF",
            Self::G => "KeyG",
            Self::H => "KeyH",
            Self::I => "KeyI",
            Self::J => "KeyJ",
            Self::K => "KeyK",
            Self::L => "KeyL",
            Self::M => "KeyM",
            Self::N => "KeyN",
            Self::O => "KeyO",
            Self::P => "KeyP",
            Self::Q => "KeyQ",
            Self::R => "KeyR",
            Self::S => "KeyS",
            Self::T => "KeyT",
            Self::U => "KeyU",
            Self::V => "KeyV",
            Self::W => "KeyW",
            Self::X => "KeyX",
            Self::Y => "KeyY",
            Self::Z => "KeyZ",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::F10 => "F10",
            Self::F11 => "F11",
            Self::F12 => "F12",
        };
        f.write_str(name)
    }
}

impl std::str::FromStr for KeyName {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let v = value.to_ascii_lowercase();
        let key = match v.as_str() {
            "controlright" | "rightcontrol" | "right-ctrl" => Self::ControlRight,
            "space" => Self::Space,
            "enter" | "return" => Self::Enter,
            "tab" => Self::Tab,
            "escape" | "esc" => Self::Escape,
            "backspace" => Self::Backspace,
            "delete" => Self::Delete,
            "insert" => Self::Insert,
            "home" => Self::Home,
            "end" => Self::End,
            "pageup" => Self::PageUp,
            "pagedown" => Self::PageDown,
            "arrowup" | "up" => Self::ArrowUp,
            "arrowdown" | "down" => Self::ArrowDown,
            "arrowleft" | "left" => Self::ArrowLeft,
            "arrowright" | "right" => Self::ArrowRight,
            "backquote" => Self::Backquote,
            "backslash" => Self::Backslash,
            "bracketleft" => Self::BracketLeft,
            "bracketright" => Self::BracketRight,
            "comma" => Self::Comma,
            "period" => Self::Period,
            "slash" => Self::Slash,
            "semicolon" => Self::Semicolon,
            "quote" => Self::Quote,
            "minus" => Self::Minus,
            "equal" => Self::Equal,
            "digit0" => Self::Digit0,
            "digit1" => Self::Digit1,
            "digit2" => Self::Digit2,
            "digit3" => Self::Digit3,
            "digit4" => Self::Digit4,
            "digit5" => Self::Digit5,
            "digit6" => Self::Digit6,
            "digit7" => Self::Digit7,
            "digit8" => Self::Digit8,
            "digit9" => Self::Digit9,
            "keya" | "a" => Self::A,
            "keyb" | "b" => Self::B,
            "keyc" | "c" => Self::C,
            "keyd" | "d" => Self::D,
            "keye" | "e" => Self::E,
            "keyf" | "f" => Self::F,
            "keyg" | "g" => Self::G,
            "keyh" | "h" => Self::H,
            "keyi" | "i" => Self::I,
            "keyj" | "j" => Self::J,
            "keyk" | "k" => Self::K,
            "keyl" | "l" => Self::L,
            "keym" | "m" => Self::M,
            "keyn" | "n" => Self::N,
            "keyo" | "o" => Self::O,
            "keyp" | "p" => Self::P,
            "keyq" | "q" => Self::Q,
            "keyr" | "r" => Self::R,
            "keys" | "s" => Self::S,
            "keyt" | "t" => Self::T,
            "keyu" | "u" => Self::U,
            "keyv" | "v" => Self::V,
            "keyw" | "w" => Self::W,
            "keyx" | "x" => Self::X,
            "keyy" | "y" => Self::Y,
            "keyz" | "z" => Self::Z,
            "f1" => Self::F1,
            "f2" => Self::F2,
            "f3" => Self::F3,
            "f4" => Self::F4,
            "f5" => Self::F5,
            "f6" => Self::F6,
            "f7" => Self::F7,
            "f8" => Self::F8,
            "f9" => Self::F9,
            "f10" => Self::F10,
            "f11" => Self::F11,
            "f12" => Self::F12,
            _ => return Err(format!("unsupported shortcut key: {value}")),
        };
        Ok(key)
    }
}

impl Serialize for KeyName {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for KeyName {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u8);
impl Modifiers {
    pub const CONTROL: Self = Self(1);
    pub const ALT: Self = Self(2);
    pub const SHIFT: Self = Self(4);
    pub const META: Self = Self(8);
    pub const fn empty() -> Self {
        Self(0)
    }
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    fn from_tao(m: ModifiersState) -> Self {
        let mut out = Self::empty();
        if m.control_key() {
            out |= Self::CONTROL;
        }
        if m.alt_key() {
            out |= Self::ALT;
        }
        if m.shift_key() {
            out |= Self::SHIFT;
        }
        if m.super_key() {
            out |= Self::META;
        }
        out
    }
    pub fn native_labels(self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.contains(Self::CONTROL) {
            out.push(if cfg!(target_os = "macos") {
                "⌃"
            } else {
                "Ctrl"
            });
        }
        if self.contains(Self::ALT) {
            out.push(if cfg!(target_os = "macos") {
                "⌥"
            } else {
                "Alt"
            });
        }
        if self.contains(Self::SHIFT) {
            out.push(if cfg!(target_os = "macos") {
                "⇧"
            } else {
                "Shift"
            });
        }
        if self.contains(Self::META) {
            out.push(if cfg!(target_os = "macos") {
                "⌘"
            } else {
                "Super"
            });
        }
        out
    }
    pub fn display(self, key: KeyName) -> String {
        let mut labels = self
            .native_labels()
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        labels.push(key.to_string());
        labels.join(if cfg!(target_os = "macos") { "" } else { "+" })
    }
}
impl std::ops::BitOr for Modifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
impl std::ops::BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
impl Serialize for Modifiers {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut values = Vec::new();
        if self.contains(Self::CONTROL) {
            values.push("control");
        }
        if self.contains(Self::ALT) {
            values.push("alt");
        }
        if self.contains(Self::SHIFT) {
            values.push("shift");
        }
        if self.contains(Self::META) {
            values.push("meta");
        }
        values.serialize(s)
    }
}
impl<'de> Deserialize<'de> for Modifiers {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let values = Vec::<String>::deserialize(d)?;
        let mut m = Self::empty();
        for value in values {
            match value.to_ascii_lowercase().as_str() {
                "control" | "ctrl" => m |= Self::CONTROL,
                "alt" | "option" => m |= Self::ALT,
                "shift" => m |= Self::SHIFT,
                "meta" | "super" | "command" | "cmd" => m |= Self::META,
                _ => {
                    return Err(serde::de::Error::custom(format!(
                        "unsupported modifier: {value}"
                    )));
                }
            }
        }
        Ok(m)
    }
}
impl Modifiers {
    fn global(self) -> GlobalModifiers {
        let mut m = GlobalModifiers::empty();
        if self.contains(Self::CONTROL) {
            m |= GlobalModifiers::CONTROL;
        }
        if self.contains(Self::ALT) {
            m |= GlobalModifiers::ALT;
        }
        if self.contains(Self::SHIFT) {
            m |= GlobalModifiers::SHIFT;
        }
        if self.contains(Self::META) {
            m |= GlobalModifiers::SUPER;
        }
        m
    }
    pub fn to_global(self) -> GlobalModifiers {
        self.global()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shortcut {
    pub key: KeyName,
    pub modifiers: Modifiers,
}
impl Shortcut {
    pub const fn new(key: KeyName, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }
    pub fn hotkey(self) -> HotKey {
        HotKey::new(Some(self.modifiers.global()), self.key.to_code())
    }
    pub fn to_hotkey(self) -> HotKey {
        self.hotkey()
    }
}
#[derive(Debug, thiserror::Error)]
pub enum HotkeyError {
    #[error("could not register shortcut: {0}")]
    Register(#[source] global_hotkey::Error),
    #[error("could not unregister shortcut: {0}")]
    Unregister(#[source] global_hotkey::Error),
}

#[cfg(target_os = "macos")]
pub fn right_control_is_down() -> bool {
    CGEventSource::key_state(CGEventSourceStateID::CombinedSessionState, 62)
}

pub fn key_state_transition(previous: &mut bool, current: bool) -> Option<HotKeyState> {
    if *previous == current {
        return None;
    }
    *previous = current;
    Some(if current {
        HotKeyState::Pressed
    } else {
        HotKeyState::Released
    })
}

pub struct RegisteredShortcut {
    shortcut: Shortcut,
    hotkey: HotKey,
}
pub trait HotkeyRegistrar {
    fn register_hotkey(&mut self, hotkey: HotKey) -> Result<(), global_hotkey::Error>;
    fn unregister_hotkey(&mut self, hotkey: HotKey) -> Result<(), global_hotkey::Error>;
}
impl HotkeyRegistrar for GlobalHotKeyManager {
    fn register_hotkey(&mut self, hotkey: HotKey) -> Result<(), global_hotkey::Error> {
        self.register(hotkey)
    }
    fn unregister_hotkey(&mut self, hotkey: HotKey) -> Result<(), global_hotkey::Error> {
        self.unregister(hotkey)
    }
}
impl RegisteredShortcut {
    pub fn register(
        manager: &GlobalHotKeyManager,
        shortcut: Shortcut,
    ) -> Result<Self, HotkeyError> {
        #[cfg(target_os = "macos")]
        if shortcut.key == KeyName::ControlRight {
            return Ok(Self {
                shortcut,
                hotkey: shortcut.hotkey(),
            });
        }

        Self::register_with(&mut ManagerRef(manager), shortcut)
    }

    pub fn register_with<R: HotkeyRegistrar>(
        registrar: &mut R,
        shortcut: Shortcut,
    ) -> Result<Self, HotkeyError> {
        let hotkey = shortcut.hotkey();
        registrar
            .register_hotkey(hotkey)
            .map_err(HotkeyError::Register)?;
        Ok(Self { shortcut, hotkey })
    }

    pub fn new(manager: &GlobalHotKeyManager, shortcut: Shortcut) -> Result<Self, HotkeyError> {
        Self::register(manager, shortcut)
    }

    pub fn replace(
        &mut self,
        manager: &GlobalHotKeyManager,
        replacement: Shortcut,
    ) -> Result<(), HotkeyError> {
        #[cfg(target_os = "macos")]
        if self.shortcut.key == KeyName::ControlRight || replacement.key == KeyName::ControlRight {
            return self.replace_with_native(manager, replacement);
        }
        self.replace_with(&mut ManagerRef(manager), replacement)
    }

    #[cfg(target_os = "macos")]
    fn replace_with_native(
        &mut self,
        manager: &GlobalHotKeyManager,
        replacement: Shortcut,
    ) -> Result<(), HotkeyError> {
        let old_is_polled = self.shortcut.key == KeyName::ControlRight;
        let next_is_polled = replacement.key == KeyName::ControlRight;
        let next = replacement.hotkey();

        match (old_is_polled, next_is_polled) {
            (true, true) => {}
            (true, false) => manager.register(next).map_err(HotkeyError::Register)?,
            (false, true) => manager
                .unregister(self.hotkey)
                .map_err(HotkeyError::Unregister)?,
            (false, false) => unreachable!("native replacement requires Right Control"),
        }

        self.shortcut = replacement;
        self.hotkey = next;
        Ok(())
    }

    pub fn replace_with<R: HotkeyRegistrar>(
        &mut self,
        registrar: &mut R,
        replacement: Shortcut,
    ) -> Result<(), HotkeyError> {
        let next = replacement.hotkey();
        registrar
            .register_hotkey(next)
            .map_err(HotkeyError::Register)?;
        if let Err(error) = registrar.unregister_hotkey(self.hotkey) {
            let _ = registrar.unregister_hotkey(next);
            return Err(HotkeyError::Unregister(error));
        }
        self.shortcut = replacement;
        self.hotkey = next;
        Ok(())
    }

    pub fn shortcut(&self) -> Shortcut {
        self.shortcut
    }

    pub fn id(&self) -> u32 {
        self.hotkey.id()
    }
    /// Standalone Right Control is derived exclusively from the physical key
    /// state on macOS. Mixing event-monitor and polling transitions can pair a
    /// press from one source with a stale release from the other.
    pub fn uses_physical_polling(&self) -> bool {
        cfg!(target_os = "macos") && self.shortcut.key == KeyName::ControlRight
    }

    pub fn hotkey(&self) -> HotKey {
        self.hotkey
    }
}

struct ManagerRef<'a>(&'a GlobalHotKeyManager);
impl HotkeyRegistrar for ManagerRef<'_> {
    fn register_hotkey(&mut self, hotkey: HotKey) -> Result<(), global_hotkey::Error> {
        self.0.register(hotkey)
    }
    fn unregister_hotkey(&mut self, hotkey: HotKey) -> Result<(), global_hotkey::Error> {
        self.0.unregister(hotkey)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PressReleaseState {
    pressed: bool,
}
impl PressReleaseState {
    pub fn pressed(&self) -> bool {
        self.pressed
    }
    pub fn accept(&mut self, event: GlobalHotKeyEvent, expected_id: u32) -> Option<HotKeyState> {
        if event.id != expected_id {
            return None;
        }
        match event.state {
            HotKeyState::Pressed if !self.pressed => {
                self.pressed = true;
                Some(HotKeyState::Pressed)
            }
            HotKeyState::Released if self.pressed => {
                self.pressed = false;
                Some(HotKeyState::Released)
            }
            _ => None,
        }
    }
    pub fn reset(&mut self) {
        self.pressed = false;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecorderAction {
    Pending,
    Completed(Shortcut),
    Cancelled,
    Ignored,
}

#[derive(Debug, Default)]
pub struct ShortcutRecorder {
    modifiers: ModifiersState,
    candidate: Option<(KeyName, Modifiers)>,
}
impl ShortcutRecorder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn modifiers_changed(&mut self, modifiers: ModifiersState) {
        self.modifiers = modifiers;
    }
    pub fn handle_window_event(&mut self, event: &WindowEvent<'_>) -> RecorderAction {
        match event {
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers_changed(*modifiers);
                RecorderAction::Pending
            }
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_key(event.physical_key, event.state)
            }
            _ => RecorderAction::Pending,
        }
    }
    pub fn handle_key(&mut self, code: KeyCode, state: ElementState) -> RecorderAction {
        if code == KeyCode::Escape && state == ElementState::Pressed {
            self.candidate = None;
            return RecorderAction::Cancelled;
        }
        if code == KeyCode::ControlRight {
            return match state {
                ElementState::Pressed => {
                    self.candidate = Some((KeyName::ControlRight, Modifiers::default()));
                    RecorderAction::Pending
                }
                ElementState::Released
                    if self.candidate == Some((KeyName::ControlRight, Modifiers::default())) =>
                {
                    self.candidate = None;
                    RecorderAction::Completed(Shortcut::new(
                        KeyName::ControlRight,
                        Modifiers::default(),
                    ))
                }
                _ => RecorderAction::Ignored,
            };
        }
        if matches!(
            code,
            KeyCode::ControlLeft
                | KeyCode::AltLeft
                | KeyCode::AltRight
                | KeyCode::ShiftLeft
                | KeyCode::ShiftRight
                | KeyCode::SuperLeft
                | KeyCode::SuperRight
        ) {
            return RecorderAction::Pending;
        }
        let Some(key) = KeyName::from_tao(code) else {
            return RecorderAction::Ignored;
        };
        if state == ElementState::Pressed {
            let modifiers = Modifiers::from_tao(self.modifiers);
            if modifiers.is_empty()
                && !matches!(
                    key,
                    KeyName::F1
                        | KeyName::F2
                        | KeyName::F3
                        | KeyName::F4
                        | KeyName::F5
                        | KeyName::F6
                        | KeyName::F7
                        | KeyName::F8
                        | KeyName::F9
                        | KeyName::F10
                        | KeyName::F11
                        | KeyName::F12
                )
            {
                return RecorderAction::Ignored;
            }
            self.candidate = Some((key, modifiers));
            RecorderAction::Pending
        } else if let Some((candidate, modifiers)) = self.candidate {
            if candidate == key {
                self.candidate = None;
                RecorderAction::Completed(Shortcut::new(candidate, modifiers))
            } else {
                RecorderAction::Ignored
            }
        } else {
            RecorderAction::Ignored
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_and_unmatched_events_are_ignored() {
        let mut state = PressReleaseState::default();
        let press = GlobalHotKeyEvent {
            id: 7,
            state: HotKeyState::Pressed,
        };
        let release = GlobalHotKeyEvent {
            id: 7,
            state: HotKeyState::Released,
        };
        assert_eq!(state.accept(press, 7), Some(HotKeyState::Pressed));
        assert_eq!(state.accept(press, 7), None);
        assert_eq!(
            state.accept(
                GlobalHotKeyEvent {
                    id: 9,
                    state: HotKeyState::Released
                },
                7
            ),
            None
        );
        assert_eq!(state.accept(release, 7), Some(HotKeyState::Released));
        assert_eq!(state.accept(release, 7), None);
    }
    #[test]
    fn recorder_rejects_bare_typing_and_accepts_modified_key() {
        let mut recorder = ShortcutRecorder::new();
        assert_eq!(
            recorder.handle_key(KeyCode::KeyA, ElementState::Pressed),
            RecorderAction::Ignored
        );
        recorder.modifiers_changed(ModifiersState::CONTROL | ModifiersState::SHIFT);
        assert_eq!(
            recorder.handle_key(KeyCode::Space, ElementState::Pressed),
            RecorderAction::Pending
        );
        assert_eq!(
            recorder.handle_key(KeyCode::Space, ElementState::Released),
            RecorderAction::Completed(Shortcut::new(
                KeyName::Space,
                Modifiers::CONTROL | Modifiers::SHIFT
            ))
        );
    }

    #[test]
    fn recorder_accepts_standalone_right_control() {
        let mut recorder = ShortcutRecorder::new();
        assert_eq!(
            recorder.handle_key(KeyCode::ControlRight, ElementState::Pressed),
            RecorderAction::Pending
        );
        assert_eq!(
            recorder.handle_key(KeyCode::ControlRight, ElementState::Released),
            RecorderAction::Completed(Shortcut::new(KeyName::ControlRight, Modifiers::default()))
        );
    }

    #[test]
    fn physical_key_polling_emits_only_edges() {
        let mut previous = false;
        assert_eq!(key_state_transition(&mut previous, false), None);
        assert_eq!(
            key_state_transition(&mut previous, true),
            Some(HotKeyState::Pressed)
        );
        assert_eq!(key_state_transition(&mut previous, true), None);
        assert_eq!(
            key_state_transition(&mut previous, false),
            Some(HotKeyState::Released)
        );
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn standalone_right_control_has_only_the_physical_polling_backend() {
        let shortcut = Shortcut::new(KeyName::ControlRight, Modifiers::default());
        let binding = RegisteredShortcut {
            shortcut,
            hotkey: shortcut.hotkey(),
        };
        assert!(binding.uses_physical_polling());

        let shortcut = Shortcut::new(KeyName::Space, Modifiers::ALT);
        let binding = RegisteredShortcut {
            shortcut,
            hotkey: shortcut.hotkey(),
        };
        assert!(!binding.uses_physical_polling());
    }

    #[test]
    fn replacement_registers_before_removing_old_and_preserves_on_collision() {
        struct Fake {
            registered: Vec<HotKey>,
        }
        impl HotkeyRegistrar for Fake {
            fn register_hotkey(&mut self, hotkey: HotKey) -> Result<(), global_hotkey::Error> {
                if self.registered.contains(&hotkey) {
                    return Err(global_hotkey::Error::AlreadyRegistered(hotkey));
                }
                self.registered.push(hotkey);
                Ok(())
            }
            fn unregister_hotkey(&mut self, hotkey: HotKey) -> Result<(), global_hotkey::Error> {
                let Some(index) = self
                    .registered
                    .iter()
                    .position(|current| *current == hotkey)
                else {
                    return Err(global_hotkey::Error::FailedToUnRegister(hotkey));
                };
                self.registered.remove(index);
                Ok(())
            }
        }
        let first = Shortcut::new(KeyName::Space, Modifiers::CONTROL);
        let second = Shortcut::new(KeyName::A, Modifiers::CONTROL);
        let mut fake = Fake {
            registered: Vec::new(),
        };
        let mut current = RegisteredShortcut::register_with(&mut fake, first).unwrap();
        assert!(current.replace_with(&mut fake, second).is_ok());
        assert_eq!(current.shortcut(), second);
        assert!(current.replace_with(&mut fake, second).is_err());
        assert_eq!(current.shortcut(), second);
    }
}
