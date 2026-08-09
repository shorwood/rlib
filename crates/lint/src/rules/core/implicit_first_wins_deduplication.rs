extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::{Span, sym};

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Implicit representative selection
// -----------------------------------------------------------------------------

/// Set insertion predicate that silently chooses the first value for each key.
struct Violation {
    /// `HashSet::insert` call acting as the filter predicate.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("this filter silently keeps the first value for each key")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "later values are discarded without exposing whether they should replace, merge with, or conflict with the first value",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("collect by key and make the collision or merge policy explicit")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            IMPLICIT_FIRST_WINS_DEDUPLICATION,
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
// ImplicitFirstWinsDeduplication: Lint pass
// -----------------------------------------------------------------------------

/// Recognizes set insertion used directly as an iterator predicate.
struct ImplicitFirstWinsDeduplication;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds iterator filters that deduplicate values by returning the result of
    /// `HashSet::insert` directly. Other uses of `HashSet::insert` and ordinary predicates remain
    /// valid.
    ///
    /// ### Why is this bad?
    ///
    /// This compact idiom silently selects the first value for every key. That policy is often
    /// harmless for identical values, but it loses information when later records contain richer
    /// state. An explicit merge makes representative selection reviewable.
    ///
    /// For example, this always retains the first participant encountered:
    ///
    /// ```rust
    /// # use std::collections::HashSet;
    /// # struct Participant { id: u32 }
    /// # let participants = Vec::<Participant>::new();
    /// let mut seen = HashSet::new();
    /// let unique = participants.iter().filter(|item| seen.insert(item.id));
    /// # let _ = unique;
    /// ```
    ///
    /// Collect by key and state how collisions are resolved instead:
    ///
    /// ```rust
    /// # use std::collections::HashMap;
    /// # struct Participant { id: u32 }
    /// # let participants = Vec::<Participant>::new();
    /// let mut unique = HashMap::new();
    /// for item in participants {
    ///     unique.entry(item.id).or_insert(item);
    /// }
    /// ```
    pub IMPLICIT_FIRST_WINS_DEDUPLICATION,
    Warn,
    "rejects HashSet insertion used as an iterator filter predicate",
    ImplicitFirstWinsDeduplication
}

impl LateLintPass<'_> for ImplicitFirstWinsDeduplication {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Abort early if this expression is a macro expansion or not a method call on an iterator.
        if expression.span.from_expansion() {
            return;
        }

        // Abort early if this expression is not a `filter` method call with a single predicate argument.
        let ExprKind::MethodCall(filter, _, [predicate], _) = expression.kind else {
            return;
        };

        // Abort early if the method name is not `filter`.
        if filter.ident.name.as_str() != "filter" {
            return;
        }

        // Abort early if the predicate is not a closure that returns the result of a `HashSet::insert` call.
        let ExprKind::Closure(closure) = predicate.kind else {
            return;
        };

        // Abort early if the closure body is not a method call on a `HashSet` receiver.
        let body = cx.tcx.hir_body(closure.body).value.peel_blocks();
        let ExprKind::MethodCall(insert, receiver, _, _) = body.kind else {
            return;
        };

        // Abort early if the method name is not `insert` or the receiver is not a `HashSet`.
        if insert.ident.name.as_str() != "insert" || !Self::is_hash_set(cx, receiver) {
            return;
        }

        Violation { span: body.span }.emit(cx);
    }
}

impl ImplicitFirstWinsDeduplication {
    /// Returns whether the receiver is the standard library's `HashSet` type.
    fn is_hash_set(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        let expression_type = cx.typeck_results().expr_ty(expression);
        let referent_type = expression_type.peel_refs();
        let ty::Adt(definition, _) = referent_type.kind() else {
            return false;
        };
        cx.tcx.is_diagnostic_item(sym::HashSet, definition.did())
    }
}
