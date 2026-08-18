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

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_UNSANITIZED_INNER_HTML,
    Warn,
    "rejects ordinary runtime strings passed directly to Leptos inner_html",
    LeptosUnsanitizedInnerHtml
}

impl LeptosUnsanitizedInnerHtml {
    /// Returns whether the selected method is Leptos's semantic inner HTML attribute operation.
    fn is_inner_html(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        // Accept only standard conversion methods that preserve already-static markup.
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);

        // Unresolved methods cannot establish the semantic inner HTML operation.
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

        // Borrowed string slices are ordinary untrusted runtime text.
        if matches!(ty.kind(), ty::Str) {
            return true;
        }

        // Nonalgebraic values cannot be the owned string type.
        let ty::Adt(definition, _) = ty.kind() else {
            return false;
        };
        cx.tcx.crate_name(definition.did().krate).as_str() == "alloc"
            && cx.tcx.item_name(definition.did()).as_str() == "String"
    }

    /// Accepts static authored markup whose complete content is visible at the call site.
    fn is_static_markup(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        // A literal string exposes its complete markup for local review.
        if matches!(expression.kind, ExprKind::Lit(literal) if matches!(literal.node, LitKind::Str(..)))
        {
            return true;
        }

        // Peel one zero-argument conversion applied to recursively static markup.
        // Other expression shapes may compute runtime text.
        let ExprKind::MethodCall(_, input, [], _) = expression.kind else {
            return false;
        };

        // Converting dynamic input does not make its markup statically auditable.
        if !Self::is_static_markup(cx, input) {
            return false;
        }

        // Resolve whether the conversion is a standard owned-string operation.
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);

        // Unresolved conversions cannot prove that the static text is preserved.
        let Some(method) = cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };

        // Accept conversions resolved to the standard allocation traits only.
        // These conversions preserve the already-audited literal content exactly.
        if let Some(trait_id) = cx.tcx.trait_of_assoc(method)
            && matches!(
                (
                    cx.tcx.crate_name(trait_id.krate).as_str(),
                    cx.tcx.item_name(trait_id).as_str(),
                    cx.tcx.item_name(method).as_str(),
                ),
                ("alloc", "ToOwned", "to_owned") | ("alloc", "ToString", "to_string")
            )
        {
            return true;
        }

        false
    }

    /// Peels the dynamic attribute conversion inserted by `view!`.
    fn authored_value<'tcx>(
        cx: &LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
    ) -> &'tcx Expr<'tcx> {
        // Only a single-argument call can be the macro's attribute-value conversion.
        let ExprKind::Call(callee, [value]) = expression.kind else {
            return expression;
        };

        // Indirect callees cannot identify the inserted conversion operation.
        let ExprKind::Path(path) = callee.kind else {
            return expression;
        };

        // Unresolved paths cannot be matched to the attribute conversion trait.
        let Res::Def(_, method) = cx.qpath_res(&path, callee.hir_id) else {
            return expression;
        };
        let is_conversion = cx.tcx.item_name(method).as_str() == "into_attribute_value"
            && cx.tcx.trait_of_assoc(method).is_some_and(|trait_id| {
                cx.tcx.item_name(trait_id).as_str() == "IntoAttributeValue"
            });

        if is_conversion { value } else { expression }
    }

    /// Follows transparent macro conversions and the output of a reactive attribute closure.
    fn authored_output<'tcx>(
        cx: &LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
    ) -> &'tcx Expr<'tcx> {
        let expression = Self::authored_value(cx, expression);
        match expression.kind {
            ExprKind::Closure(closure) => {
                Self::authored_output(cx, cx.tcx.hir_body(closure.body).value)
            }
            ExprKind::Block(block, _) => block
                .expr
                .map_or(expression, |tail| Self::authored_output(cx, tail)),
            _ => expression,
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosUnsanitizedInnerHtml {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Raw HTML assignment must be expressed as a method call.
        let ExprKind::MethodCall(_, _, arguments, _) = expression.kind else {
            return;
        };

        // The semantic inner HTML operation accepts exactly one raw value.
        let [value] = arguments else {
            return;
        };
        let value = Self::authored_output(cx, value);

        let owner = cx.tcx.hir_enclosing_body_owner(value.hir_id);

        // Emit only for dynamic ordinary text reaching the semantic raw HTML boundary.
        if !Self::is_inner_html(cx, expression)
            || Self::is_static_markup(cx, value)
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
