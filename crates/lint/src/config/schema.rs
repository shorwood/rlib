use serde::Deserialize;

use super::providers::{
    CollectionProvider, DisplayProvider, ErrorImplementationProvider,
    ErrorVariantConversionProvider, PredicateProvider, StringParserProvider,
};

// -----------------------------------------------------------------------------
// BonBaselineEntry: Deserialized Bon compatibility entry
// -----------------------------------------------------------------------------

/// One configured Bon builder and its required member names.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(super) struct BonBaselineEntry {
    /// Published builder type name.
    pub(super) builder: String,
    /// Published required builder members.
    pub(super) members: Vec<String>,
}

// -----------------------------------------------------------------------------
// LegacyTable: Removed nested-table migration
// -----------------------------------------------------------------------------

/// One removed nested table and the flat replacement described to users.
struct LegacyTable {
    /// Removed table name.
    name: &'static str,
    /// Replacement key or key family.
    replacement: &'static str,
}

/// Removed nested tables recognized for explicit migration diagnostics.
const LEGACY_TABLES: &[LegacyTable] = &[
    LegacyTable {
        name: "bon_api_baseline",
        replacement: "bon-api-baseline",
    },
    LegacyTable {
        name: "derive_resolution",
        replacement: "the flat `*-provider` keys",
    },
    LegacyTable {
        name: "extension_traits",
        replacement: "extension-trait-methods-threshold",
    },
    LegacyTable {
        name: "function_structure",
        replacement: "the flat function and control-flow threshold keys",
    },
    LegacyTable {
        name: "leptos_architecture",
        replacement: "the flat `leptos-*-threshold` keys",
    },
    LegacyTable {
        name: "leptos_view_structure",
        replacement: "the flat Leptos view threshold keys",
    },
    LegacyTable {
        name: "leptos_view_formatting",
        replacement: "leptos-view-max-width",
    },
    LegacyTable {
        name: "leptos_styling_css_formatting",
        replacement: "leptos-css-max-width",
    },
    LegacyTable {
        name: "leptos_server_authorization",
        replacement: "the flat Leptos authorization keys",
    },
    LegacyTable {
        name: "miette_help",
        replacement: "miette-generic-help-phrases",
    },
    LegacyTable {
        name: "section_dividers",
        replacement: "declarations-per-section-threshold",
    },
];

// -----------------------------------------------------------------------------
// FileConfig: Flat deserialization schema
// -----------------------------------------------------------------------------

/// Deserialized flat configuration before semantic validation and assembly.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
#[expect(
    rlib::undocumented_items,
    reason = "the README key table is the canonical documentation for mechanical schema fields"
)]
pub(super) struct FileConfig {
    pub(super) function_phase_lines_threshold: usize,
    pub(super) control_flow_depth_threshold: usize,
    pub(super) match_arm_lines_threshold: usize,
    pub(super) method_chain_calls_threshold: usize,
    pub(super) declarations_per_section_threshold: usize,
    pub(super) extension_trait_methods_threshold: usize,
    pub(super) boolean_predicate_prefixes: Option<Vec<String>>,
    pub(super) boolean_query_roots: Option<Vec<String>>,
    pub(super) error_implementation_provider: Option<ErrorImplementationProvider>,
    pub(super) error_variant_conversion_provider: Option<ErrorVariantConversionProvider>,
    pub(super) enum_variant_collection_provider: Option<CollectionProvider>,
    pub(super) enum_variant_predicate_provider: Option<PredicateProvider>,
    pub(super) enum_string_parsing_provider: Option<StringParserProvider>,
    pub(super) enum_display_provider: Option<DisplayProvider>,
    pub(super) bon_api_baseline: Vec<BonBaselineEntry>,
    pub(super) leptos_setup_statements_threshold: usize,
    pub(super) leptos_reactive_primitives_threshold: usize,
    pub(super) leptos_event_handler_statements_threshold: usize,
    pub(super) leptos_event_handler_control_flow_depth_threshold: usize,
    pub(super) leptos_view_nesting_depth_threshold: usize,
    pub(super) leptos_view_control_flow_depth_threshold: usize,
    pub(super) leptos_component_composition_depth_threshold: usize,
    pub(super) leptos_component_props_threshold: usize,
    pub(super) leptos_components_per_module_threshold: usize,
    pub(super) leptos_repeated_view_fragment_nodes_threshold: usize,
    pub(super) leptos_repeated_view_fragment_occurrences_threshold: usize,
    pub(super) leptos_unnamed_view_complexity_threshold: usize,
    pub(super) leptos_view_section_complexity_threshold: usize,
    pub(super) leptos_unnamed_view_attribute_complexity_threshold: usize,
    pub(super) leptos_view_attribute_group_complexity_threshold: usize,
    pub(super) leptos_view_max_width: usize,
    pub(super) leptos_css_max_width: usize,
    pub(super) leptos_sensitive_call_terms: Option<Vec<String>>,
    pub(super) leptos_authorization_functions: Option<Vec<String>>,
    pub(super) leptos_protected_endpoint_attributes: Option<Vec<String>>,
    pub(super) leptos_public_endpoint_attributes: Option<Vec<String>>,
    pub(super) miette_generic_help_phrases: Option<Vec<String>>,
}

impl Default for FileConfig {
    fn default() -> Self {
        Self {
            function_phase_lines_threshold: 7,
            control_flow_depth_threshold: 2,
            match_arm_lines_threshold: 7,
            method_chain_calls_threshold: 3,
            declarations_per_section_threshold: 5,
            extension_trait_methods_threshold: 8,
            boolean_predicate_prefixes: None,
            boolean_query_roots: None,
            error_implementation_provider: None,
            error_variant_conversion_provider: None,
            enum_variant_collection_provider: None,
            enum_variant_predicate_provider: None,
            enum_string_parsing_provider: None,
            enum_display_provider: None,
            bon_api_baseline: Vec::new(),
            leptos_setup_statements_threshold: 8,
            leptos_reactive_primitives_threshold: 4,
            leptos_event_handler_statements_threshold: 3,
            leptos_event_handler_control_flow_depth_threshold: 1,
            leptos_view_nesting_depth_threshold: 7,
            leptos_view_control_flow_depth_threshold: 3,
            leptos_component_composition_depth_threshold: 10,
            leptos_component_props_threshold: 6,
            leptos_components_per_module_threshold: 8,
            leptos_repeated_view_fragment_nodes_threshold: 6,
            leptos_repeated_view_fragment_occurrences_threshold: 2,
            leptos_unnamed_view_complexity_threshold: 4,
            leptos_view_section_complexity_threshold: 4,
            leptos_unnamed_view_attribute_complexity_threshold: 6,
            leptos_view_attribute_group_complexity_threshold: 4,
            leptos_view_max_width: 100,
            leptos_css_max_width: 100,
            leptos_sensitive_call_terms: None,
            leptos_authorization_functions: None,
            leptos_protected_endpoint_attributes: None,
            leptos_public_endpoint_attributes: None,
            miette_generic_help_phrases: None,
        }
    }
}

impl FileConfig {
    /// Rejects removed nested tables with direct flat-key migration guidance.
    pub(super) fn reject_legacy_tables(value: &toml::Value) -> Result<(), String> {
        // Non-table roots are left to Serde's ordinary type diagnostic.
        let Some(table) = value.as_table() else {
            return Ok(());
        };
        let migrations = LEGACY_TABLES
            .iter()
            .filter(|legacy| table.contains_key(legacy.name))
            .map(|legacy| format!("`{}` was removed; use {}", legacy.name, legacy.replacement))
            .collect::<Vec<_>>();
        if migrations.is_empty() {
            Ok(())
        } else {
            Err(migrations.join("; "))
        }
    }
}
