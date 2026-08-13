extern crate rustc_lint;
extern crate rustc_session;

use rustc_session::lint::LintId;

use crate::rules;

// Registers this library as a dylint plugin with the compiler process that loaded it.
dylint_linting::dylint_library!();

/// Registers the Bon-aware lint layer.
#[cfg(feature = "bon")]
fn register_bon_lints(sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    rules::bon::bon_builders_bypassing_construction_invariants::register_lints(sess, lint_store);
    rules::bon::bon_escaping_incomplete_builders::register_lints(sess, lint_store);
    rules::bon::bon_implicit_optional_builder_members::register_lints(sess, lint_store);
    rules::bon::bon_incoherent_conditional_builder_members::register_lints(sess, lint_store);
    rules::bon::bon_incoherent_builder_vocabulary::register_lints(sess, lint_store);
    rules::bon::bon_inconsistent_builder_conversions::register_lints(sess, lint_store);
    rules::bon::bon_manual_builder_implementations::register_lints(sess, lint_store);
    rules::bon::bon_needless_builders_for_small_apis::register_lints(sess, lint_store);
    rules::bon::bon_parameter_heavy_apis_without_builders::register_lints(sess, lint_store);
    rules::bon::bon_public_builder_implementation_types::register_lints(sess, lint_store);
    rules::bon::bon_redundant_positional_and_builder_apis::register_lints(sess, lint_store);
    rules::bon::bon_required_builder_members_breaking_compatibility::register_lints(
        sess, lint_store,
    );
    rules::bon::bon_skipped_builder_members_without_policy::register_lints(sess, lint_store);
    rules::bon::bon_undocumented_builder_members::register_lints(sess, lint_store);
}

/// Registers the `derive_more`-aware lint layer.
#[cfg(feature = "derive_more")]
fn register_derive_more_lints(
    sess: &rustc_session::Session,
    lint_store: &mut rustc_lint::LintStore,
) {
    rules::derive_more::derive_more_ambiguous_derived_error_sources::register_lints(
        sess, lint_store,
    );
    rules::derive_more::derive_more_derived_constructors_bypassing_invariants::register_lints(
        sess, lint_store,
    );
    rules::derive_more::derive_more_derived_conversions_bypassing_invariants::register_lints(
        sess, lint_store,
    );
    rules::derive_more::derive_more_inconsistent_derived_equality::register_lints(sess, lint_store);
    rules::derive_more::derive_more_manual_aggregation_impls::register_lints(sess, lint_store);
    rules::derive_more::derive_more_manual_conversion_impls::register_lints(sess, lint_store);
    rules::derive_more::derive_more_manual_constructors::register_lints(sess, lint_store);
    rules::derive_more::derive_more_manual_equality_impls::register_lints(sess, lint_store);
    rules::derive_more::derive_more_manual_error_impls::register_lints(sess, lint_store);
    rules::derive_more::derive_more_manual_forwarding_interfaces::register_lints(sess, lint_store);
    rules::derive_more::derive_more_manual_formatting_impls::register_lints(sess, lint_store);
    rules::derive_more::derive_more_manual_from_str_impls::register_lints(sess, lint_store);
    rules::derive_more::derive_more_manual_into_iterator_impls::register_lints(sess, lint_store);
    rules::derive_more::derive_more_manual_operator_impls::register_lints(sess, lint_store);
    rules::derive_more::derive_more_manual_variant_accessors::register_lints(sess, lint_store);
    rules::derive_more::derive_more_mutable_forwarding_bypassing_invariants::register_lints(
        sess, lint_store,
    );
    rules::derive_more::derive_more_non_roundtripping_derived_text_contracts::register_lints(
        sess, lint_store,
    );
    rules::derive_more::derive_more_opaque_derived_display_contracts::register_lints(
        sess, lint_store,
    );
    rules::derive_more::derive_more_operator_derives_bypassing_invariants::register_lints(
        sess, lint_store,
    );
    rules::derive_more::derive_more_panic_prone_derived_variant_accessors::register_lints(
        sess, lint_store,
    );
}

/// Registers cross-framework resolution policies.
#[cfg(feature = "framework")]
fn register_framework_lints(sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    rules::framework::framework_resolution_required::register_lints(sess, lint_store);
}

/// Registers the Miette-aware lint layer.
#[cfg(feature = "miette")]
fn register_miette_lints(sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    rules::miette::miette_ad_hoc_diagnostics_at_domain_boundaries::register_lints(sess, lint_store);
    rules::miette::miette_duplicate_diagnostic_codes::register_lints(sess, lint_store);
    rules::miette::miette_generic_diagnostic_help::register_lints(sess, lint_store);
    rules::miette::miette_incoherent_diagnostic_severity::register_lints(sess, lint_store);
    rules::miette::miette_labels_without_source_code::register_lints(sess, lint_store);
    rules::miette::miette_malformed_diagnostic_codes::register_lints(sess, lint_store);
    rules::miette::miette_manual_diagnostic_impls::register_lints(sess, lint_store);
    rules::miette::miette_misclassified_related_diagnostics::register_lints(sess, lint_store);
    rules::miette::miette_missing_diagnostic_codes::register_lints(sess, lint_store);
    rules::miette::miette_plain_error_diagnostic_sources::register_lints(sess, lint_store);
    rules::miette::miette_reports_in_library_interfaces::register_lints(sess, lint_store);
    rules::miette::miette_sensitive_diagnostic_source::register_lints(sess, lint_store);
    rules::miette::miette_source_code_without_labels::register_lints(sess, lint_store);
    rules::miette::miette_unstable_diagnostic_urls::register_lints(sess, lint_store);
    rules::miette::miette_unfocused_diagnostic_labels::register_lints(sess, lint_store);
}

/// Registers the Serde-aware lint layer.
#[cfg(feature = "serde")]
fn register_serde_lints(sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    rules::serde::serde_ambiguous_untagged_enums::register_lints(sess, lint_store);
    rules::serde::serde_asymmetric_serde_contracts::register_lints(sess, lint_store);
    rules::serde::serde_catch_all_variants_hiding_schema_drift::register_lints(sess, lint_store);
    rules::serde::serde_deserialization_bypassing_invariants::register_lints(sess, lint_store);
    rules::serde::serde_defaults_hiding_missing_data::register_lints(sess, lint_store);
    rules::serde::serde_duplicate_serialized_names::register_lints(sess, lint_store);
    rules::serde::serde_flattened_field_collisions::register_lints(sess, lint_store);
    rules::serde::serde_flattened_unknown_field_policies::register_lints(sess, lint_store);
    rules::serde::serde_format_specific_serde_impls::register_lints(sess, lint_store);
    rules::serde::serde_lossy_conditional_serialization::register_lints(sess, lint_store);
    rules::serde::serde_non_roundtripping_serde_adapters::register_lints(sess, lint_store);
    rules::serde::serde_remote_representations_drifting_from_sources::register_lints(
        sess, lint_store,
    );
    rules::serde::serde_sensitive_fields_serialized_by_default::register_lints(sess, lint_store);
    rules::serde::serde_unstable_implicit_wire_names::register_lints(sess, lint_store);
    rules::serde::serde_manual_deserialize_impls::register_lints(sess, lint_store);
    rules::serde::serde_manual_serialize_impls::register_lints(sess, lint_store);
}

/// Registers the thiserror-aware lint layer.
#[cfg(feature = "thiserror")]
fn register_thiserror_lints(sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    rules::thiserror::thiserror_ambiguous_error_sources::register_lints(sess, lint_store);
    rules::thiserror::thiserror_duplicate_error_messages::register_lints(sess, lint_store);
    rules::thiserror::thiserror_dynamic_errors_in_library_interfaces::register_lints(
        sess, lint_store,
    );
    rules::thiserror::thiserror_error_messages_used_as_identifiers::register_lints(
        sess, lint_store,
    );
    rules::thiserror::thiserror_from_sources_without_context::register_lints(sess, lint_store);
    rules::thiserror::thiserror_manual_error_impls::register_lints(sess, lint_store);
    rules::thiserror::thiserror_manual_from_error_variants::register_lints(sess, lint_store);
    rules::thiserror::thiserror_non_send_sync_public_errors::register_lints(sess, lint_store);
    rules::thiserror::thiserror_opaque_errors_exposing_representations::register_lints(
        sess, lint_store,
    );
    rules::thiserror::thiserror_unpropagated_error_backtraces::register_lints(sess, lint_store);
    rules::thiserror::thiserror_unreported_error_sources::register_lints(sess, lint_store);
}

/// Registers each framework-independent lint pass.
fn register_core_rules(sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    rules::core::ad_hoc_collection_construction::register_lints(sess, lint_store);
    rules::core::ad_hoc_conversions::register_lints(sess, lint_store);
    rules::core::ad_hoc_equality::register_lints(sess, lint_store);
    rules::core::ad_hoc_error_interfaces::register_lints(sess, lint_store);
    rules::core::ad_hoc_formatting::register_lints(sess, lint_store);
    rules::core::ad_hoc_iterators::register_lints(sess, lint_store);
    rules::core::ad_hoc_ordering::register_lints(sess, lint_store);
    rules::core::ad_hoc_string_parsers::register_lints(sess, lint_store);
    rules::core::ambiguous_primitive_parameters::register_lints(sess, lint_store);
    rules::core::bare_tuple_types::register_lints(sess, lint_store);
    rules::core::bidirectional_module_dependencies::register_lints(sess, lint_store);
    rules::core::bool_fields_without_predicate_prefix::register_lints(sess, lint_store);
    rules::core::boolean_function_arguments::register_lints(sess, lint_store);
    rules::core::collection_method_like_free_functions::register_lints(sess, lint_store);
    rules::core::constructor_like_free_functions::register_lints(sess, lint_store);
    rules::core::cross_file_struct_impls::register_lints(sess, lint_store);
    rules::core::deeply_nested_control_flow::register_lints(sess, lint_store);
    rules::core::discarded_results::register_lints(sess, lint_store);
    rules::core::duplicate_section_divider_prefixes::register_lints(sess, lint_store);
    rules::core::fallible_values_replaced_with_defaults::register_lints(sess, lint_store);
    rules::core::foreign_type_method_like_free_functions::register_lints(sess, lint_store);
    rules::core::incoherent_extension_traits::register_lints(sess, lint_store);
    rules::core::incoherent_type_family_names::register_lints(sess, lint_store);
    rules::core::implicit_first_wins_deduplication::register_lints(sess, lint_store);
    rules::core::invalid_barrel_file_items::register_lints(sess, lint_store);
    rules::core::long_method_chains::register_lints(sess, lint_store);
    rules::core::malformed_code_phase_comments::register_lints(sess, lint_store);
    rules::core::malformed_section_dividers::register_lints(sess, lint_store);
    rules::core::method_like_free_functions::register_lints(sess, lint_store);
    rules::core::misordered_inherent_impl_items::register_lints(sess, lint_store);
    rules::core::misordered_module_declarations::register_lints(sess, lint_store);
    rules::core::misordered_type_declarations::register_lints(sess, lint_store);
    rules::core::mismatched_section_divider_prefixes::register_lints(sess, lint_store);
    rules::core::missing_code_phase_comments::register_lints(sess, lint_store);
    rules::core::missing_section_dividers::register_lints(sess, lint_store);
    rules::core::mixed_module_file_layouts::register_lints(sess, lint_store);
    rules::core::needless_delegating_types::register_lints(sess, lint_store);
    rules::core::needless_function_wrappers::register_lints(sess, lint_store);
    rules::core::needlessly_nested_control_flow::register_lints(sess, lint_store);
    rules::core::nested_tuple_types::register_lints(sess, lint_store);
    rules::core::non_adjacent_extension_trait_impls::register_lints(sess, lint_store);
    rules::core::non_adjacent_struct_impls::register_lints(sess, lint_store);
    rules::core::non_defining_module_reexports::register_lints(sess, lint_store);
    rules::core::noncanonical_restricted_visibility::register_lints(sess, lint_store);
    rules::core::overloaded_declaration_sections::register_lints(sess, lint_store);
    rules::core::oversized_match_arms::register_lints(sess, lint_store);
    rules::core::positional_aggregate_fields::register_lints(sess, lint_store);
    rules::core::repeated_identical_statements::register_lints(sess, lint_store);
    rules::core::results_converted_to_options::register_lints(sess, lint_store);
    rules::core::revalidated_string_parameters::register_lints(sess, lint_store);
    rules::core::single_implementation_traits::register_lints(sess, lint_store);
    rules::core::stringly_typed_domain_function_families::register_lints(sess, lint_store);
    rules::core::unconsumed_generic_abstractions::register_lints(sess, lint_store);
    rules::core::undocumented_items::register_lints(sess, lint_store);
    rules::core::unencapsulated_binary_enum_classification::register_lints(sess, lint_store);
    rules::core::unnamed_policy_literals::register_lints(sess, lint_store);
    rules::core::unnecessarily_broad_visibility::register_lints(sess, lint_store);
    rules::core::unseparated_associated_items::register_lints(sess, lint_store);
    rules::core::unparenthesized_mixed_boolean_operators::register_lints(sess, lint_store);
    rules::core::visibility_required_only_by_tests::register_lints(sess, lint_store);
}

/// Registers the framework-independent lint layer and its group.
fn register_core_lints(sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    let core_lint_start = lint_store.get_lints().len();
    register_core_rules(sess, lint_store);
    let core_lints = lint_store.get_lints()[core_lint_start..]
        .iter()
        .map(|lint| LintId::of(lint))
        .collect();
    lint_store.register_group(true, "rlib_core", None, core_lints);
}

/// Registers the Leptos-aware lint layer.
#[cfg(feature = "leptos")]
fn register_leptos_lints(sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    rules::leptos::leptos_attribute_bound_controlled_inputs::register_lints(sess, lint_store);
    rules::leptos::leptos_boolean_component_props::register_lints(sess, lint_store);
    rules::leptos::leptos_duplicate_view_section_comments::register_lints(sess, lint_store);
    rules::leptos::leptos_effects_synchronizing_signals::register_lints(sess, lint_store);
    rules::leptos::leptos_hydration_divergent_views::register_lints(sess, lint_store);
    rules::leptos::leptos_implicit_default_component_props::register_lints(sess, lint_store);
    rules::leptos::leptos_manual_resource_refetch_signals::register_lints(sess, lint_store);
    rules::leptos::leptos_malformed_view_section_comments::register_lints(sess, lint_store);
    rules::leptos::leptos_markup_repeating_view_comments::register_lints(sess, lint_store);
    rules::leptos::leptos_mismatched_view_attribute_groups::register_lints(sess, lint_store);
    rules::leptos::leptos_missing_view_section_comments::register_lints(sess, lint_store);
    rules::leptos::leptos_missing_view_attribute_group_comments::register_lints(sess, lint_store);
    rules::leptos::leptos_needlessly_cloned_signal_values::register_lints(sess, lint_store);
    rules::leptos::leptos_oversized_view_sections::register_lints(sess, lint_store);
    rules::leptos::leptos_oversized_view_attribute_groups::register_lints(sess, lint_store);
    rules::leptos::leptos_primitive_context_values::register_lints(sess, lint_store);
    rules::leptos::leptos_read_then_replace_signals::register_lints(sess, lint_store);
    rules::leptos::leptos_reactive_writes_during_view_construction::register_lints(
        sess, lint_store,
    );
    rules::leptos::leptos_reactive_writes_in_resource_fetchers::register_lints(sess, lint_store);
    rules::leptos::leptos_resource_fetchers_rereading_sources::register_lints(sess, lint_store);
    rules::leptos::leptos_server_functions_without_authorization_boundaries::register_lints(
        sess, lint_store,
    );
    rules::leptos::leptos_unsanitized_inner_html::register_lints(sess, lint_store);
    rules::leptos::leptos_unkeyed_reactive_collections::register_lints(sess, lint_store);
    rules::leptos::leptos_unreactive_signal_reads_in_views::register_lints(sess, lint_store);
    rules::leptos::leptos_unstable_for_keys::register_lints(sess, lint_store);
    rules::leptos::leptos_writable_signal_component_props::register_lints(sess, lint_store);
}

/// Registers every enabled lint layer in this library.
fn register_all_lints(sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    #[cfg(feature = "bon")]
    register_bon_lints(sess, lint_store);
    #[cfg(feature = "derive_more")]
    register_derive_more_lints(sess, lint_store);
    #[cfg(feature = "framework")]
    register_framework_lints(sess, lint_store);
    #[cfg(feature = "miette")]
    register_miette_lints(sess, lint_store);
    #[cfg(feature = "serde")]
    register_serde_lints(sess, lint_store);
    #[cfg(feature = "strum")]
    register_strum_lints(sess, lint_store);
    #[cfg(feature = "thiserror")]
    register_thiserror_lints(sess, lint_store);
    register_core_lints(sess, lint_store);
    #[cfg(feature = "leptos")]
    register_leptos_lints(sess, lint_store);
}

/// Registers every project lint with the compiler process that loaded this library.
#[expect(
    unsafe_code,
    reason = "Dylint locates the registration entry point by its unmangled symbol name"
)]
#[unsafe(no_mangle)]
pub extern "Rust" fn register_lints(
    sess: &rustc_session::Session,
    lint_store: &mut rustc_lint::LintStore,
) {
    register_all_lints(sess, lint_store);
}

/// Registers every enabled Strum-aware lint pass.
#[cfg(feature = "strum")]
fn register_strum_lints(sess: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    rules::strum::strum_conflicting_enum_serializations::register_lints(sess, lint_store);
    rules::strum::strum_declaration_order_domain_contracts::register_lints(sess, lint_store);
    rules::strum::strum_defaulted_payload_enum_construction::register_lints(sess, lint_store);
    rules::strum::strum_divergent_discriminant_contracts::register_lints(sess, lint_store);
    rules::strum::strum_divergent_variant_name_contracts::register_lints(sess, lint_store);
    rules::strum::strum_documentation_used_as_enum_messages::register_lints(sess, lint_store);
    rules::strum::strum_filtered_enum_count_contracts::register_lints(sess, lint_store);
    rules::strum::strum_manual_enum_accessors::register_lints(sess, lint_store);
    rules::strum::strum_manual_discriminant_enums::register_lints(sess, lint_store);
    rules::strum::strum_manual_enum_counts::register_lints(sess, lint_store);
    rules::strum::strum_manual_enum_iteration::register_lints(sess, lint_store);
    rules::strum::strum_manual_enum_metadata::register_lints(sess, lint_store);
    rules::strum::strum_manual_enum_predicates::register_lints(sess, lint_store);
    rules::strum::strum_manual_enum_string_conversions::register_lints(sess, lint_store);
    rules::strum::strum_manual_enum_string_parsers::register_lints(sess, lint_store);
    rules::strum::strum_manual_repr_conversions::register_lints(sess, lint_store);
    rules::strum::strum_manual_variant_arrays::register_lints(sess, lint_store);
    rules::strum::strum_manual_variant_names::register_lints(sess, lint_store);
    rules::strum::strum_non_roundtripping_enum_strings::register_lints(sess, lint_store);
}
