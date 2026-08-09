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
// Violation: Nested tuple type diagnostic
// -----------------------------------------------------------------------------

/// Explicit structural type containing an unnamed tuple-shaped component.
struct Violation {
    /// Complete structural type receiving the primary diagnostic.
    root_span: Span,
    /// Nested tuple highlighted as the anonymous protocol.
    tuple_span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("this explicit type contains an unnamed tuple shape")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the surrounding container explains storage but cannot communicate what each tuple position represents",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("replace the tuple with a concept-specific record struct and named fields")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            NESTED_TUPLE_TYPES,
            self.root_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.tuple_span,
                    "the nested tuple hides its component roles",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// NestedTupleTypes: Lint pass
// -----------------------------------------------------------------------------

/// Late lint pass that rejects tuple-shaped components hidden inside explicit types.
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
        // Retain only explicit structural types containing a nested tuple.
        let Some(tuple) = ExplicitTupleType::classify(cx, ty) else {
            return;
        };
        if tuple.kind != ExplicitTupleKind::Nested {
            return;
        }

        // Preserve both the containing type and anonymous component as diagnostic evidence.
        Violation {
            root_span: tuple.root_span,
            tuple_span: tuple.tuple_span,
        }
        .emit(cx);
    }
}
