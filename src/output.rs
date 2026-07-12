//! Focused-application text output.

use enigo::{Enigo, Keyboard, Settings};
use thiserror::Error;

/// Errors from constructing the native input injector or typing text.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OutputError {
    #[error("could not initialize text injection: {0}")]
    Initialization(String),
    #[error("could not type text: {0}")]
    Typing(String),
}

/// Apply Lavtype's one output policy in one place.
///
/// Outer Unicode whitespace is removed first. Lowercasing is Unicode-aware and
/// applies to the complete trimmed string, preserving punctuation. Empty output
/// is represented as `None`, so callers never ask the injector to type nothing.
pub fn normalize_output(text: &str, lowercase: bool) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = if lowercase {
        trimmed.to_lowercase()
    } else {
        trimmed.to_owned()
    };
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

/// Alias emphasizing that this is the final policy before injection.
pub fn apply_output_policy(text: &str, lowercase: bool) -> Option<String> {
    normalize_output(text, lowercase)
}

/// Enigo-backed injector that types into whichever application is focused when
/// recognition completes. Lavtype deliberately has no clipboard fallback.
pub struct EnigoTextInjector {
    enigo: Enigo,
}

impl EnigoTextInjector {
    pub fn new() -> Result<Self, OutputError> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|error| OutputError::Initialization(error.to_string()))?;
        Ok(Self { enigo })
    }
}

impl crate::app::TextInjector for EnigoTextInjector {
    fn type_text(&mut self, text: &str) -> Result<(), OutputError> {
        self.enigo
            .text(text)
            .map_err(|error| OutputError::Typing(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_lowercases_unicode_output() {
        assert_eq!(
            normalize_output("  Hello, LAVTYPE!  ", true).as_deref(),
            Some("hello, lavtype!")
        );
        assert_eq!(
            normalize_output("  Hello, LAVTYPE!  ", false).as_deref(),
            Some("Hello, LAVTYPE!")
        );
        assert_eq!(normalize_output(" \n\t ", true), None);
    }

    #[test]
    fn lowercase_handles_unicode_expansion() {
        assert_eq!(normalize_output("  İ  ", true).as_deref(), Some("i\u{307}"));
    }
}
