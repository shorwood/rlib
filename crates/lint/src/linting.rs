/// Declares one `rlib::` tool lint and wires its pre-expansion pass into Dylint.
#[macro_export]
macro_rules! impl_pre_expansion_lint {
    ($(#[$attr:meta])* $vis:vis $NAME:ident, $Level:ident, $desc:expr, $pass:expr) => {
        extern crate rustc_lint;
        extern crate rustc_session;

        rustc_session::declare_tool_lint! {
            $(#[$attr])* $vis rlib::$NAME,
            $Level,
            $desc
        }

        #[doc(hidden)]
        #[allow(
            rlib::foreign_type_method_like_free_functions,
            rlib::undocumented_items
        )]
        pub fn register_lints(
            _session: &rustc_session::Session,
            lint_store: &mut rustc_lint::LintStore,
        ) {
            lint_store.register_lints(&[$NAME]);
            lint_store.register_pre_expansion_pass(|| Box::new($pass));
        }

        dylint_linting::paste::paste! {
            rustc_session::impl_lint_pass!([< $NAME:camel >] => [$NAME]);
        }
    };
}

/// Declares one `rlib::` tool lint and wires its early pass into Dylint.
#[macro_export]
macro_rules! impl_early_lint {
    ($(#[$attr:meta])* $vis:vis $NAME:ident, $Level:ident, $desc:expr, $pass:expr) => {
        extern crate rustc_lint;
        extern crate rustc_session;

        rustc_session::declare_tool_lint! {
            $(#[$attr])* $vis rlib::$NAME,
            $Level,
            $desc
        }

        #[doc(hidden)]
        #[allow(
            rlib::foreign_type_method_like_free_functions,
            rlib::undocumented_items
        )]
        pub fn register_lints(
            _session: &rustc_session::Session,
            lint_store: &mut rustc_lint::LintStore,
        ) {
            lint_store.register_lints(&[$NAME]);
            lint_store.register_early_pass(|| Box::new($pass));
        }

        dylint_linting::paste::paste! {
            rustc_session::impl_lint_pass!([< $NAME:camel >] => [$NAME]);
        }
    };
}

/// Declares one `rlib::` tool lint and wires its late pass into Dylint.
#[macro_export]
macro_rules! impl_late_lint {
    ($(#[$attr:meta])* $vis:vis $NAME:ident, $Level:ident, $desc:expr, $pass:expr) => {
        extern crate rustc_lint;
        extern crate rustc_session;

        rustc_session::declare_tool_lint! {
            $(#[$attr])* $vis rlib::$NAME,
            $Level,
            $desc
        }

        #[doc(hidden)]
        #[allow(
            rlib::foreign_type_method_like_free_functions,
            rlib::undocumented_items
        )]
        pub fn register_lints(
            _session: &rustc_session::Session,
            lint_store: &mut rustc_lint::LintStore,
        ) {
            lint_store.register_lints(&[$NAME]);
            lint_store.register_late_pass(dylint_linting::__make_late_closure!($pass));
        }

        dylint_linting::paste::paste! {
            rustc_session::impl_lint_pass!([< $NAME:camel >] => [$NAME]);
        }
    };
}
