use leptosfmt_formatter::{
    AttributeValueBraceStyle, ClosingTagStyle, FormatterSettings, IndentationStyle, NewlineStyle,
};
use serde::Deserialize;

use crate::utils::config::LibraryConfig;

// -----------------------------------------------------------------------------
// LeptosViewFormattingConfig: Canonical RSX rendering
// -----------------------------------------------------------------------------

/// Formatting policy for authored Leptos `view!` macros.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct LeptosViewFormattingConfig {
    /// Maximum rendered source width.
    max_width: usize,
}

impl Default for LeptosViewFormattingConfig {
    fn default() -> Self {
        Self { max_width: 100 }
    }
}

impl LeptosViewFormattingConfig {
    /// Loads and validates the configured view formatter policy.
    pub(crate) fn from_config() -> Self {
        let config = LibraryConfig::load().leptos_view_formatting;
        assert!(
            config.max_width > 0,
            "leptos_view_formatting.max_width must be greater than zero"
        );
        config
    }

    /// Builds deterministic leptosfmt settings from the public policy.
    pub(crate) fn settings(&self) -> FormatterSettings {
        FormatterSettings {
            max_width: self.max_width,
            tab_spaces: 4,
            indentation_style: IndentationStyle::Spaces,
            newline_style: NewlineStyle::Unix,
            attr_value_brace_style: AttributeValueBraceStyle::WhenRequired,
            closing_tag_style: ClosingTagStyle::Preserve,
            ..FormatterSettings::default()
        }
    }
}
