use malva::config::{FormatOptions, LineBreak};
use serde::Deserialize;

use crate::utils::config::LibraryConfig;

// -----------------------------------------------------------------------------
// LeptosStylingCssFormattingConfig: Canonical CSS rendering
// -----------------------------------------------------------------------------

/// Formatting policy for CSS files owned by `leptos_styling` macros.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct LeptosStylingCssFormattingConfig {
    /// Maximum rendered source width.
    max_width: usize,
}

impl Default for LeptosStylingCssFormattingConfig {
    fn default() -> Self {
        Self { max_width: 100 }
    }
}

impl LeptosStylingCssFormattingConfig {
    /// Loads and validates the configured CSS formatter policy.
    pub(crate) fn from_config() -> Self {
        let config = LibraryConfig::load().leptos_styling_css_formatting;
        assert!(
            config.max_width > 0,
            "leptos_styling_css_formatting.max_width must be greater than zero"
        );
        config
    }

    /// Builds deterministic Malva settings without declaration sorting.
    pub(crate) fn options(&self) -> FormatOptions {
        let mut options = FormatOptions::default();
        options.layout.print_width = self.max_width;
        options.layout.use_tabs = false;
        options.layout.indent_width = 2;
        options.layout.line_break = LineBreak::Lf;
        options.language.declaration_order = None;
        options
    }
}
