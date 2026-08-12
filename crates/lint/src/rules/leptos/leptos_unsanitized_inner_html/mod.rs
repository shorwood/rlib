extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::LitKind;
use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Unsanitized inner html diagnostic
// -----------------------------------------------------------------------------

/// Runtime text passed directly to Leptos's raw HTML rendering boundary.
struct Violation {
    /// Attribute expression used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored raw value highlighted by the diagnostic.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("runtime text is passed directly to `inner_html`")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "Leptos does not escape `inner_html`, so an ordinary string carries no evidence that its markup has been audited or sanitized",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "introduce a trusted HTML wrapper and confine any lint allowance to the component that unwraps it",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_UNSANITIZED_INNER_HTML,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this value has an ordinary textual type");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosUnsanitizedInnerHtml: Raw markup boundary policy
// -----------------------------------------------------------------------------

/// Late lint pass that requires an explicit trust boundary around dynamic raw markup.
struct LeptosUnsanitizedInnerHtml;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_UNSANITIZED_INNER_HTML,
    Warn,
    "rejects ordinary runtime strings passed directly to Leptos inner_html",
    LeptosUnsanitizedInnerHtml
}

impl LeptosUnsanitizedInnerHtml {
    /// Returns whether the selected method is Leptos's semantic inner HTML attribute operation.
    fn is_inner_html(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        let Some(method) = cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };
        cx.tcx.crate_name(method.krate).as_str() == "tachys"
            && cx.tcx.item_name(method).as_str() == "inner_html"
            && cx
                .tcx
                .trait_of_assoc(method)
                .is_some_and(|trait_id| cx.tcx.item_name(trait_id).as_str() == "InnerHtmlAttribute")
    }

    /// Returns whether a resolved value is an ordinary owned or borrowed string.
    fn is_raw_text(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
        let ty = ty.peel_refs();
        if matches!(ty.kind(), ty::Str) {
            return true;
        }
        let ty::Adt(definition, _) = ty.kind() else {
            return false;
        };
        cx.tcx.crate_name(definition.did().krate).as_str() == "alloc"
            && cx.tcx.item_name(definition.did()).as_str() == "String"
    }

    /// Accepts static authored markup whose complete content is visible at the call site.
    fn is_string_literal(expression: &Expr<'_>) -> bool {
        matches!(expression.kind, ExprKind::Lit(literal) if matches!(literal.node, LitKind::Str(..)))
    }

    /// Peels the dynamic attribute conversion inserted by `view!`.
    fn authored_value<'tcx>(
        cx: &LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
    ) -> &'tcx Expr<'tcx> {
        let ExprKind::Call(callee, [value]) = expression.kind else {
            return expression;
        };
        let ExprKind::Path(path) = callee.kind else {
            return expression;
        };
        let Res::Def(_, method) = cx.qpath_res(&path, callee.hir_id) else {
            return expression;
        };
        let is_conversion = cx.tcx.item_name(method).as_str() == "into_attribute_value"
            && cx.tcx.trait_of_assoc(method).is_some_and(|trait_id| {
                cx.tcx.item_name(trait_id).as_str() == "IntoAttributeValue"
            });
        if is_conversion { value } else { expression }
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosUnsanitizedInnerHtml {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let ExprKind::MethodCall(_, _, arguments, _) = expression.kind else {
            return;
        };
        let [value] = arguments else {
            return;
        };
        let value = Self::authored_value(cx, value);
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        if !Self::is_inner_html(cx, expression)
            || Self::is_string_literal(value)
            || !Self::is_raw_text(cx, cx.tcx.typeck(owner).expr_ty(value))
        {
            return;
        }
        Violation {
            owner: expression.hir_id,
            span: value.span,
        }
        .emit(cx);
    }
}
