use serde::Deserialize;

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct MietteHelpConfig {
    pub(crate) generic_phrases: Vec<String>,
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
