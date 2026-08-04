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
    rules::core::enforce_barrel_files::register_lints(sess, lint_store);
    rules::core::enforce_implementable_methods::register_lints(sess, lint_store);
    rules::core::enforce_implementable_methods_on_vectors::register_lints(sess, lint_store);
    rules::core::enforce_inherent_impl_item_order::register_lints(sess, lint_store);
    rules::core::enforce_module_declaration_order::register_lints(sess, lint_store);
    rules::core::enforce_no_redundant_function_wrappers::register_lints(sess, lint_store);
    rules::core::enforce_struct_bool_prefix::register_lints(sess, lint_store);
    rules::core::enforce_struct_impl_colocation::register_lints(sess, lint_store);
    rules::core::enforce_struct_impl_definition_order::register_lints(sess, lint_store);
    rules::core::enforce_type_dependency_order::register_lints(sess, lint_store);
}
