extern crate rustc_lint;
extern crate rustc_session;

use crate::rules;

// Registers this library as a dylint plugin with the compiler process that loaded it.
dylint_linting::dylint_library!();

/// Registers every project lint with the compiler process that loaded this library.
///
/// Keeping this wiring outside `lib.rs` lets the crate root remain a readable map of the library.
#[expect(
    unsafe_code,
    reason = "Dylint locates the registration entry point by its unmangled symbol name"
)]
#[unsafe(no_mangle)]
pub extern "Rust" fn register_lints(
    sess: &rustc_session::Session,
    lint_store: &mut rustc_lint::LintStore,
) {
    // Register aggregate, dependency, and field-naming policies.
    rules::core::bare_tuple_types::register_lints(sess, lint_store);
    rules::core::bidirectional_module_dependencies::register_lints(sess, lint_store);
    rules::core::bool_fields_without_predicate_prefix::register_lints(sess, lint_store);
    rules::core::collection_method_like_free_functions::register_lints(sess, lint_store);
    rules::core::cross_file_struct_impls::register_lints(sess, lint_store);

    // Register control-flow and section identity policies.
    rules::core::deeply_nested_control_flow::register_lints(sess, lint_store);
    rules::core::duplicate_section_divider_prefixes::register_lints(sess, lint_store);
    rules::core::incoherent_type_family_names::register_lints(sess, lint_store);
    rules::core::implicit_first_wins_deduplication::register_lints(sess, lint_store);
    rules::core::invalid_barrel_file_items::register_lints(sess, lint_store);

    // Register expression layout and declaration ordering policies.
    rules::core::long_method_chains::register_lints(sess, lint_store);
    rules::core::malformed_code_phase_comments::register_lints(sess, lint_store);
    rules::core::malformed_section_dividers::register_lints(sess, lint_store);
    rules::core::method_like_free_functions::register_lints(sess, lint_store);
    rules::core::misordered_inherent_impl_items::register_lints(sess, lint_store);

    // Register remaining ordering and phase-boundary policies.
    rules::core::misordered_module_declarations::register_lints(sess, lint_store);
    rules::core::misordered_type_declarations::register_lints(sess, lint_store);
    rules::core::mismatched_section_divider_prefixes::register_lints(sess, lint_store);
    rules::core::missing_code_phase_comments::register_lints(sess, lint_store);
    rules::core::missing_section_dividers::register_lints(sess, lint_store);

    // Register nesting, tuple, and statement-level policies.
    rules::core::needless_function_wrappers::register_lints(sess, lint_store);
    rules::core::needlessly_nested_control_flow::register_lints(sess, lint_store);
    rules::core::nested_tuple_types::register_lints(sess, lint_store);
    rules::core::non_adjacent_struct_impls::register_lints(sess, lint_store);
    rules::core::oversized_match_arms::register_lints(sess, lint_store);

    // Register aggregate representation and statement-expression policies.
    rules::core::positional_aggregate_fields::register_lints(sess, lint_store);
    rules::core::repeated_identical_statements::register_lints(sess, lint_store);
    rules::core::undocumented_items::register_lints(sess, lint_store);
    rules::core::unparenthesized_mixed_boolean_operators::register_lints(sess, lint_store);
}
