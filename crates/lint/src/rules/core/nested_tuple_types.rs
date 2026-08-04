extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::{AmbigArg, Ty};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::tuple_types::{ExplicitTupleKind, ExplicitTupleType};

// -----------------------------------------------------------------------------
// NestedTupleTypes
// -----------------------------------------------------------------------------

struct NestedTupleTypes;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds non-unit tuple types nested inside another explicit type, including references,
    /// slices, arrays, collections, options, results, and associated type bindings.
    ///
    /// ### Why is this bad?
    ///
    /// Nesting an anonymous tuple makes an already structural type harder to understand. The
    /// surrounding container explains storage or ownership, but it cannot explain what each tuple
    /// position represents. Repeating the shape also spreads an unnamed protocol across APIs.
    ///
    /// For example, neither component of each affected entry is named:
    ///
    /// ```rust
    /// fn affected() -> &'static [(&'static Participant, NameTokens)] {
    ///     todo!()
    /// }
    /// ```
    ///
    /// Name the element concept and its roles before placing it in the container:
    ///
    /// ```rust
    /// struct AffectedParticipant<'a> {
    ///     participant: &'a Participant,
    ///     name: NameTokens,
    /// }
    ///
    /// fn affected() -> &'static [AffectedParticipant<'static>] {
    ///     todo!()
    /// }
    /// ```
    pub NESTED_TUPLE_TYPES,
    Warn,
    "rejects tuple types nested inside explicit structural types",
    NestedTupleTypes
}

impl LateLintPass<'_> for NestedTupleTypes {
    fn check_ty(&mut self, cx: &LateContext<'_>, ty: &Ty<'_, AmbigArg>) {
        let Some(tuple) = ExplicitTupleType::classify(cx, ty) else {
            return;
        };
        if tuple.kind != ExplicitTupleKind::Nested {
            return;
        }
        cx.emit_span_lint(
            NESTED_TUPLE_TYPES,
            tuple.root_span,
            DiagDecorator(|diag| {
                diag.primary_message("this explicit type contains an unnamed tuple shape");
                diag.span_label(
                    tuple.tuple_span,
                    "the nested tuple hides its component roles",
                );
                diag.help(
                    "replace the tuple with a concept-specific record struct and named fields",
                );
            }),
        );
    }
}
