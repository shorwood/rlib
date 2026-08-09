extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{AmbigArg, Ty};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::tuple_types::{ExplicitTupleKind, ExplicitTupleType};

// -----------------------------------------------------------------------------
// Violation: Bare tuple type diagnostic
// -----------------------------------------------------------------------------

/// Explicit root tuple whose positional components have no semantic names.
struct Violation {
    /// Complete tuple type receiving the diagnostic.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("this explicit tuple type leaves component roles unnamed")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "callers must repeatedly infer each position, and reordered components remain type-compatible",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "introduce a concept-specific record struct with a meaningful name and named fields",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            BARE_TUPLE_TYPES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// BareTupleTypes: Lint pass
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
        // Check for a bare tuple type at the root of an explicit type annotation.
        let Some(tuple) = ExplicitTupleType::classify(cx, ty) else {
            return;
        };

        // Only bare tuples are rejected; named tuples and unit tuples are permitted.
        if tuple.kind != ExplicitTupleKind::Bare {
            return;
        }

        Violation {
            span: tuple.root_span,
        }
        .emit(cx);
    }
}
