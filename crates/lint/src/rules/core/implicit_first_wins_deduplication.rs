extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::sym;

// -----------------------------------------------------------------------------
// ImplicitFirstWinsDeduplication: Representative selection policy
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
        // Recognize an authored iterator filter call.
        if expression.span.from_expansion() {
            return;
        }
        let ExprKind::MethodCall(filter, _, [predicate], _) = expression.kind else {
            return;
        };
        if filter.ident.name.as_str() != "filter" {
            return;
        }

        // Inspect the filter closure for a direct set insertion predicate.
        let ExprKind::Closure(closure) = predicate.kind else {
            return;
        };
        let body = cx.tcx.hir_body(closure.body).value.peel_blocks();
        let ExprKind::MethodCall(insert, receiver, _, _) = body.kind else {
            return;
        };
        if insert.ident.name.as_str() != "insert" || !Self::is_hash_set(cx, receiver) {
            return;
        }

        // Explain the implicit collision policy at the insertion predicate.
        cx.emit_span_lint(
            IMPLICIT_FIRST_WINS_DEDUPLICATION,
            body.span,
            DiagDecorator(|diag| {
                diag.primary_message("this filter silently keeps the first value for each key");
                diag.help("collect by key and make the collision or merge policy explicit");
            }),
        );
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
