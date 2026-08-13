extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use convert_case::{Case, Casing};
use rustc_errors::DiagDecorator;
use rustc_hir::def::{CtorOf, DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

struct Violation {
    span: Span,
    method: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "derive_more-generated `{}` is called without an established variant",
            self.method
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("the generated accessor panics whenever the receiver has another variant")
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "pattern-match the receiver or use a derive_more `TryUnwrap` accessor for a fallible extraction",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_PANIC_PRONE_DERIVED_VARIANT_ACCESSORS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this call can panic on another variant");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct DeriveMorePanicProneDerivedVariantAccessors;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_PANIC_PRONE_DERIVED_VARIANT_ACCESSORS,
    Warn,
    "finds unchecked calls to derive_more-generated variant unwrap accessors",
    DeriveMorePanicProneDerivedVariantAccessors
}

impl LateLintPass<'_> for DeriveMorePanicProneDerivedVariantAccessors {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        let ExprKind::MethodCall(segment, receiver, arguments, _) = expression.kind else {
            return;
        };
        if !arguments.is_empty() || !segment.ident.name.as_str().starts_with("unwrap_") {
            return;
        }
        let Some(target) = cx
            .tcx
            .typeck(expression.hir_id.owner.def_id)
            .type_dependent_def_id(expression.hir_id)
        else {
            return;
        };
        if !is_derive_more_unwrap(cx, target) {
            return;
        }
        let method = segment.ident.name.to_string();
        if receiver_constructs_expected_variant(cx, receiver, &method) {
            return;
        }
        Violation {
            span: expression.span,
            method,
        }
        .emit(cx);
    }
}

fn is_derive_more_unwrap(cx: &LateContext<'_>, target: rustc_hir::def_id::DefId) -> bool {
    cx.tcx.def_span(target).macro_backtrace().any(|expansion| {
        expansion.macro_def_id.is_some_and(|definition| {
            cx.tcx.crate_name(definition.krate).as_str() == "derive_more_impl"
                && cx.tcx.item_name(definition).as_str() == "Unwrap"
        })
    })
}

fn receiver_constructs_expected_variant(
    cx: &LateContext<'_>,
    receiver: &Expr<'_>,
    method: &str,
) -> bool {
    let ExprKind::Call(constructor, _) = receiver.kind else {
        return false;
    };
    let ExprKind::Path(path) = constructor.kind else {
        return false;
    };
    let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
        cx.qpath_res(&path, constructor.hir_id)
    else {
        return false;
    };
    let variant = cx.tcx.parent(constructor);
    let expected = format!(
        "unwrap_{}",
        cx.tcx.item_name(variant).as_str().to_case(Case::Snake)
    );
    method == expected || method == format!("{expected}_ref") || method == format!("{expected}_mut")
}
