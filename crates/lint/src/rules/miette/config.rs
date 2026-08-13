use serde::Deserialize;

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
/// Project vocabulary that is too vague to serve as diagnostic help.
pub struct MietteHelpConfig {
    /// Case-insensitive phrases that should be replaced by concrete recovery steps.
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
