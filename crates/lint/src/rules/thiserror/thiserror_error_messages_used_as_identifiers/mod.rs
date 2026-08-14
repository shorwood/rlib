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

use super::utils::contracts::ThiserrorContractCatalog;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Display text used as machine identity
// -----------------------------------------------------------------------------

/// Typed error decision made by parsing its presentation string.
struct Violation {
    /// Comparison or string-pattern expression receiving the diagnostic.
    span: Span,
    /// Kind of text-dependent decision explained in the rationale.
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

// -----------------------------------------------------------------------------
// Candidate: Text-dependent error decision evidence
// -----------------------------------------------------------------------------

/// Potential text-based decision retained until its error type is confirmed.
struct Candidate {
    /// Displayed local error type.
    definition: LocalDefId,
    /// Decision expression receiving a later diagnostic.
    span: Span,
    /// Kind of string operation performed.
    operation: &'static str,
}

// -----------------------------------------------------------------------------
// ThiserrorErrorMessagesUsedAsIdentifiers: Typed identity policy
// -----------------------------------------------------------------------------

/// Correlates string decisions with local thiserror contracts.
#[derive(Default)]
struct ThiserrorErrorMessagesUsedAsIdentifiers {
    /// Local derived error contracts.
    catalog: ThiserrorContractCatalog,
    /// String decisions awaiting derive confirmation.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_ERROR_MESSAGES_USED_AS_IDENTIFIERS,
    Warn,
    "finds machine decisions based on thiserror display text",
    ThiserrorErrorMessagesUsedAsIdentifiers::default()
}

impl ThiserrorErrorMessagesUsedAsIdentifiers {
    /// Resolves the local error type formatted by a `to_string` call.
    fn displayed_error(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<LocalDefId> {
        let ExprKind::MethodCall(segment, receiver, _, _) = expression.kind else {
            return None;
        };
        if segment.ident.as_str() == "as_str" {
            let receiver_type = cx.typeck_results().expr_ty(receiver).peel_refs();
            let definition = receiver_type.ty_adt_def()?;
            if cx.tcx.item_name(definition.did()).as_str() == "String"
                && cx.tcx.crate_name(definition.did().krate).as_str() == "alloc"
            {
                return Self::displayed_error(cx, receiver);
            }
            return None;
        }
        if segment.ident.as_str() != "to_string" {
            return None;
        }

        let method = cx
            .typeck_results()
            .type_dependent_def_id(expression.hir_id)?;
        let trait_id = cx.tcx.trait_of_assoc(method)?;
        if cx.tcx.item_name(trait_id).as_str() != "ToString"
            || cx.tcx.crate_name(trait_id.krate).as_str() != "alloc"
        {
            return None;
        }

        cx.typeck_results()
            .expr_ty(receiver)
            .peel_refs()
            .ty_adt_def()?
            .did()
            .as_local()
    }

    /// Returns whether an operand is a literal string rather than domain data.
    const fn is_string_literal(expression: &Expr<'_>) -> bool {
        matches!(expression.kind, ExprKind::Lit(literal)
        if matches!(literal.node, rustc_ast::LitKind::Str(..)))
    }
}
impl LateLintPass<'_> for ThiserrorErrorMessagesUsedAsIdentifiers {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        if expression.span.from_expansion() {
            return;
        }

        let candidate = match expression.kind {
            ExprKind::Binary(operator, left, right)
                if matches!(operator.node, BinOpKind::Eq | BinOpKind::Ne) =>
            {
                if Self::is_string_literal(right) {
                    Self::displayed_error(cx, left)
                        .map(|definition| (definition, "string comparison"))
                } else if Self::is_string_literal(left) {
                    Self::displayed_error(cx, right)
                        .map(|definition| (definition, "string comparison"))
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
                    .is_some_and(|argument| Self::is_string_literal(argument)) =>
            {
                Self::displayed_error(cx, receiver)
                    .map(|definition| (definition, "string-pattern check"))
            }
            _ => None,
        };

        let Some((definition, operation)) = candidate else {
            return;
        };

        self.candidates.push(Candidate {
            definition,
            span: expression.span,
            operation,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if self.catalog.derived_type(candidate.definition).is_none() {
                continue;
            }

            Violation {
                span: candidate.span,
                operation: candidate.operation,
            }
            .emit(cx);
        }
    }
}
