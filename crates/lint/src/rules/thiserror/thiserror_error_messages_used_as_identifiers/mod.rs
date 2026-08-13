extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{BinOpKind, Expr, ExprKind, Item};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::ThiserrorContractCatalog;
use crate::utils::diagnostic::LateViolation;

/// Carries the `Candidate` state used by this analysis.
struct Candidate {
    /// Stores the `definition` value used by this analysis.
    definition: LocalDefId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `operation` value used by this analysis.
    operation: &'static str,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `operation` value used by this analysis.
    operation: &'static str,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("thiserror display text is used as machine identity")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "this {0} makes control flow depend on presentation text instead of the error's typed identity",
            self.operation
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "match the typed error variant or introduce an explicit stable diagnostic code",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            THISERROR_ERROR_MESSAGES_USED_AS_IDENTIFIERS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this decision parses error presentation");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Performs the `displayed_error` step of the lint analysis.
fn displayed_error(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<LocalDefId> {
    // Prepare the values used by this stage.
    let ExprKind::MethodCall(segment, receiver, _, _) = expression.kind else {
        return None;
    };
    if segment.ident.as_str() != "to_string" {
        return None;
    }

    // Prepare the values used by this stage.
    let method = cx
        .typeck_results()
        .type_dependent_def_id(expression.hir_id)?;
    let trait_id = cx.tcx.trait_of_assoc(method)?;
    if cx.tcx.item_name(trait_id).as_str() != "ToString" {
        return None;
    }

    // Perform the next step of the analysis.
    cx.typeck_results()
        .expr_ty(receiver)
        .peel_refs()
        .ty_adt_def()?
        .did()
        .as_local()
}

/// Performs the `is_string_literal` step of the lint analysis.
const fn is_string_literal(expression: &Expr<'_>) -> bool {
    matches!(expression.kind, ExprKind::Lit(literal)
        if matches!(literal.node, rustc_ast::LitKind::Str(..)))
}

#[derive(Default)]
/// Carries the `ThiserrorErrorMessagesUsedAsIdentifiers` state used by this analysis.
struct ThiserrorErrorMessagesUsedAsIdentifiers {
    /// Stores the `catalog` value used by this analysis.
    catalog: ThiserrorContractCatalog,
    /// Stores the `candidates` value used by this analysis.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_ERROR_MESSAGES_USED_AS_IDENTIFIERS,
    Warn,
    "finds machine decisions based on thiserror display text",
    ThiserrorErrorMessagesUsedAsIdentifiers::default()
}

impl LateLintPass<'_> for ThiserrorErrorMessagesUsedAsIdentifiers {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Reject inputs that do not satisfy this stage.
        if expression.span.from_expansion() {
            return;
        }

        // Prepare the values used by this stage.
        let analyze_candidate = match expression.kind {
            ExprKind::Binary(analyze_operator, left, right)
                if matches!(analyze_operator.node, BinOpKind::Eq | BinOpKind::Ne) =>
            {
                if is_string_literal(right) {
                    displayed_error(cx, left).map(|definition| (definition, "string comparison"))
                } else if is_string_literal(left) {
                    displayed_error(cx, right).map(|definition| (definition, "string comparison"))
                } else {
                    None
                }
            }
            ExprKind::MethodCall(segment, receiver, arguments, _)
                if matches!(
                    segment.ident.as_str(),
                    "starts_with" | "ends_with" | "contains"
                ) && arguments
                    .first()
                    .is_some_and(|argument| is_string_literal(argument)) =>
            {
                displayed_error(cx, receiver).map(|definition| (definition, "string-pattern check"))
            }
            _ => None,
        };

        // Prepare the values used by this stage.
        let Some((definition, operation)) = analyze_candidate else {
            return;
        };

        // Update the accumulated analysis state.
        self.candidates.push(Candidate {
            definition,
            span: expression.span,
            operation,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for analyze_candidate in self.candidates.drain(..) {
            // Reject inputs that do not satisfy this stage.
            if self
                .catalog
                .derived_type(analyze_candidate.definition)
                .is_none()
            {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                operation: analyze_candidate.operation,
            }
            .emit(cx);
        }
    }
}
