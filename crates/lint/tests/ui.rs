//! UI regression tests for every default-configuration lint fixture.

use std::env::{VarError, var};
#[cfg(all(
    any(
        feature = "strum",
        feature = "bon",
        feature = "derive_more",
        feature = "miette",
        feature = "serde",
        feature = "thiserror"
    ),
    unix
))]
use std::env::{current_exe, join_paths, split_paths, temp_dir, var_os};
#[cfg(all(
    any(
        feature = "strum",
        feature = "bon",
        feature = "derive_more",
        feature = "miette",
        feature = "serde",
        feature = "thiserror"
    ),
    unix
))]
use std::fs::{create_dir_all, set_permissions, write};
#[cfg(all(
    any(
        feature = "strum",
        feature = "bon",
        feature = "derive_more",
        feature = "miette",
        feature = "serde",
        feature = "thiserror"
    ),
    unix
))]
use std::iter::once;
#[cfg(all(
    any(
        feature = "strum",
        feature = "bon",
        feature = "derive_more",
        feature = "miette",
        feature = "serde",
        feature = "thiserror"
    ),
    unix
))]
use std::os::unix::fs::PermissionsExt;
#[cfg(all(
    any(
        feature = "strum",
        feature = "bon",
        feature = "derive_more",
        feature = "miette",
        feature = "serde",
        feature = "thiserror"
    ),
    unix
))]
use std::process::{Command, id};

use dylint_testing::ui::Test;

// -----------------------------------------------------------------------------
// Fixture: UI fixture isolation and execution
// -----------------------------------------------------------------------------

/// Cross-cutting lints excluded while each fixture isolates its own diagnostic contract.
const FIXTURE_CROSS_CUTTING_LINT_ALLOWS: [&str; 110] = [
    "-Zcrate-attr=feature(register_tool)",
    "-Zcrate-attr=register_tool(rlib)",
    "-A",
    "duplicate_features",
    "-A",
    "rlib::ad_hoc_collection_construction",
    "-A",
    "rlib::ad_hoc_conversions",
    "-A",
    "rlib::ad_hoc_equality",
    "-A",
    "rlib::ad_hoc_error_interfaces",
    "-A",
    "rlib::ad_hoc_formatting",
    "-A",
    "rlib::ad_hoc_iterators",
    "-A",
    "rlib::ad_hoc_ordering",
    "-A",
    "rlib::ad_hoc_string_parsers",
    "-A",
    "rlib::ambiguous_primitive_parameters",
    "-A",
    "rlib::bare_tuple_types",
    "-A",
    "rlib::bidirectional_module_dependencies",
    "-A",
    "rlib::boolean_function_arguments",
    "-A",
    "rlib::constructor_like_free_functions",
    "-A",
    "rlib::deeply_nested_control_flow",
    "-A",
    "rlib::discarded_results",
    "-A",
    "rlib::documentation_after_attributes",
    "-A",
    "rlib::duplicate_section_divider_prefixes",
    "-A",
    "rlib::fallible_values_replaced_with_defaults",
    "-A",
    "rlib::foreign_type_method_like_free_functions",
    "-A",
    "rlib::incoherent_extension_traits",
    "-A",
    "rlib::incoherent_type_family_names",
    "-A",
    "rlib::implicit_first_wins_deduplication",
    "-A",
    "rlib::long_method_chains",
    "-A",
    "rlib::malformed_code_phase_comments",
    "-A",
    "rlib::malformed_section_dividers",
    "-A",
    "rlib::mismatched_section_divider_prefixes",
    "-A",
    "rlib::missing_code_phase_comments",
    "-A",
    "rlib::missing_section_dividers",
    "-A",
    "rlib::misordered_test_declarations",
    "-A",
    "rlib::needless_delegating_types",
    "-A",
    "rlib::needlessly_nested_control_flow",
    "-A",
    "rlib::nested_tuple_types",
    "-A",
    "rlib::non_defining_module_reexports",
    "-A",
    "rlib::noncanonical_restricted_visibility",
    "-A",
    "rlib::non_adjacent_extension_trait_impls",
    "-A",
    "rlib::overloaded_declaration_sections",
    "-A",
    "rlib::oversized_match_arms",
    "-A",
    "rlib::positional_aggregate_fields",
    "-A",
    "rlib::repeated_identical_statements",
    "-A",
    "rlib::results_converted_to_options",
    "-A",
    "rlib::revalidated_string_parameters",
    "-A",
    "rlib::single_implementation_traits",
    "-A",
    "rlib::stringly_typed_domain_function_families",
    "-A",
    "rlib::unconsumed_generic_abstractions",
    "-A",
    "rlib::undocumented_early_returns",
    "-A",
    "rlib::undocumented_items",
    "-A",
    "rlib::unencapsulated_binary_enum_classification",
    "-A",
    "rlib::unnamed_policy_literals",
    "-A",
    "rlib::unnecessarily_broad_visibility",
    "-A",
    "rlib::unseparated_associated_items",
    "-A",
    "rlib::unseparated_module_items",
    "-A",
    "rlib::unparenthesized_mixed_boolean_operators",
    "-A",
    "rlib::visibility_required_only_by_tests",
];

// Core fixtures exercise analogous language-level contracts intentionally. Framework-provider
// recommendations are tested by their own dependency-aware examples and must not change core
// fixture snapshots when all features are enabled together.
/// Framework lints excluded from dependency-free core fixture snapshots.
#[cfg(any(feature = "derive_more", feature = "framework", feature = "thiserror"))]
const FIXTURE_CORE_FRAMEWORK_LINT_ALLOWS: [&str; 14] = [
    "-A",
    "unknown_lints",
    "-A",
    "rlib::derive_more_manual_equality_impls",
    "-A",
    "rlib::derive_more_manual_error_impls",
    "-A",
    "rlib::derive_more_manual_formatting_impls",
    "-A",
    "rlib::derive_more_manual_forwarding_interfaces",
    "-A",
    "rlib::framework_resolution_required",
    "-A",
    "rlib::thiserror_manual_error_impls",
];

// Leptos macro expansion intentionally produces shapes covered by these core lints. Keeping the
// exceptions here lets each core lint's own standalone fixture continue to exercise the warning.
/// Core lints excluded for shapes generated by Leptos macro expansion.
#[cfg(feature = "leptos")]
const FIXTURE_LEPTOS_GENERATED_CORE_LINT_ALLOWS: [&str; 20] = [
    "-A",
    "rlib::bool_fields_without_predicate_prefix",
    "-A",
    "rlib::collection_method_like_free_functions",
    "-A",
    "rlib::cross_file_struct_impls",
    "-A",
    "rlib::invalid_barrel_file_items",
    "-A",
    "rlib::method_like_free_functions",
    "-A",
    "rlib::misordered_inherent_impl_items",
    "-A",
    "rlib::misordered_module_declarations",
    "-A",
    "rlib::misordered_type_declarations",
    "-A",
    "rlib::needless_function_wrappers",
    "-A",
    "rlib::non_adjacent_struct_impls",
];

/// New source-wide policies excluded from the pre-existing Leptos fixture snapshots.
#[cfg(feature = "leptos")]
const FIXTURE_LEPTOS_SOURCE_POLICY_ALLOWS: [&str; 38] = [
    "-A",
    "rlib::leptos_noncanonical_view_formatting",
    "-A",
    "rlib::leptos_excessive_component_composition_depth",
    "-A",
    "rlib::leptos_excessive_component_props",
    "-A",
    "rlib::leptos_excessively_nested_views",
    "-A",
    "rlib::leptos_fragmented_reactive_state",
    "-A",
    "rlib::leptos_overpopulated_component_modules",
    "-A",
    "rlib::leptos_oversized_event_handlers",
    "-A",
    "rlib::leptos_oversized_reactive_setups",
    "-A",
    "rlib::leptos_repeated_view_fragments",
    "-A",
    "rlib::leptos_static_str_component_props",
    "-A",
    "rlib::leptos_unlocalized_view_literals",
    "-A",
    "rlib::leptos_unnamed_composables",
    "-A",
    "rlib::leptos_unscoped_spawned_tasks",
    "-A",
    "rlib::leptos_styling_inline_style_properties",
    "-A",
    "rlib::leptos_styling_non_colocated_component_styles",
    "-A",
    "rlib::leptos_styling_noncanonical_css",
    "-A",
    "rlib::leptos_styling_unscoped_component_selectors",
    "-A",
    "rlib::leptos_styling_unused_stylesheet_classes",
    "-A",
    "rlib::leptos_styling_untyped_component_classes",
];

/// Styling policies excluded while each Leptos architecture fixture isolates its target lint.
#[cfg(feature = "leptos")]
const FIXTURE_LEPTOS_STYLING_POLICY_ALLOWS: [&str; 12] = [
    "-A",
    "rlib::leptos_styling_inline_style_properties",
    "-A",
    "rlib::leptos_styling_non_colocated_component_styles",
    "-A",
    "rlib::leptos_styling_noncanonical_css",
    "-A",
    "rlib::leptos_styling_unscoped_component_selectors",
    "-A",
    "rlib::leptos_styling_unused_stylesheet_classes",
    "-A",
    "rlib::leptos_styling_untyped_component_classes",
];

/// Localization lint excluded while non-localization Leptos fixtures isolate their own warning.
#[cfg(feature = "leptos")]
const FIXTURE_LEPTOS_I18N_POLICY_ALLOWS: [&str; 4] = [
    "-A",
    "unknown_lints",
    "-A",
    "rlib::leptos_unlocalized_view_literals",
];

/// Makes Dylint's internal `cargo build` preserve the test process's feature set.
#[cfg(all(
    any(
        feature = "strum",
        feature = "bon",
        feature = "derive_more",
        feature = "miette",
        feature = "serde",
        feature = "thiserror"
    ),
    unix
))]
fn fixture_rerun_with_feature_aware_cargo_wrapper() -> bool {
    // The feature-aware child process must not recursively spawn another Cargo run.
    if var_os("RLIB_LINT_ALL_FEATURE_UI_CHILD").is_some() {
        return false;
    }

    // Derive the exact Cargo feature arguments inherited by the child process.
    let directory = temp_dir().join(format!("rlib-lint-cargo-wrapper-{}", id()));
    create_dir_all(&directory).expect("Cargo wrapper directory should be creatable");
    let wrapper = directory.join("cargo");

    // Select Cargo flags that reproduce the parent crate's enabled providers.
    let enabled_features = [
        cfg!(feature = "strum").then_some("strum"),
        cfg!(feature = "bon").then_some("bon"),
        cfg!(feature = "derive_more").then_some("derive_more"),
        cfg!(feature = "miette").then_some("miette"),
        cfg!(feature = "serde").then_some("serde"),
        cfg!(feature = "thiserror").then_some("thiserror"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(",");
    let feature_flags = if cfg!(feature = "leptos") {
        "--all-features".to_owned()
    } else {
        format!("--no-default-features --features {enabled_features}")
    };

    // Materialize an executable Cargo shim that preserves those feature arguments.
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

    // Re-enter this exact test through the shim and propagate its success.
    let cargo = var("CARGO").expect("Cargo should expose its own executable path");
    let path = join_paths(once(directory).chain(split_paths(
        &var_os("PATH").expect("test process should have PATH"),
    )))
    .expect("Cargo wrapper PATH should be valid");

    // Configure the child command with the wrapper contract and inherited path.
    let executable = current_exe().expect("UI test executable should be available");
    let mut command = Command::new(executable);
    command.args(["fixture_ui", "--exact", "--nocapture"]);
    command.env("RLIB_LINT_ALL_FEATURE_UI_CHILD", "1");
    command.env("RLIB_LINT_REAL_CARGO", cargo);
    command.env("DYLINT_TOML", include_str!("../../../dylint.toml"));
    command.env("PATH", path);
    let status = command
        .status()
        .expect("wrapped all-feature UI child should start");
    assert!(status.success(), "wrapped all-feature UI child failed");
    true
}

/// Rejects unsupported all-feature UI execution platforms explicitly.
#[cfg(all(
    any(
        feature = "strum",
        feature = "bon",
        feature = "derive_more",
        feature = "miette",
        feature = "serde",
        feature = "thiserror"
    ),
    not(unix)
))]
fn fixture_rerun_with_feature_aware_cargo_wrapper() -> bool {
    panic!("all-feature Dylint UI tests currently require a Unix Cargo wrapper");
}

/// Returns the core fixture selected for focused local regression testing.
fn fixture_selected_core() -> Option<String> {
    match var("RLIB_LINT_CORE_FIXTURE") {
        Ok(fixture) if !fixture.is_empty() => Some(fixture),
        Ok(_) | Err(VarError::NotPresent) => None,
        Err(VarError::NotUnicode(_)) => panic!("RLIB_LINT_CORE_FIXTURE must be valid Unicode"),
    }
}

/// Runs fixtures that rustc can compile directly without Cargo dependency metadata.
fn fixture_run_standalone() {
    let source = fixture_selected_core().map_or_else(
        || "ui/core".to_owned(),
        |fixture| format!("ui/core/{fixture}"),
    );
    let mut test = Test::src_base(env!("CARGO_PKG_NAME"), source);
    test.rustc_flags(["--edition=2024"]);
    test.rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS);
    #[cfg(any(feature = "derive_more", feature = "framework", feature = "thiserror"))]
    test.rustc_flags(FIXTURE_CORE_FRAMEWORK_LINT_ALLOWS);
    test.run();
}

/// Returns the framework fixture selected by the current or legacy environment variable.
#[cfg(any(
    feature = "strum",
    feature = "leptos",
    feature = "bon",
    feature = "derive_more",
    feature = "miette",
    feature = "serde",
    feature = "thiserror"
))]
fn fixture_selected_framework() -> Option<String> {
    match var("RLIB_LINT_FRAMEWORK_FIXTURE") {
        Ok(fixture) => Some(fixture),
        Err(VarError::NotPresent) => match var("RLIB_LINT_STRUM_FIXTURE") {
            Ok(fixture) => Some(fixture),
            Err(VarError::NotPresent) => None,
            Err(VarError::NotUnicode(_)) => {
                panic!("RLIB_LINT_STRUM_FIXTURE must be valid Unicode")
            }
        },
        Err(VarError::NotUnicode(_)) => {
            panic!("RLIB_LINT_FRAMEWORK_FIXTURE must be valid Unicode")
        }
    }
}

/// Runs the compatibility fixture that requires an authored Bon API baseline.
#[cfg(feature = "bon")]
fn fixture_run_bon_compatibility() {
    Test::example(
        env!("CARGO_PKG_NAME"),
        "bon_required_builder_members_breaking_compatibility",
    )
    .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
    .dylint_toml(
        r#"
                [rlib-lint]
                bon-api-baseline = [
                    { builder = "Request", members = ["host"] },
                    { builder = "upload", members = ["path"] },
                    { builder = "Client::connect", members = ["host"] },
                ]
            "#,
    )
    .run();
}

/// Runs Bon examples selected by the shared framework fixture filter.
#[cfg(feature = "bon")]
fn fixture_run_bon() {
    let selected = fixture_selected_framework();
    for example in [
        "bon_builders_bypassing_construction_invariants",
        "bon_escaping_incomplete_builders",
        "bon_implicit_optional_builder_members",
        "bon_incoherent_builder_vocabulary",
        "bon_incoherent_conditional_builder_members",
        "bon_inconsistent_builder_conversions",
        "bon_manual_builder_implementations",
        "bon_needless_builders_for_small_apis",
        "bon_parameter_heavy_apis_without_builders",
        "bon_public_builder_implementation_types",
        "bon_redundant_positional_and_builder_apis",
        "bon_skipped_builder_members_without_policy",
        "bon_undocumented_builder_members",
    ] {
        if selected
            .as_deref()
            .is_some_and(|selected| selected != example)
        {
            continue;
        }
        Test::example(env!("CARGO_PKG_NAME"), example)
            .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
            .run();
    }

    // Selective UI runs skip this compatibility fixture unless its lint was requested.
    if selected
        .as_deref()
        .is_some_and(|selected| selected != "bon_required_builder_members_breaking_compatibility")
    {
        return;
    }
    fixture_run_bon_compatibility();
}

/// Runs `derive_more` examples selected by the shared framework fixture filter.
#[cfg(feature = "derive_more")]
fn fixture_run_derive_more() {
    let selected = fixture_selected_framework();
    for example in [
        "derive_more_ambiguous_derived_error_sources",
        "derive_more_derived_constructors_bypassing_invariants",
        "derive_more_derived_conversions_bypassing_invariants",
        "derive_more_inconsistent_derived_equality",
        "derive_more_manual_aggregation_impls",
        "derive_more_manual_conversion_impls",
        "derive_more_manual_constructors",
        "derive_more_manual_equality_impls",
        "derive_more_manual_forwarding_interfaces",
        "derive_more_manual_formatting_impls",
        "derive_more_manual_from_str_impls",
        "derive_more_manual_into_iterator_impls",
        "derive_more_manual_operator_impls",
        "derive_more_mutable_forwarding_bypassing_invariants",
        "derive_more_non_roundtripping_derived_text_contracts",
        "derive_more_opaque_derived_display_contracts",
        "derive_more_operator_derives_bypassing_invariants",
        "derive_more_panic_prone_derived_variant_accessors",
    ] {
        if selected
            .as_deref()
            .is_some_and(|selected| selected != example)
        {
            continue;
        }
        Test::example(env!("CARGO_PKG_NAME"), example)
            .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
            .run();
    }
    if selected
        .as_deref()
        .is_none_or(|selected| selected == "derive_more_manual_variant_accessors")
    {
        Test::example(
            env!("CARGO_PKG_NAME"),
            "derive_more_manual_variant_accessors",
        )
        .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
        .dylint_toml(
            r#"
                [rlib-lint]
                enum-variant-predicate-provider = "derive_more_is_variant"
            "#,
        )
        .run();
    }

    // Selective UI runs skip this manual-error fixture unless its lint was requested.
    if selected
        .as_deref()
        .is_some_and(|selected| selected != "derive_more_manual_error_impls")
    {
        return;
    }
    #[cfg(not(feature = "thiserror"))]
    Test::example(env!("CARGO_PKG_NAME"), "derive_more_manual_error_impls")
        .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
        .run();
    #[cfg(feature = "thiserror")]
    Test::example(env!("CARGO_PKG_NAME"), "derive_more_manual_error_impls")
        .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
        .dylint_toml(
            r#"
                    [rlib-lint]
                    error-implementation-provider = "derive_more_error"
                "#,
        )
        .run();
}

/// Runs Serde examples selected by the shared framework fixture filter.
#[cfg(feature = "serde")]
fn fixture_run_serde() {
    let selected = fixture_selected_framework();
    for example in [
        "serde_ambiguous_untagged_enums",
        "serde_asymmetric_serde_contracts",
        "serde_catch_all_variants_hiding_schema_drift",
        "serde_deserialization_bypassing_invariants",
        "serde_defaults_hiding_missing_data",
        "serde_duplicate_serialized_names",
        "serde_flattened_field_collisions",
        "serde_flattened_unknown_field_policies",
        "serde_format_specific_serde_impls",
        "serde_lossy_conditional_serialization",
        "serde_manual_deserialize_impls",
        "serde_manual_serialize_impls",
        "serde_non_roundtripping_serde_adapters",
        "serde_remote_representations_drifting_from_sources",
        "serde_sensitive_fields_serialized_by_default",
        "serde_unstable_implicit_wire_names",
    ] {
        if selected
            .as_deref()
            .is_some_and(|selected| selected != example)
        {
            continue;
        }
        Test::example(env!("CARGO_PKG_NAME"), example)
            .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
            .run();
    }
}

/// Runs Miette examples selected by the shared framework fixture filter.
#[cfg(feature = "miette")]
fn fixture_run_miette() {
    let selected = fixture_selected_framework();
    for example in [
        "miette_ad_hoc_diagnostics_at_domain_boundaries",
        "miette_duplicate_diagnostic_codes",
        "miette_generic_diagnostic_help",
        "miette_incoherent_diagnostic_severity",
        "miette_labels_without_source_code",
        "miette_malformed_diagnostic_codes",
        "miette_manual_diagnostic_impls",
        "miette_misclassified_related_diagnostics",
        "miette_missing_diagnostic_codes",
        "miette_plain_error_diagnostic_sources",
        "miette_sensitive_diagnostic_source",
        "miette_source_code_without_labels",
        "miette_unstable_diagnostic_urls",
        "miette_unfocused_diagnostic_labels",
    ] {
        if selected
            .as_deref()
            .is_some_and(|selected| selected != example)
        {
            continue;
        }
        let mut test = Test::example(env!("CARGO_PKG_NAME"), example);
        test.rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS);
        #[cfg(feature = "thiserror")]
        if example == "miette_ad_hoc_diagnostics_at_domain_boundaries" {
            test.rustc_flags(["-A", "rlib::thiserror_dynamic_errors_in_library_interfaces"]);
        }
        test.run();
    }

    // Selective UI runs skip this library-report fixture unless its lint was requested.
    if selected
        .as_deref()
        .is_some_and(|selected| selected != "miette_reports_in_library_interfaces")
    {
        return;
    }
    let mut test = Test::example(
        env!("CARGO_PKG_NAME"),
        "miette_reports_in_library_interfaces",
    );
    test.rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
        .rustc_flags(["--crate-type=lib"]);
    #[cfg(feature = "thiserror")]
    test.rustc_flags(["-A", "rlib::thiserror_dynamic_errors_in_library_interfaces"]);
    test.run();
}

/// Runs thiserror examples selected by the shared framework fixture filter.
#[cfg(feature = "thiserror")]
fn fixture_run_thiserror() {
    let selected = fixture_selected_framework();
    #[cfg(feature = "derive_more")]
    if selected
        .as_deref()
        .is_none_or(|selected| selected == "framework_error_variant_conversion_resolution_required")
    {
        Test::example(
            env!("CARGO_PKG_NAME"),
            "framework_error_variant_conversion_resolution_required",
        )
        .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
        .run();
    }
    #[cfg(feature = "derive_more")]
    if selected
        .as_deref()
        .is_none_or(|selected| selected == "framework_error_implementation_resolution_required")
    {
        Test::example(
            env!("CARGO_PKG_NAME"),
            "framework_error_implementation_resolution_required",
        )
        .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
        .run();
    }
    for example in [
        "thiserror_ambiguous_error_sources",
        "thiserror_duplicate_error_messages",
        "thiserror_dynamic_errors_in_library_interfaces",
        "thiserror_error_messages_used_as_identifiers",
        "thiserror_from_sources_without_context",
        "thiserror_non_send_sync_public_errors",
        "thiserror_opaque_errors_exposing_representations",
        "thiserror_unpropagated_error_backtraces",
        "thiserror_unreported_error_sources",
    ] {
        if selected
            .as_deref()
            .is_some_and(|selected| selected != example)
        {
            continue;
        }
        Test::example(env!("CARGO_PKG_NAME"), example)
            .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
            .run();
    }
    if selected
        .as_deref()
        .is_none_or(|selected| selected == "thiserror_manual_error_impls")
    {
        Test::example(env!("CARGO_PKG_NAME"), "thiserror_manual_error_impls")
            .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
            .dylint_toml(
                r#"
                    [rlib-lint]
                    error-implementation-provider = "thiserror_error"
                "#,
            )
            .run();
    }

    // Selective UI runs skip this error-conversion fixture unless its lint was requested.
    if selected
        .as_deref()
        .is_some_and(|selected| selected != "thiserror_manual_from_error_variants")
    {
        return;
    }
    Test::example(
        env!("CARGO_PKG_NAME"),
        "thiserror_manual_from_error_variants",
    )
    .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
    .dylint_toml(
        r#"
                [rlib-lint]
                error-variant-conversion-provider = "thiserror_from"
            "#,
    )
    .run();
}

/// Runs the authorization fixture with representative endpoint vocabulary.
#[cfg(feature = "leptos")]
fn fixture_run_leptos_authorization() {
    Test::example(
        env!("CARGO_PKG_NAME"),
        "leptos_server_functions_without_authorization_boundaries",
    )
    .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
    .rustc_flags(FIXTURE_LEPTOS_GENERATED_CORE_LINT_ALLOWS)
    .rustc_flags(FIXTURE_LEPTOS_SOURCE_POLICY_ALLOWS)
    .dylint_toml(
        r#"
                [rlib-lint]
                leptos-sensitive-call-terms = ["delete_account", "read_private_profile"]
                leptos-authorization-functions = ["authorize_account_admin"]
                leptos-protected-endpoint-attributes = ["protected_endpoint"]
                leptos-public-endpoint-attributes = ["public_endpoint"]
            "#,
    )
    .run();
}

/// Runs the source-wide Leptos architecture fixtures.
#[cfg(feature = "leptos")]
fn fixture_run_leptos_architecture(selected: Option<&str>) {
    for example in [
        "leptos_excessive_component_composition_depth",
        "leptos_excessive_component_props",
        "leptos_excessively_nested_views",
        "leptos_fragmented_reactive_state",
        "leptos_overpopulated_component_modules",
        "leptos_oversized_event_handlers",
        "leptos_oversized_reactive_setups",
        "leptos_repeated_view_fragments",
        "leptos_unnamed_composables",
        "leptos_unscoped_spawned_tasks",
    ] {
        if selected.is_some_and(|selected| selected != example) {
            continue;
        }
        Test::example(env!("CARGO_PKG_NAME"), example)
            .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
            .rustc_flags(FIXTURE_LEPTOS_GENERATED_CORE_LINT_ALLOWS)
            .rustc_flags(FIXTURE_LEPTOS_STYLING_POLICY_ALLOWS)
            .rustc_flags(FIXTURE_LEPTOS_I18N_POLICY_ALLOWS)
            .run();
    }
}

/// Runs all Cargo examples that need dependency linking or macro expansion.
#[cfg(feature = "leptos")]
fn fixture_run_leptos() {
    let selected = fixture_selected_framework();
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
        "leptos_implicit_default_component_props",
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
            .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
            .rustc_flags(FIXTURE_LEPTOS_GENERATED_CORE_LINT_ALLOWS)
            .rustc_flags(FIXTURE_LEPTOS_SOURCE_POLICY_ALLOWS)
            .run();
    }
    if selected
        .as_deref()
        .is_none_or(|selected| selected == "leptos_noncanonical_view_formatting")
    {
        Test::example(
            env!("CARGO_PKG_NAME"),
            "leptos_noncanonical_view_formatting",
        )
        .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
        .rustc_flags(FIXTURE_LEPTOS_GENERATED_CORE_LINT_ALLOWS)
        .rustc_flags(FIXTURE_LEPTOS_I18N_POLICY_ALLOWS)
        .run();
    }

    for example in ["leptos_static_str_component_props"] {
        if selected
            .as_deref()
            .is_some_and(|selected| selected != example)
        {
            continue;
        }
        Test::example(env!("CARGO_PKG_NAME"), example)
            .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
            .rustc_flags(FIXTURE_LEPTOS_GENERATED_CORE_LINT_ALLOWS)
            .rustc_flags([
                "-A",
                "rlib::leptos_excessive_component_props",
                "-A",
                "rlib::leptos_noncanonical_view_formatting",
                "-A",
                "rlib::leptos_overpopulated_component_modules",
                "-A",
                "rlib::leptos_unlocalized_view_literals",
            ])
            .run();
    }

    fixture_run_leptos_architecture(selected.as_deref());

    // Selective UI runs skip this authorization fixture unless its lint was requested.
    if selected.as_deref().is_some_and(|selected| {
        selected != "leptos_server_functions_without_authorization_boundaries"
    }) {
        return;
    }
    fixture_run_leptos_authorization();
}

/// Runs the localization example with unrelated Leptos policies disabled.
#[cfg(feature = "leptos_i18n")]
fn fixture_run_leptos_i18n_example() {
    Test::example(env!("CARGO_PKG_NAME"), "leptos_unlocalized_view_literals")
        .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
        .rustc_flags(FIXTURE_LEPTOS_GENERATED_CORE_LINT_ALLOWS)
        .rustc_flags([
            "-A",
            "rlib::leptos_duplicate_view_section_comments",
            "-A",
            "rlib::leptos_missing_view_attribute_group_comments",
            "-A",
            "rlib::leptos_missing_view_section_comments",
            "-A",
            "rlib::leptos_noncanonical_view_formatting",
            "-A",
            "rlib::leptos_overpopulated_component_modules",
            "-A",
            "rlib::leptos_oversized_view_attribute_groups",
            "-A",
            "rlib::leptos_repeated_view_fragments",
        ])
        .run();
}

/// Runs localization-specific Leptos fixtures in their own feature layer.
#[cfg(feature = "leptos_i18n")]
fn fixture_run_leptos_i18n() {
    let selected = fixture_selected_framework();

    // A focused run for another lint must not execute the localization fixture.
    if selected
        .as_deref()
        .is_some_and(|selected| selected != "leptos_unlocalized_view_literals")
    {
        return;
    }
    fixture_run_leptos_i18n_example();
}

/// Runs source-oriented styling fixtures with only their sibling policies suppressed in-source.
#[cfg(feature = "leptos_styling")]
fn fixture_run_leptos_styling() {
    let selected = fixture_selected_framework();
    for example in [
        "leptos_styling_inline_style_properties",
        "leptos_styling_non_colocated_component_styles",
        "leptos_styling_noncanonical_css",
        "leptos_styling_unscoped_component_selectors",
        "leptos_styling_unused_stylesheet_classes",
        "leptos_styling_untyped_component_classes",
    ] {
        if selected
            .as_deref()
            .is_some_and(|selected| selected != example)
        {
            continue;
        }
        Test::example(env!("CARGO_PKG_NAME"), example)
            .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
            .rustc_flags(FIXTURE_LEPTOS_GENERATED_CORE_LINT_ALLOWS)
            .rustc_flags(FIXTURE_LEPTOS_I18N_POLICY_ALLOWS)
            .run();
    }
}

/// Runs each Strum fixture with the provider policy its expected diagnostic requires.
#[cfg(feature = "strum")]
fn fixture_run_strum() {
    let selected = fixture_selected_framework();
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
            .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
            .run();
    }
    if selected
        .as_deref()
        .is_none_or(|selected| selected == "strum_manual_variant_arrays")
    {
        Test::example(env!("CARGO_PKG_NAME"), "strum_manual_variant_arrays")
            .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
            .dylint_toml(
                r#"
                [rlib-lint]
                enum-variant-collection-provider = "strum_variant_array"
            "#,
            )
            .run();
    }
    if selected
        .as_deref()
        .is_none_or(|selected| selected == "strum_manual_enum_predicates")
    {
        Test::example(env!("CARGO_PKG_NAME"), "strum_manual_enum_predicates")
            .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
            .dylint_toml(
                r#"
                [rlib-lint]
                enum-variant-predicate-provider = "strum_enum_is"
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
        .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
        .dylint_toml(
            r#"
                [rlib-lint]
                enum-display-provider = "strum_display"
            "#,
        )
        .run();
    }

    // Selective UI runs skip this enum-parser fixture unless its lint was requested.
    if selected
        .as_deref()
        .is_some_and(|selected| selected != "strum_manual_enum_string_parsers")
    {
        return;
    }
    Test::example(env!("CARGO_PKG_NAME"), "strum_manual_enum_string_parsers")
        .rustc_flags(FIXTURE_CROSS_CUTTING_LINT_ALLOWS)
        .dylint_toml(
            r#"
                    [rlib-lint]
                    enum-string-parsing-provider = "strum_enum_string"
                "#,
        )
        .run();
}

/// Runs every standalone and dependency-aware UI fixture against the lint library.
#[test]
fn fixture_ui() {
    #[cfg(any(
        feature = "strum",
        feature = "bon",
        feature = "derive_more",
        feature = "miette",
        feature = "serde",
        feature = "thiserror"
    ))]
    // A completed feature-aware rerun owns the fixture result and ends the parent test.
    if fixture_rerun_with_feature_aware_cargo_wrapper() {
        return;
    }
    #[cfg(any(
        feature = "strum",
        feature = "leptos",
        feature = "bon",
        feature = "derive_more",
        feature = "miette",
        feature = "serde",
        feature = "thiserror"
    ))]
    if fixture_selected_framework().is_none() {
        fixture_run_standalone();
    }
    #[cfg(not(any(
        feature = "strum",
        feature = "leptos",
        feature = "bon",
        feature = "derive_more",
        feature = "miette",
        feature = "serde",
        feature = "thiserror"
    )))]
    fixture_run_standalone();
    #[cfg(feature = "strum")]
    fixture_run_strum();
    #[cfg(feature = "leptos")]
    fixture_run_leptos();
    #[cfg(feature = "leptos_i18n")]
    fixture_run_leptos_i18n();
    #[cfg(feature = "leptos_styling")]
    fixture_run_leptos_styling();
    #[cfg(feature = "bon")]
    fixture_run_bon();
    #[cfg(feature = "derive_more")]
    fixture_run_derive_more();
    #[cfg(feature = "miette")]
    fixture_run_miette();
    #[cfg(feature = "serde")]
    fixture_run_serde();
    #[cfg(feature = "thiserror")]
    fixture_run_thiserror();
}
