#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_lint;
extern crate rustc_session;

mod rules;

dylint_linting::dylint_library!();

#[unsafe(no_mangle)]
pub extern "Rust" fn register_lints(
    sess: &rustc_session::Session,
    lint_store: &mut rustc_lint::LintStore,
) {
    rules::core::enforce_struct_bool_prefix::register_lints(sess, lint_store);
}

#[cfg(test)]
mod tests {
    #[test]
    fn ui() {
        dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
    }
}
