use serde::Deserialize;

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
/// Carries the `MietteHelpConfig` state used by this analysis.
pub struct MietteHelpConfig {
    /// Stores the `generic_phrases` value used by this analysis.
    pub(super) generic_phrases: Vec<String>,
}

impl Default for MietteHelpConfig {
    fn default() -> Self {
        Self {
            generic_phrases: [
                "try again",
                "retry",
                "contact an administrator",
                "contact support",
                "an error occurred",
            ]
            .map(str::to_owned)
            .into(),
        }
    }
}
