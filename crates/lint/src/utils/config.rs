use serde::Deserialize;

use super::function_structure_config::FunctionStructureConfig;
use super::section_divider_config::SectionDividerConfig;
#[cfg(feature = "bon")]
use crate::rules::bon::utils::config::BonApiBaselineConfig;
use crate::rules::core::incoherent_extension_traits::ExtensionTraitConfig;
use crate::rules::framework::utils::config::DeriveResolutionConfig;
#[cfg(feature = "leptos")]
use crate::rules::leptos::leptos_server_functions_without_authorization_boundaries::LeptosServerAuthorizationConfig;
#[cfg(feature = "leptos")]
use crate::rules::leptos::utils::component_architecture::LeptosArchitectureConfig;
#[cfg(feature = "leptos")]
use crate::rules::leptos::utils::view_formatting::LeptosViewFormattingConfig;
#[cfg(feature = "leptos")]
use crate::rules::leptos::utils::view_structure::LeptosViewStructureConfig;
#[cfg(feature = "leptos_styling")]
use crate::rules::leptos_styling::utils::config::LeptosStylingCssFormattingConfig;
#[cfg(feature = "miette")]
use crate::rules::miette::utils::config::MietteHelpConfig;

// -----------------------------------------------------------------------------
// LibraryConfig: Complete lint library configuration
// -----------------------------------------------------------------------------
/// Every configurable policy exposed through the `rlib-lint` Dylint table.
#[derive(Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct LibraryConfig {
    /// Historical public Bon member snapshot used for compatibility checks.
    #[cfg(feature = "bon")]
    pub(crate) bon_api_baseline: BonApiBaselineConfig,
    /// Explicit provider choices for overlapping framework remediations.
    pub(crate) derive_resolution: DeriveResolutionConfig,
    /// Size policy used by focused extension-trait lints.
    pub(crate) extension_traits: ExtensionTraitConfig,
    /// Limits and syntax used by the function-structure lint family.
    pub(super) function_structure: FunctionStructureConfig,
    /// Complexity and heading policy used by Leptos view-structure lints.
    #[cfg(feature = "leptos")]
    pub(crate) leptos_view_structure: LeptosViewStructureConfig,
    /// Complexity and extraction policy for authored Leptos components.
    #[cfg(feature = "leptos")]
    pub(crate) leptos_architecture: LeptosArchitectureConfig,
    /// Canonical rendering policy for authored Leptos views.
    #[cfg(feature = "leptos")]
    pub(crate) leptos_view_formatting: LeptosViewFormattingConfig,
    /// Canonical rendering policy for component CSS.
    #[cfg(feature = "leptos_styling")]
    pub(crate) leptos_styling_css_formatting: LeptosStylingCssFormattingConfig,
    /// Explicit application vocabulary used to audit Leptos server endpoints.
    #[cfg(feature = "leptos")]
    pub(crate) leptos_server_authorization: LeptosServerAuthorizationConfig,
    /// Vocabulary used to identify non-actionable Miette help text.
    #[cfg(feature = "miette")]
    pub(crate) miette_help: MietteHelpConfig,
    /// Rendering and width policy used by section-divider lints.
    pub(super) section_dividers: SectionDividerConfig,
}

impl LibraryConfig {
    /// Loads the complete library configuration from Dylint's process environment.
    pub(crate) fn load() -> Self {
        dylint_linting::config_or_default(env!("CARGO_PKG_NAME"))
    }
}
