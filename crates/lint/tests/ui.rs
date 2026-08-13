//! UI regression tests for every default-configuration lint fixture.

use dylint_testing::ui::Test;

const CROSS_CUTTING_LINT_ALLOWS: [&str; 98] = [
    "-A",
    "ad_hoc_collection_construction",
    "-A",
    "ad_hoc_conversions",
    "-A",
    "ad_hoc_equality",
    "-A",
    "ad_hoc_error_interfaces",
    "-A",
    "ad_hoc_formatting",
    "-A",
    "ad_hoc_iterators",
    "-A",
    "ad_hoc_ordering",
    "-A",
    "ad_hoc_string_parsers",
    "-A",
    "ambiguous_primitive_parameters",
    "-A",
    "bare_tuple_types",
    "-A",
    "bidirectional_module_dependencies",
    "-A",
    "boolean_function_arguments",
    "-A",
    "constructor_like_free_functions",
    "-A",
    "deeply_nested_control_flow",
    "-A",
    "discarded_results",
    "-A",
    "duplicate_section_divider_prefixes",
    "-A",
    "fallible_values_replaced_with_defaults",
    "-A",
    "foreign_type_method_like_free_functions",
    "-A",
    "incoherent_extension_traits",
    "-A",
    "incoherent_type_family_names",
    "-A",
    "implicit_first_wins_deduplication",
    "-A",
    "long_method_chains",
    "-A",
    "malformed_code_phase_comments",
    "-A",
    "malformed_section_dividers",
    "-A",
    "mismatched_section_divider_prefixes",
    "-A",
    "missing_code_phase_comments",
    "-A",
    "missing_section_dividers",
    "-A",
    "needless_delegating_types",
    "-A",
    "needlessly_nested_control_flow",
    "-A",
    "nested_tuple_types",
    "-A",
    "non_defining_module_reexports",
    "-A",
    "noncanonical_restricted_visibility",
    "-A",
    "non_adjacent_extension_trait_impls",
    "-A",
    "overloaded_declaration_sections",
    "-A",
    "oversized_match_arms",
    "-A",
    "positional_aggregate_fields",
    "-A",
    "repeated_identical_statements",
    "-A",
    "results_converted_to_options",
    "-A",
    "revalidated_string_parameters",
    "-A",
    "single_implementation_traits",
    "-A",
    "stringly_typed_domain_function_families",
    "-A",
    "unconsumed_generic_abstractions",
    "-A",
    "undocumented_items",
    "-A",
    "unencapsulated_binary_enum_classification",
    "-A",
    "unnamed_policy_literals",
    "-A",
    "unnecessarily_broad_visibility",
    "-A",
    "unseparated_associated_items",
    "-A",
    "unparenthesized_mixed_boolean_operators",
    "-A",
    "visibility_required_only_by_tests",
];

// Leptos macro expansion intentionally produces shapes covered by these core lints. Keeping the
// exceptions here lets each core lint's own standalone fixture continue to exercise the warning.
#[cfg(feature = "leptos")]
const LEPTOS_FIXTURE_LINT_ALLOWS: [&str; 20] = [
    "-A",
    "bool_fields_without_predicate_prefix",
    "-A",
    "collection_method_like_free_functions",
    "-A",
    "cross_file_struct_impls",
    "-A",
    "invalid_barrel_file_items",
    "-A",
    "method_like_free_functions",
    "-A",
    "misordered_inherent_impl_items",
    "-A",
    "misordered_module_declarations",
    "-A",
    "misordered_type_declarations",
    "-A",
    "needless_function_wrappers",
    "-A",
    "non_adjacent_struct_impls",
];

/// Runs every standalone and dependency-aware UI fixture against the lint library.
#[test]
fn ui() {
    #[cfg(any(feature = "strum", feature = "bon"))]
    if rerun_with_feature_aware_cargo_wrapper() {
        return;
    }
    #[cfg(any(feature = "strum", feature = "leptos", feature = "bon"))]
    if selected_framework_fixture().is_none() {
        run_standalone_fixtures();
    }
    #[cfg(not(any(feature = "strum", feature = "leptos", feature = "bon")))]
    run_standalone_fixtures();
    #[cfg(feature = "strum")]
    run_strum_fixtures();
    #[cfg(feature = "leptos")]
    run_leptos_fixtures();
    #[cfg(feature = "bon")]
    run_bon_fixtures();
}

/// Makes Dylint's internal `cargo build` preserve the test process's feature set.
#[cfg(all(any(feature = "strum", feature = "bon"), unix))]
fn rerun_with_feature_aware_cargo_wrapper() -> bool {
    use std::env::{current_exe, join_paths, split_paths, temp_dir, var, var_os};
    use std::fs::{create_dir_all, set_permissions, write};
    use std::iter::once;
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, id};

    if var_os("RLIB_LINT_ALL_FEATURE_UI_CHILD").is_some() {
        return false;
    }

    let directory = temp_dir().join(format!("rlib-lint-cargo-wrapper-{}", id()));
    create_dir_all(&directory).expect("Cargo wrapper directory should be creatable");
    let wrapper = directory.join("cargo");
    let feature_flags = if cfg!(feature = "leptos") {
        "--all-features"
    } else if cfg!(all(feature = "strum", feature = "bon")) {
        "--no-default-features --features strum,bon"
    } else if cfg!(feature = "bon") {
        "--no-default-features --features bon"
    } else {
        "--no-default-features --features strum"
    };
    write(
        &wrapper,
        format!(
            "#!/bin/sh\nif [ \"$1\" = build ]; then\n  exec \"$RLIB_LINT_REAL_CARGO\" \"$@\" {feature_flags}\nfi\nexec \"$RLIB_LINT_REAL_CARGO\" \"$@\"\n"
        ),
    )
    .expect("Cargo wrapper should be writable");
    let mut permissions = wrapper
        .metadata()
        .expect("Cargo wrapper metadata should be readable")
        .permissions();
    permissions.set_mode(0o755);
    set_permissions(&wrapper, permissions).expect("Cargo wrapper should be executable");

    let cargo = var("CARGO").expect("Cargo should expose its own executable path");
    let path = join_paths(once(directory).chain(split_paths(
        &var_os("PATH").expect("test process should have PATH"),
    )))
    .expect("Cargo wrapper PATH should be valid");
    let status = Command::new(current_exe().expect("UI test executable should be available"))
        .args(["ui", "--exact", "--nocapture"])
        .env("RLIB_LINT_ALL_FEATURE_UI_CHILD", "1")
        .env("RLIB_LINT_REAL_CARGO", cargo)
        .env("PATH", path)
        .status()
        .expect("wrapped all-feature UI child should start");
    assert!(status.success(), "wrapped all-feature UI child failed");
    true
}

/// Rejects unsupported all-feature UI execution platforms explicitly.
#[cfg(all(any(feature = "strum", feature = "bon"), not(unix)))]
fn rerun_with_feature_aware_cargo_wrapper() -> bool {
    panic!("all-feature Dylint UI tests currently require a Unix Cargo wrapper");
}

/// Runs fixtures that rustc can compile directly without Cargo dependency metadata.
fn run_standalone_fixtures() {
    Test::src_base(env!("CARGO_PKG_NAME"), "ui/core")
        .rustc_flags(CROSS_CUTTING_LINT_ALLOWS)
        .run();
}

#[cfg(feature = "bon")]
fn run_bon_fixtures() {
    let selected = selected_framework_fixture();
    for example in [
        "bon_implicit_optional_builder_members",
        "bon_inconsistent_builder_conversions",
        "bon_needless_builders_for_small_apis",
        "bon_parameter_heavy_apis_without_builders",
    ] {
        if selected
            .as_deref()
            .is_some_and(|selected| selected != example)
        {
            continue;
        }
        Test::example(env!("CARGO_PKG_NAME"), example)
            .rustc_flags(CROSS_CUTTING_LINT_ALLOWS)
            .run();
    }
}

/// Runs all Cargo examples that need dependency linking or macro expansion.
#[cfg(feature = "leptos")]
fn run_leptos_fixtures() {
    let selected = selected_framework_fixture();
    for example in [
        "leptos_markup_repeating_view_comments",
        "leptos_mismatched_view_attribute_groups",
        "leptos_duplicate_view_section_comments",
        "leptos_oversized_view_sections",
        "leptos_oversized_view_attribute_groups",
        "leptos_primitive_context_values",
        "leptos_malformed_view_section_comments",
        "leptos_attribute_bound_controlled_inputs",
        "leptos_boolean_component_props",
        "leptos_effects_synchronizing_signals",
        "leptos_hydration_divergent_views",
        "leptos_manual_resource_refetch_signals",
        "leptos_missing_view_section_comments",
        "leptos_missing_view_attribute_group_comments",
        "leptos_needlessly_cloned_signal_values",
        "leptos_read_then_replace_signals",
        "leptos_reactive_writes_during_view_construction",
        "leptos_reactive_writes_in_resource_fetchers",
        "leptos_resource_fetchers_rereading_sources",
        "leptos_unsanitized_inner_html",
        "leptos_unkeyed_reactive_collections",
        "leptos_unreactive_signal_reads_in_views",
        "leptos_unstable_for_keys",
        "leptos_writable_signal_component_props",
    ] {
        if selected
            .as_deref()
            .is_some_and(|selected| selected != example)
        {
            continue;
        }
        Test::example(env!("CARGO_PKG_NAME"), example)
            .rustc_flags(CROSS_CUTTING_LINT_ALLOWS)
            .rustc_flags(LEPTOS_FIXTURE_LINT_ALLOWS)
            .run();
    }
    if selected.as_deref().is_none_or(|selected| {
        selected == "leptos_server_functions_without_authorization_boundaries"
    }) {
        Test::example(
            env!("CARGO_PKG_NAME"),
            "leptos_server_functions_without_authorization_boundaries",
        )
        .rustc_flags(CROSS_CUTTING_LINT_ALLOWS)
        .rustc_flags(LEPTOS_FIXTURE_LINT_ALLOWS)
        .dylint_toml(
            r#"
                [rlib-lint.leptos_server_authorization]
                sensitive_call_terms = ["delete_account", "read_private_profile"]
                authorization_functions = ["authorize_account_admin"]
                protected_endpoint_attributes = ["protected_endpoint"]
                public_endpoint_attributes = ["public_endpoint"]
            "#,
        )
        .run();
    }
}

/// Runs each Strum fixture with the provider policy its expected diagnostic requires.
#[cfg(feature = "strum")]
fn run_strum_fixtures() {
    let selected = selected_framework_fixture();
    for example in [
        "framework_resolution_required",
        "strum_manual_enum_accessors",
        "strum_manual_enum_counts",
        "strum_manual_enum_iteration",
        "strum_manual_repr_conversions",
        "strum_conflicting_enum_serializations",
        "strum_declaration_order_domain_contracts",
        "strum_defaulted_payload_enum_construction",
        "strum_divergent_discriminant_contracts",
        "strum_divergent_variant_name_contracts",
        "strum_documentation_used_as_enum_messages",
        "strum_filtered_enum_count_contracts",
        "strum_manual_discriminant_enums",
        "strum_manual_enum_metadata",
        "strum_manual_variant_names",
        "strum_non_roundtripping_enum_strings",
    ] {
        if selected
            .as_deref()
            .is_some_and(|selected| selected != example)
        {
            continue;
        }
        Test::example(env!("CARGO_PKG_NAME"), example)
            .rustc_flags(CROSS_CUTTING_LINT_ALLOWS)
            .run();
    }
    if selected
        .as_deref()
        .is_none_or(|selected| selected == "strum_manual_variant_arrays")
    {
        Test::example(env!("CARGO_PKG_NAME"), "strum_manual_variant_arrays")
            .rustc_flags(CROSS_CUTTING_LINT_ALLOWS)
            .dylint_toml(
                r#"
                [rlib-lint.derive_resolution]
                enum_variant_collection = "strum_variant_array"
            "#,
            )
            .run();
    }
    if selected
        .as_deref()
        .is_none_or(|selected| selected == "strum_manual_enum_predicates")
    {
        Test::example(env!("CARGO_PKG_NAME"), "strum_manual_enum_predicates")
            .rustc_flags(CROSS_CUTTING_LINT_ALLOWS)
            .dylint_toml(
                r#"
                [rlib-lint.derive_resolution]
                enum_variant_predicates = "strum_enum_is"
            "#,
            )
            .run();
    }
    if selected
        .as_deref()
        .is_none_or(|selected| selected == "strum_manual_enum_string_conversions")
    {
        Test::example(
            env!("CARGO_PKG_NAME"),
            "strum_manual_enum_string_conversions",
        )
        .rustc_flags(CROSS_CUTTING_LINT_ALLOWS)
        .dylint_toml(
            r#"
                [rlib-lint.derive_resolution]
                enum_display = "strum_display"
            "#,
        )
        .run();
    }
    if selected
        .as_deref()
        .is_none_or(|selected| selected == "strum_manual_enum_string_parsers")
    {
        Test::example(env!("CARGO_PKG_NAME"), "strum_manual_enum_string_parsers")
            .rustc_flags(CROSS_CUTTING_LINT_ALLOWS)
            .dylint_toml(
                r#"
                    [rlib-lint.derive_resolution]
                    enum_string_parsing = "strum_enum_string"
                "#,
            )
            .run();
    }
}

#[cfg(any(feature = "strum", feature = "leptos", feature = "bon"))]
fn selected_framework_fixture() -> Option<String> {
    use std::env::var;

    var("RLIB_LINT_FRAMEWORK_FIXTURE")
        .or_else(|_| var("RLIB_LINT_STRUM_FIXTURE"))
        .ok()
}
