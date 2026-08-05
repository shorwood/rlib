extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::{AmbigArg, Ty};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::tuple_types::{ExplicitTupleKind, ExplicitTupleType};

// -----------------------------------------------------------------------------
// BareTupleTypes
// -----------------------------------------------------------------------------

/// Late lint pass that rejects explicit root tuple types without semantic field names.
struct BareTupleTypes;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds non-unit tuple types used as the root of an explicit type annotation. This includes
    /// fields, parameters, return types, aliases, associated types, locals, and closure
    /// annotations.
    ///
    /// ### Why is this bad?
    ///
    /// A tuple records positions but not their meaning. Readers must recover each component's role
    /// from surrounding code, and later additions or reordered elements are easy to misuse. A
    /// concept-specific record also gives agents a vocabulary to reuse instead of propagating an
    /// anonymous structural shape.
    ///
    /// For example, this return type leaves both values unnamed:
    ///
    /// ```rust
    /// fn partition() -> (Accepted, Rejected) {
    ///     todo!()
    /// }
    /// ```
    ///
    /// Introduce a record whose type and fields explain the result:
    ///
    /// ```rust
    /// struct Partition {
    ///     accepted: Accepted,
    ///     rejected: Rejected,
    /// }
    ///
    /// fn partition() -> Partition {
    ///     todo!()
    /// }
    /// ```
    pub BARE_TUPLE_TYPES,
    Warn,
    "rejects bare tuple types in explicit type annotations",
    BareTupleTypes
}

impl LateLintPass<'_> for BareTupleTypes {
    fn check_ty(&mut self, cx: &LateContext<'_>, ty: &Ty<'_, AmbigArg>) {
        let Some(tuple) = ExplicitTupleType::classify(cx, ty) else {
            return;
        };
        if tuple.kind != ExplicitTupleKind::Bare {
            return;
        }
        cx.emit_span_lint(
            BARE_TUPLE_TYPES,
            tuple.root_span,
            DiagDecorator(|diag| {
                diag.primary_message("this explicit tuple type leaves component roles unnamed");
                diag.help(
                    "introduce a concept-specific record struct with a meaningful name and named fields",
                );
            }),
        );
    }
}
