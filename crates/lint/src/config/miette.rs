#![cfg_attr(not(feature = "miette"), allow(dead_code))]

use super::schema::FileConfig;
use super::validation;

/// Default phrases considered too generic for diagnostic help.
const GENERIC_HELP_PHRASES: &[&str] = &[
    "try again",
    "retry",
    "contact an administrator",
    "contact support",
    "an error occurred",
];

/// Validated phrases considered too generic for Miette diagnostic help.
#[derive(Clone)]
pub struct MietteHelpConfig {
    /// Case-normalized phrases matched against diagnostic help text.
    pub(crate) generic_phrases: Vec<String>,
}

impl TryFrom<&FileConfig> for MietteHelpConfig {
    type Error = String;

    fn try_from(file: &FileConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            generic_phrases: validation::ConfigList::resolve(
                "miette-generic-help-phrases",
                file.miette_generic_help_phrases.clone(),
                GENERIC_HELP_PHRASES,
            )?,
        })
    }
}
