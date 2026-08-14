# UI lint adversarial audit

This is the durable progress ledger for the sequential audit of every registered lint. The authoritative inventory is `src/registration.rs`; the initial reconciliation found 182 registered lints across 9 families. All have matching UI directories. The two additional framework UI directories (`framework_error_implementation_resolution_required` and `framework_error_variant_conversion_resolution_required`) are cross-provider resolution fixtures exercised by the `thiserror` harness, not registered lints.

Status: `[ ]` pending, `[~]` in progress, `[x]` adversarial UI coverage reviewed/extended, implementation repaired where needed, focused UI snapshot verified.

Per-lint protocol:

- Read the README, implementation, shared analyzers, fixture, and snapshot.
- Add warning-producing and suppressing boundary cases, emphasizing aliases, generics, macros, control flow, and near misses where applicable.
- Run the focused fixture, inspect every diagnostic, and fix root causes instead of blessing wrong output.
- Record cross-cutting semantic regressions in `REGRESSIONS.md` when the pattern is reusable.

## core (60)

- [x] `ad_hoc_collection_construction`
- [x] `ad_hoc_conversions`
- [x] `ad_hoc_equality`
- [x] `ad_hoc_error_interfaces`
- [x] `ad_hoc_formatting`
- [x] `ad_hoc_iterators`
- [x] `ad_hoc_ordering`
- [x] `ad_hoc_string_parsers`
- [x] `ambiguous_primitive_parameters`
- [x] `bare_tuple_types`
- [x] `bidirectional_module_dependencies`
- [x] `bool_fields_without_predicate_prefix`
- [x] `boolean_function_arguments`
- [x] `collection_method_like_free_functions`
- [x] `constructor_like_free_functions`
- [x] `cross_file_struct_impls`
- [x] `deeply_nested_control_flow`
- [x] `discarded_results`
- [x] `duplicate_section_divider_prefixes`
- [x] `fallible_values_replaced_with_defaults`
- [x] `foreign_type_method_like_free_functions`
- [x] `implicit_first_wins_deduplication`
- [x] `incoherent_extension_traits`
- [x] `incoherent_type_family_names`
- [x] `invalid_barrel_file_items`
- [x] `long_method_chains`
- [x] `malformed_code_phase_comments`
- [x] `malformed_section_dividers`
- [x] `method_like_free_functions`
- [x] `mismatched_section_divider_prefixes`
- [x] `misordered_inherent_impl_items`
- [x] `misordered_module_declarations`
- [x] `misordered_type_declarations`
- [x] `missing_code_phase_comments`
- [x] `missing_section_dividers`
- [x] `mixed_module_file_layouts`
- [x] `needless_delegating_types`
- [x] `needless_function_wrappers`
- [x] `needlessly_nested_control_flow`
- [x] `nested_tuple_types`
- [x] `non_adjacent_extension_trait_impls`
- [x] `non_adjacent_struct_impls`
- [x] `non_defining_module_reexports`
- [x] `noncanonical_restricted_visibility`
- [x] `overloaded_declaration_sections`
- [x] `oversized_match_arms`
- [x] `positional_aggregate_fields`
- [x] `repeated_identical_statements`
- [x] `results_converted_to_options`
- [x] `revalidated_string_parameters`
- [x] `single_implementation_traits`
- [x] `stringly_typed_domain_function_families`
- [x] `unconsumed_generic_abstractions`
- [x] `undocumented_items`
- [x] `unencapsulated_binary_enum_classification`
- [x] `unnamed_policy_literals`
- [x] `unnecessarily_broad_visibility`
- [x] `unparenthesized_mixed_boolean_operators`
- [x] `unseparated_associated_items`
- [x] `visibility_required_only_by_tests`

## bon (14)

- [x] `bon_builders_bypassing_construction_invariants`
- [x] `bon_escaping_incomplete_builders`
- [x] `bon_implicit_optional_builder_members`
- [x] `bon_incoherent_builder_vocabulary`
- [x] `bon_incoherent_conditional_builder_members`
- [x] `bon_inconsistent_builder_conversions`
- [x] `bon_manual_builder_implementations`
- [x] `bon_needless_builders_for_small_apis`
- [x] `bon_parameter_heavy_apis_without_builders`
- [x] `bon_public_builder_implementation_types`
- [x] `bon_redundant_positional_and_builder_apis`
- [x] `bon_required_builder_members_breaking_compatibility`
- [x] `bon_skipped_builder_members_without_policy`
- [x] `bon_undocumented_builder_members`

## derive_more (20)

- [x] `derive_more_ambiguous_derived_error_sources`
- [x] `derive_more_derived_constructors_bypassing_invariants`
- [x] `derive_more_derived_conversions_bypassing_invariants`
- [x] `derive_more_inconsistent_derived_equality`
- [x] `derive_more_manual_aggregation_impls`
- [x] `derive_more_manual_constructors`
- [x] `derive_more_manual_conversion_impls`
- [x] `derive_more_manual_equality_impls`
- [x] `derive_more_manual_error_impls`
- [x] `derive_more_manual_formatting_impls`
- [x] `derive_more_manual_forwarding_interfaces`
- [x] `derive_more_manual_from_str_impls`
- [x] `derive_more_manual_into_iterator_impls`
- [x] `derive_more_manual_operator_impls`
- [x] `derive_more_manual_variant_accessors`
- [x] `derive_more_mutable_forwarding_bypassing_invariants`
- [x] `derive_more_non_roundtripping_derived_text_contracts`
- [x] `derive_more_opaque_derived_display_contracts`
- [x] `derive_more_operator_derives_bypassing_invariants`
- [x] `derive_more_panic_prone_derived_variant_accessors`

## framework (1)

- [x] `framework_resolution_required`

## leptos (26)

- [x] `leptos_attribute_bound_controlled_inputs`
- [x] `leptos_boolean_component_props`
- [x] `leptos_duplicate_view_section_comments`
- [x] `leptos_effects_synchronizing_signals`
- [x] `leptos_hydration_divergent_views`
- [x] `leptos_implicit_default_component_props`
- [x] `leptos_malformed_view_section_comments`
- [x] `leptos_manual_resource_refetch_signals`
- [x] `leptos_markup_repeating_view_comments`
- [x] `leptos_mismatched_view_attribute_groups`
- [x] `leptos_missing_view_attribute_group_comments`
- [x] `leptos_missing_view_section_comments`
- [x] `leptos_needlessly_cloned_signal_values`
- [x] `leptos_oversized_view_attribute_groups`
- [x] `leptos_oversized_view_sections`
- [x] `leptos_primitive_context_values`
- [x] `leptos_reactive_writes_during_view_construction`
- [x] `leptos_reactive_writes_in_resource_fetchers`
- [x] `leptos_read_then_replace_signals`
- [x] `leptos_resource_fetchers_rereading_sources`
- [x] `leptos_server_functions_without_authorization_boundaries`
- [x] `leptos_unkeyed_reactive_collections`
- [x] `leptos_unreactive_signal_reads_in_views`
- [x] `leptos_unsanitized_inner_html`
- [x] `leptos_unstable_for_keys`
- [x] `leptos_writable_signal_component_props`

## miette (15)

- [x] `miette_ad_hoc_diagnostics_at_domain_boundaries`
- [x] `miette_duplicate_diagnostic_codes`
- [x] `miette_generic_diagnostic_help`
- [x] `miette_incoherent_diagnostic_severity`
- [x] `miette_labels_without_source_code`
- [x] `miette_malformed_diagnostic_codes`
- [x] `miette_manual_diagnostic_impls`
- [x] `miette_misclassified_related_diagnostics`
- [x] `miette_missing_diagnostic_codes`
- [x] `miette_plain_error_diagnostic_sources`
- [x] `miette_reports_in_library_interfaces`
- [x] `miette_sensitive_diagnostic_source`
- [x] `miette_source_code_without_labels`
- [x] `miette_unfocused_diagnostic_labels`
- [x] `miette_unstable_diagnostic_urls`

## serde (16)

- [x] `serde_ambiguous_untagged_enums`
- [x] `serde_asymmetric_serde_contracts`
- [x] `serde_catch_all_variants_hiding_schema_drift`
- [x] `serde_defaults_hiding_missing_data`
- [x] `serde_deserialization_bypassing_invariants`
- [x] `serde_duplicate_serialized_names`
- [x] `serde_flattened_field_collisions`
- [x] `serde_flattened_unknown_field_policies`
- [x] `serde_format_specific_serde_impls`
- [x] `serde_lossy_conditional_serialization`
- [x] `serde_manual_deserialize_impls`
- [x] `serde_manual_serialize_impls`
- [x] `serde_non_roundtripping_serde_adapters`
- [x] `serde_remote_representations_drifting_from_sources`
- [x] `serde_sensitive_fields_serialized_by_default`
- [x] `serde_unstable_implicit_wire_names`

## strum (19)

- [x] `strum_conflicting_enum_serializations`
- [x] `strum_declaration_order_domain_contracts`
- [x] `strum_defaulted_payload_enum_construction`
- [x] `strum_divergent_discriminant_contracts`
- [x] `strum_divergent_variant_name_contracts`
- [x] `strum_documentation_used_as_enum_messages`
- [x] `strum_filtered_enum_count_contracts`
- [x] `strum_manual_discriminant_enums`
- [x] `strum_manual_enum_accessors`
- [x] `strum_manual_enum_counts`
- [x] `strum_manual_enum_iteration`
- [x] `strum_manual_enum_metadata`
- [x] `strum_manual_enum_predicates`
- [x] `strum_manual_enum_string_conversions`
- [x] `strum_manual_enum_string_parsers`
- [x] `strum_manual_repr_conversions`
- [x] `strum_manual_variant_arrays`
- [x] `strum_manual_variant_names`
- [x] `strum_non_roundtripping_enum_strings`

## thiserror (11)

- [x] `thiserror_ambiguous_error_sources`
- [x] `thiserror_duplicate_error_messages`
- [x] `thiserror_dynamic_errors_in_library_interfaces`
- [x] `thiserror_error_messages_used_as_identifiers`
- [x] `thiserror_from_sources_without_context`
- [x] `thiserror_manual_error_impls`
- [x] `thiserror_manual_from_error_variants`
- [x] `thiserror_non_send_sync_public_errors`
- [x] `thiserror_opaque_errors_exposing_representations`
- [x] `thiserror_unpropagated_error_backtraces`
- [x] `thiserror_unreported_error_sources`
