extern crate rustc_lint;
extern crate rustc_session;

use crate::rules;

// Registers this library as a dylint plugin with the compiler process that loaded it.
dylint_linting::dylint_library!();

/// Registers every project lint with the compiler process that loaded this library.
///
/// Keeping this wiring outside `lib.rs` lets the crate root remain a readable map of the library.
#[unsafe(no_mangle)]
pub extern "Rust" fn register_lints(
    sess: &rustc_session::Session,
    lint_store: &mut rustc_lint::LintStore,
) {
    rules::core::bool_fields_without_predicate_prefix::register_lints(sess, lint_store);
    rules::core::collection_method_like_free_functions::register_lints(sess, lint_store);
    rules::core::cross_file_struct_impls::register_lints(sess, lint_store);
    rules::core::invalid_barrel_file_items::register_lints(sess, lint_store);
    rules::core::method_like_free_functions::register_lints(sess, lint_store);
    rules::core::misordered_inherent_impl_items::register_lints(sess, lint_store);
    rules::core::misordered_module_declarations::register_lints(sess, lint_store);
    rules::core::misordered_type_declarations::register_lints(sess, lint_store);
    rules::core::needless_function_wrappers::register_lints(sess, lint_store);
    rules::core::non_adjacent_struct_impls::register_lints(sess, lint_store);
}
