extern crate rustc_lint;
extern crate rustc_session;

use rustc_session::lint::LintId;

use crate::rules;

// Registers this library as a dylint plugin with the compiler process that loaded it.
dylint_linting::dylint_library!();

// -----------------------------------------------------------------------------
// DefineLintRegistration: Feature-aware registration generator
// -----------------------------------------------------------------------------

macro_rules! define_lint_registration {
    (
        core { $($core_lint:ident),+ $(,)? }
        $(
            #[cfg(feature = $feature:literal)]
            $layer:ident { $($lint:ident),+ $(,)? }
        )*
    ) => {
        /// Registers every enabled project lint with the compiler process that loaded this library.
        #[expect(
            unsafe_code,
            reason = "Dylint locates the registration entry point by its unmangled symbol name"
        )]
        #[unsafe(no_mangle)]
        pub extern "Rust" fn register_lints(
            sess: &rustc_session::Session,
            lint_store: &mut rustc_lint::LintStore,
        ) {
            let core_lint_start = lint_store.get_lints().len();
            $(rules::core::$core_lint::register_lints(sess, lint_store);)+
            let core_lints = lint_store.get_lints()[core_lint_start..]
                .iter()
                .map(|lint| LintId::of(lint))
                .collect();
            lint_store.register_group(true, "rlib_core", None, core_lints);

            $(
                #[cfg(feature = $feature)]
                {
                    $(rules::$layer::$lint::register_lints(sess, lint_store);)+
                }
            )*
        }
    };
}

// -----------------------------------------------------------------------------
// LintInventory: Complete registration catalog
// -----------------------------------------------------------------------------

define_lint_registration! {
    core {
        ad_hoc_collection_construction,
        ad_hoc_conversions,
        ad_hoc_equality,
        ad_hoc_error_interfaces,
        ad_hoc_formatting,
        ad_hoc_iterators,
        ad_hoc_ordering,
        ad_hoc_string_parsers,
        ambiguous_primitive_parameters,
        bare_tuple_types,
        bidirectional_module_dependencies,
        bool_fields_without_predicate_prefix,
        boolean_function_arguments,
        collection_method_like_free_functions,
        constructor_like_free_functions,
        cross_file_struct_impls,
        deeply_nested_control_flow,
        discarded_results,
        duplicate_section_divider_prefixes,
        fallible_values_replaced_with_defaults,
        foreign_type_method_like_free_functions,
        incoherent_extension_traits,
        incoherent_type_family_names,
        implicit_first_wins_deduplication,
        invalid_barrel_file_items,
        long_method_chains,
        malformed_code_phase_comments,
        malformed_section_dividers,
        method_like_free_functions,
        misordered_inherent_impl_items,
        misordered_module_declarations,
        misordered_type_declarations,
        mismatched_section_divider_prefixes,
        missing_code_phase_comments,
        missing_section_dividers,
        mixed_module_file_layouts,
        needless_delegating_types,
        needless_function_wrappers,
        needlessly_nested_control_flow,
        nested_tuple_types,
        non_adjacent_extension_trait_impls,
        non_adjacent_struct_impls,
        non_defining_module_reexports,
        noncanonical_restricted_visibility,
        overloaded_declaration_sections,
        oversized_match_arms,
        positional_aggregate_fields,
        repeated_identical_statements,
        results_converted_to_options,
        revalidated_string_parameters,
        single_implementation_traits,
        stringly_typed_domain_function_families,
        unconsumed_generic_abstractions,
        undocumented_items,
        unencapsulated_binary_enum_classification,
        unnamed_policy_literals,
        unnecessarily_broad_visibility,
        unseparated_associated_items,
        unparenthesized_mixed_boolean_operators,
        visibility_required_only_by_tests,
    }
    #[cfg(feature = "bon")]
    bon {
        bon_builders_bypassing_construction_invariants,
        bon_escaping_incomplete_builders,
        bon_implicit_optional_builder_members,
        bon_incoherent_conditional_builder_members,
        bon_incoherent_builder_vocabulary,
        bon_inconsistent_builder_conversions,
        bon_manual_builder_implementations,
        bon_needless_builders_for_small_apis,
        bon_parameter_heavy_apis_without_builders,
        bon_public_builder_implementation_types,
        bon_redundant_positional_and_builder_apis,
        bon_required_builder_members_breaking_compatibility,
        bon_skipped_builder_members_without_policy,
        bon_undocumented_builder_members,
    }
    #[cfg(feature = "derive_more")]
    derive_more {
        derive_more_ambiguous_derived_error_sources,
        derive_more_derived_constructors_bypassing_invariants,
        derive_more_derived_conversions_bypassing_invariants,
        derive_more_inconsistent_derived_equality,
        derive_more_manual_aggregation_impls,
        derive_more_manual_conversion_impls,
        derive_more_manual_constructors,
        derive_more_manual_equality_impls,
        derive_more_manual_error_impls,
        derive_more_manual_forwarding_interfaces,
        derive_more_manual_formatting_impls,
        derive_more_manual_from_str_impls,
        derive_more_manual_into_iterator_impls,
        derive_more_manual_operator_impls,
        derive_more_manual_variant_accessors,
        derive_more_mutable_forwarding_bypassing_invariants,
        derive_more_non_roundtripping_derived_text_contracts,
        derive_more_opaque_derived_display_contracts,
        derive_more_operator_derives_bypassing_invariants,
        derive_more_panic_prone_derived_variant_accessors,
    }
    #[cfg(feature = "framework")]
    framework {
        framework_resolution_required,
    }
    #[cfg(feature = "leptos")]
    leptos {
        leptos_attribute_bound_controlled_inputs,
        leptos_boolean_component_props,
        leptos_duplicate_view_section_comments,
        leptos_effects_synchronizing_signals,
        leptos_hydration_divergent_views,
        leptos_implicit_default_component_props,
        leptos_manual_resource_refetch_signals,
        leptos_malformed_view_section_comments,
        leptos_markup_repeating_view_comments,
        leptos_mismatched_view_attribute_groups,
        leptos_missing_view_section_comments,
        leptos_missing_view_attribute_group_comments,
        leptos_needlessly_cloned_signal_values,
        leptos_oversized_view_sections,
        leptos_oversized_view_attribute_groups,
        leptos_primitive_context_values,
        leptos_read_then_replace_signals,
        leptos_reactive_writes_during_view_construction,
        leptos_reactive_writes_in_resource_fetchers,
        leptos_resource_fetchers_rereading_sources,
        leptos_server_functions_without_authorization_boundaries,
        leptos_unsanitized_inner_html,
        leptos_unkeyed_reactive_collections,
        leptos_unreactive_signal_reads_in_views,
        leptos_unstable_for_keys,
        leptos_writable_signal_component_props,
    }
    #[cfg(feature = "miette")]
    miette {
        miette_ad_hoc_diagnostics_at_domain_boundaries,
        miette_duplicate_diagnostic_codes,
        miette_generic_diagnostic_help,
        miette_incoherent_diagnostic_severity,
        miette_labels_without_source_code,
        miette_malformed_diagnostic_codes,
        miette_manual_diagnostic_impls,
        miette_misclassified_related_diagnostics,
        miette_missing_diagnostic_codes,
        miette_plain_error_diagnostic_sources,
        miette_reports_in_library_interfaces,
        miette_sensitive_diagnostic_source,
        miette_source_code_without_labels,
        miette_unfocused_diagnostic_labels,
        miette_unstable_diagnostic_urls,
    }
    #[cfg(feature = "serde")]
    serde {
        serde_ambiguous_untagged_enums,
        serde_asymmetric_serde_contracts,
        serde_catch_all_variants_hiding_schema_drift,
        serde_defaults_hiding_missing_data,
        serde_deserialization_bypassing_invariants,
        serde_duplicate_serialized_names,
        serde_flattened_field_collisions,
        serde_flattened_unknown_field_policies,
        serde_format_specific_serde_impls,
        serde_lossy_conditional_serialization,
        serde_manual_deserialize_impls,
        serde_manual_serialize_impls,
        serde_non_roundtripping_serde_adapters,
        serde_remote_representations_drifting_from_sources,
        serde_sensitive_fields_serialized_by_default,
        serde_unstable_implicit_wire_names,
    }
    #[cfg(feature = "strum")]
    strum {
        strum_conflicting_enum_serializations,
        strum_declaration_order_domain_contracts,
        strum_defaulted_payload_enum_construction,
        strum_divergent_discriminant_contracts,
        strum_divergent_variant_name_contracts,
        strum_documentation_used_as_enum_messages,
        strum_filtered_enum_count_contracts,
        strum_manual_discriminant_enums,
        strum_manual_enum_accessors,
        strum_manual_enum_counts,
        strum_manual_enum_iteration,
        strum_manual_enum_metadata,
        strum_manual_enum_predicates,
        strum_manual_enum_string_conversions,
        strum_manual_enum_string_parsers,
        strum_manual_repr_conversions,
        strum_manual_variant_arrays,
        strum_manual_variant_names,
        strum_non_roundtripping_enum_strings,
    }
    #[cfg(feature = "thiserror")]
    thiserror {
        thiserror_ambiguous_error_sources,
        thiserror_duplicate_error_messages,
        thiserror_dynamic_errors_in_library_interfaces,
        thiserror_error_messages_used_as_identifiers,
        thiserror_from_sources_without_context,
        thiserror_manual_error_impls,
        thiserror_manual_from_error_variants,
        thiserror_non_send_sync_public_errors,
        thiserror_opaque_errors_exposing_representations,
        thiserror_unpropagated_error_backtraces,
        thiserror_unreported_error_sources,
    }
}
