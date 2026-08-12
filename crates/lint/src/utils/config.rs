use serde::Deserialize;

use super::function_structure::FunctionStructureConfig;
use super::section_analysis::SectionDividerConfig;
use crate::rules::core::incoherent_extension_traits::ExtensionTraitConfig;
use crate::rules::framework::config::DeriveResolutionConfig;
#[cfg(feature = "leptos")]
use crate::rules::leptos::utils::view_structure::LeptosViewStructureConfig;

// -----------------------------------------------------------------------------
// LibraryConfig: Complete lint library configuration
// -----------------------------------------------------------------------------

/// Every configurable policy exposed through the `rlib-lint` Dylint table.
#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LibraryConfig {
    /// Explicit provider choices for overlapping framework remediations.
    pub(crate) derive_resolution: DeriveResolutionConfig,
    /// Size policy used by focused extension-trait lints.
    pub(crate) extension_traits: ExtensionTraitConfig,
    /// Limits and syntax used by the function-structure lint family.
    pub(super) function_structure: FunctionStructureConfig,
    /// Complexity and heading policy used by Leptos view-structure lints.
    #[cfg(feature = "leptos")]
    pub(crate) leptos_view_structure: LeptosViewStructureConfig,
    /// Rendering and width policy used by section-divider lints.
    pub(super) section_dividers: SectionDividerConfig,
}

impl LibraryConfig {
    /// Loads the complete library configuration from Dylint's process environment.
    pub(crate) fn load() -> Self {
        dylint_linting::config_or_default(env!("CARGO_PKG_NAME"))
    }
}

// -----------------------------------------------------------------------------
// Tests: Unit tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::LibraryConfig;

    #[test]
    fn parses_custom_extension_trait_limit() {
        let config = toml::from_str::<LibraryConfig>(
            r"
                [extension_traits]
                max_methods = 5
            ",
        )
        .expect("custom extension trait limit should parse");
        assert_eq!(config.extension_traits.max_methods, 5);
    }
}
