use super::bon::BonApiBaselineConfig;
use super::core::{
    BooleanPredicateConfig, ExtensionTraitConfig, FunctionStructureConfig, SectionDividerConfig,
};
use super::framework::DeriveResolutionConfig;
use super::leptos::{
    LeptosArchitectureConfig, LeptosServerAuthorizationConfig, LeptosStylingCssFormattingConfig,
    LeptosViewFormattingConfig, LeptosViewStructureConfig,
};
use super::miette::MietteHelpConfig;
use super::schema::FileConfig;
use super::validation;

/// Name used by Dylint to locate this library's configuration table.
const LIBRARY_NAME: &str = env!("CARGO_PKG_NAME");

/// Validated configuration consumed by all lint passes.
pub struct Config {
    /// Shared boolean property and query vocabulary.
    pub(crate) boolean_predicates: BooleanPredicateConfig,
    /// Shared function-complexity policy.
    pub(crate) function_structure: FunctionStructureConfig,
    /// Shared module-section policy.
    pub(crate) section_dividers: SectionDividerConfig,
    /// Shared extension-trait policy.
    pub(crate) extension_traits: ExtensionTraitConfig,
    /// Selected derive providers.
    #[cfg_attr(not(any(feature = "strum", feature = "thiserror")), allow(dead_code))]
    pub(crate) derive_resolution: DeriveResolutionConfig,
    /// Baseline of published Bon builder members.
    #[cfg_attr(not(feature = "bon"), allow(dead_code))]
    pub(crate) bon_api_baseline: BonApiBaselineConfig,
    /// Leptos component architecture limits.
    #[cfg_attr(not(feature = "leptos"), allow(dead_code))]
    pub(crate) leptos_architecture: LeptosArchitectureConfig,
    /// Leptos view structure limits.
    #[cfg_attr(not(feature = "leptos"), allow(dead_code))]
    pub(crate) leptos_view_structure: LeptosViewStructureConfig,
    /// Leptos view formatter policy.
    #[cfg_attr(not(feature = "leptos"), allow(dead_code))]
    pub(crate) leptos_view_formatting: LeptosViewFormattingConfig,
    /// Leptos CSS formatter policy.
    #[cfg_attr(not(feature = "leptos_styling"), allow(dead_code))]
    pub(crate) leptos_styling_css_formatting: LeptosStylingCssFormattingConfig,
    /// Leptos server authorization vocabulary.
    #[cfg_attr(not(feature = "leptos"), allow(dead_code))]
    pub(crate) leptos_server_authorization: LeptosServerAuthorizationConfig,
    /// Miette help-text policy.
    #[cfg_attr(not(feature = "miette"), allow(dead_code))]
    pub(crate) miette_help: MietteHelpConfig,
}

impl Config {
    /// Reads the flat Dylint table and constructs validated runtime configuration.
    pub(super) fn load() -> Result<Self, String> {
        let value = dylint_linting::config_toml(LIBRARY_NAME)
            .map_err(|error| format!("could not read `{LIBRARY_NAME}` configuration: {error}"))?;

        // Missing configuration selects the complete built-in defaults.
        let Some(value) = value else {
            return Self::try_from(FileConfig::default());
        };
        FileConfig::reject_legacy_tables(&value)?;
        let file = value
            .try_into::<FileConfig>()
            .map_err(|error| format!("could not parse `{LIBRARY_NAME}` configuration: {error}"))?;
        Self::try_from(file)
    }

    /// Maps core function thresholds into runtime policy.
    const fn function_structure(file: &FileConfig) -> FunctionStructureConfig {
        FunctionStructureConfig {
            max_phase_lines: file.function_phase_lines_threshold,
            max_control_flow_depth: file.control_flow_depth_threshold,
            max_match_arm_lines: file.match_arm_lines_threshold,
            max_method_chain_calls: file.method_chain_calls_threshold,
        }
    }

    /// Maps the module section threshold into runtime policy.
    const fn section_dividers(file: &FileConfig) -> SectionDividerConfig {
        SectionDividerConfig {
            max_declarations_per_section: file.declarations_per_section_threshold,
        }
    }

    /// Maps the extension-trait threshold into runtime policy.
    const fn extension_traits(file: &FileConfig) -> ExtensionTraitConfig {
        ExtensionTraitConfig {
            max_methods: file.extension_trait_methods_threshold,
        }
    }

    /// Maps explicit derive provider selections into runtime policy.
    const fn derive_resolution(file: &FileConfig) -> DeriveResolutionConfig {
        DeriveResolutionConfig {
            error_implementation: file.error_implementation_provider,
            error_variant_conversion: file.error_variant_conversion_provider,
            enum_variant_collection: file.enum_variant_collection_provider,
            enum_variant_predicates: file.enum_variant_predicate_provider,
            enum_string_parsing: file.enum_string_parsing_provider,
            enum_display: file.enum_display_provider,
        }
    }

    /// Maps Leptos component thresholds into runtime policy.
    const fn leptos_architecture(file: &FileConfig) -> LeptosArchitectureConfig {
        LeptosArchitectureConfig {
            max_setup_statements: file.leptos_setup_statements_threshold,
            max_reactive_primitives: file.leptos_reactive_primitives_threshold,
            max_handler_statements: file.leptos_event_handler_statements_threshold,
            max_handler_control_flow_depth: file.leptos_event_handler_control_flow_depth_threshold,
            max_view_nesting_depth: file.leptos_view_nesting_depth_threshold,
            max_view_control_depth: file.leptos_view_control_flow_depth_threshold,
            max_component_composition_depth: file.leptos_component_composition_depth_threshold,
            max_component_props: file.leptos_component_props_threshold,
            max_components_per_module: file.leptos_components_per_module_threshold,
            min_repeated_fragment_nodes: file.leptos_repeated_view_fragment_nodes_threshold,
            min_repeated_fragment_occurrences: file
                .leptos_repeated_view_fragment_occurrences_threshold,
        }
    }

    /// Maps Leptos view thresholds into runtime policy.
    const fn leptos_view_structure(file: &FileConfig) -> LeptosViewStructureConfig {
        LeptosViewStructureConfig {
            max_unnamed_view_complexity: file.leptos_unnamed_view_complexity_threshold,
            max_view_section_complexity: file.leptos_view_section_complexity_threshold,
            max_unnamed_view_attribute_complexity: file
                .leptos_unnamed_view_attribute_complexity_threshold,
            max_view_attribute_group_complexity: file
                .leptos_view_attribute_group_complexity_threshold,
        }
    }

    /// Maps Leptos view formatting into runtime policy.
    const fn leptos_view_formatting(file: &FileConfig) -> LeptosViewFormattingConfig {
        LeptosViewFormattingConfig {
            max_width: file.leptos_view_max_width,
        }
    }

    /// Maps Leptos CSS formatting into runtime policy.
    const fn leptos_styling_css_formatting(file: &FileConfig) -> LeptosStylingCssFormattingConfig {
        LeptosStylingCssFormattingConfig {
            max_width: file.leptos_css_max_width,
        }
    }
}

impl TryFrom<FileConfig> for Config {
    type Error = String;

    fn try_from(file: FileConfig) -> Result<Self, Self::Error> {
        // Reject invalid scalar limits before assembling feature-specific policy.
        validation::thresholds(&file)?;

        // Assemble every validated domain policy from focused mappings.
        Ok(Self {
            boolean_predicates: BooleanPredicateConfig::try_from(&file)?,
            function_structure: Self::function_structure(&file),
            section_dividers: Self::section_dividers(&file),
            extension_traits: Self::extension_traits(&file),
            derive_resolution: Self::derive_resolution(&file),
            bon_api_baseline: BonApiBaselineConfig::try_from(file.bon_api_baseline.clone())?,
            leptos_architecture: Self::leptos_architecture(&file),
            leptos_view_structure: Self::leptos_view_structure(&file),
            leptos_view_formatting: Self::leptos_view_formatting(&file),
            leptos_styling_css_formatting: Self::leptos_styling_css_formatting(&file),
            leptos_server_authorization: LeptosServerAuthorizationConfig::try_from(&file)?,
            miette_help: MietteHelpConfig::try_from(&file)?,
        })
    }
}

// -----------------------------------------------------------------------------
// Tests: Unit tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::Config;
    use crate::config::schema::FileConfig;

    /// Parses one inline flat configuration through the production assembly path.
    fn parse(source: &str) -> Result<Config, String> {
        let file = toml::from_str::<FileConfig>(source).map_err(|error| error.to_string())?;
        Config::try_from(file)
    }

    #[test]
    fn parses_partial_flat_configuration() {
        let config = parse(
            r#"
                function-phase-lines-threshold = 11
                enum-display-provider = "strum_display"
                bon-api-baseline = [{ builder = "Request", members = ["host"] }]
            "#,
        )
        .expect("flat configuration should parse");
        assert_eq!(config.function_structure.max_phase_lines, 11);
        assert!(config.bon_api_baseline.contains_builder("Request"));
        assert!(config.derive_resolution.enum_variant_collection().is_none());
        assert!(config.derive_resolution.enum_variant_predicates().is_none());
        assert!(config.derive_resolution.enum_display().is_some());
        assert!(config.derive_resolution.enum_string_parsing().is_none());
    }

    #[test]
    fn extends_default_lists_at_the_sentinel() {
        let config = parse(r#"miette-generic-help-phrases = ["be specific", ".."]"#)
            .expect("default extension should parse");
        assert_eq!(config.miette_help.generic_phrases[0], "be specific");
        assert!(
            config
                .miette_help
                .generic_phrases
                .iter()
                .any(|phrase| phrase == "try again")
        );
    }

    #[test]
    fn applies_default_boolean_naming_vocabulary() {
        let config = parse("").expect("default configuration should parse");
        assert!(config.boolean_predicates.is_field_name("is_ready"));
        assert!(config.boolean_predicates.is_field_name("has_items"));
        assert!(config.boolean_predicates.is_field_name("should_retry"));
        assert!(!config.boolean_predicates.is_field_name("contains_items"));
        assert!(config.boolean_predicates.is_callable_name("contains"));
        assert!(
            config
                .boolean_predicates
                .is_callable_name("starts_with_prefix")
        );
        assert_eq!(
            config
                .boolean_predicates
                .semantic_callable_name("is_collection_candidate"),
            "collection_candidate"
        );
        assert_eq!(
            config
                .boolean_predicates
                .semantic_callable_name("contains_collection_candidate"),
            "collection_candidate"
        );
        assert!(!config.boolean_predicates.is_callable_name("contains_"));
        assert!(!config.boolean_predicates.is_callable_name("contains__item"));
    }

    #[test]
    fn replaces_boolean_naming_vocabulary() {
        let config = parse(
            r#"
                boolean-predicate-prefixes = ["can_"]
                boolean-query-roots = ["matches"]
            "#,
        )
        .expect("replacement vocabulary should parse");
        assert!(config.boolean_predicates.is_field_name("can_retry"));
        assert!(!config.boolean_predicates.is_field_name("is_ready"));
        assert!(config.boolean_predicates.is_callable_name("matches"));
        assert!(config.boolean_predicates.is_callable_name("matches_kind"));
        assert!(!config.boolean_predicates.is_callable_name("contains"));
    }

    #[test]
    fn extends_boolean_naming_vocabulary() {
        let config = parse(
            r#"
                boolean-predicate-prefixes = ["can_", ".."]
                boolean-query-roots = ["matches", ".."]
            "#,
        )
        .expect("extended vocabulary should parse");
        assert!(config.boolean_predicates.is_field_name("can_retry"));
        assert!(config.boolean_predicates.is_field_name("should_retry"));
        assert!(config.boolean_predicates.is_callable_name("matches_kind"));
        assert!(config.boolean_predicates.is_callable_name("contains_item"));
    }

    #[test]
    fn rejects_zero_thresholds() {
        assert!(parse("control-flow-depth-threshold = 0").is_err());
    }

    #[test]
    fn rejects_unknown_keys() {
        assert!(toml::from_str::<FileConfig>("max-phase-lines = 7").is_err());
    }

    #[test]
    fn rejects_duplicate_baseline_builders() {
        assert!(
            parse(
                r#"
                    bon-api-baseline = [
                        { builder = "Request", members = [] },
                        { builder = "Request", members = ["host"] },
                    ]
                "#,
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_invalid_list_entries() {
        assert!(parse(r#"miette-generic-help-phrases = ["", "use the source"]"#).is_err());
        assert!(parse(r#"miette-generic-help-phrases = ["retry", "retry"]"#).is_err());
        assert!(parse(r#"miette-generic-help-phrases = ["..", ".."]"#).is_err());
        assert!(parse(r"boolean-predicate-prefixes = []").is_err());
        assert!(parse(r#"boolean-predicate-prefixes = ["is"]"#).is_err());
        assert!(parse(r#"boolean-predicate-prefixes = ["is__"]"#).is_err());
        assert!(parse(r#"boolean-query-roots = ["contains_"]"#).is_err());
        assert!(
            parse(
                r#"
                    boolean-predicate-prefixes = ["matches_"]
                    boolean-query-roots = ["matches"]
                "#,
            )
            .is_err()
        );
    }

    #[test]
    fn identifies_removed_nested_tables() {
        let value = toml::from_str::<toml::Value>(
            r"
                [function_structure]
                max_phase_lines = 12
            ",
        )
        .expect("legacy configuration should remain valid TOML");
        let message = FileConfig::reject_legacy_tables(&value)
            .expect_err("legacy configuration should be rejected explicitly");
        assert!(message.contains("function_structure"));
        assert!(message.contains("flat function and control-flow threshold keys"));
    }
}
