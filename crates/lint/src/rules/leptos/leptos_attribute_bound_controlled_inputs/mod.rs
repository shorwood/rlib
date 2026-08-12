extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{BytePos, Span, Symbol};

use super::utils::reactive_capability::ReactiveCapability;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Attribute-bound control diagnostic
// -----------------------------------------------------------------------------

/// Writable state supplied through an initial-state HTML attribute.
struct Violation {
    /// Attribute call used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored attribute expression highlighted by the diagnostic.
    span: Span,
    /// Form state name used in the remediation.
    attribute: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "writable state is passed through the `{}` HTML attribute",
            self.attribute
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the HTML attribute sets initial state, while the DOM property holds the value after user interaction",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "use `bind:{0}` for two-way state, or `prop:{0}` with an explicit event handler",
            self.attribute
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_ATTRIBUTE_BOUND_CONTROLLED_INPUTS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this sets an attribute rather than the live property");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosAttributeBoundControlledInputs: Form state policy
// -----------------------------------------------------------------------------

/// Late lint pass that keeps reactive form state on live DOM properties.
struct LeptosAttributeBoundControlledInputs;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_ATTRIBUTE_BOUND_CONTROLLED_INPUTS,
    Warn,
    "rejects writable form state supplied through initial-state HTML attributes",
    LeptosAttributeBoundControlledInputs
}

impl LeptosAttributeBoundControlledInputs {
    /// Recovers a raw authored state attribute immediately before a reactive value.
    fn raw_state_attribute(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<Symbol> {
        if !matches!(expression.kind, ExprKind::Path(_)) {
            return None;
        }
        let callsite = expression.span.source_callsite();
        let prefix = Span::with_root_ctxt(
            BytePos(callsite.lo().0.saturating_sub(128)),
            callsite.lo(),
        );
        let source = cx.sess().source_map().span_to_snippet(prefix).ok()?;
        let source = source.trim_end();
        let tag = source
            .rsplit_once('<')?
            .1
            .split_ascii_whitespace()
            .next()?;
        if !matches!(tag, "input" | "select" | "textarea") {
            return None;
        }
        for attribute in ["value", "checked"] {
            let Some(before) = source
                .strip_suffix('=')
                .and_then(|value| value.strip_suffix(attribute))
            else {
                continue;
            };
            if !before.ends_with(':') {
                return Some(Symbol::intern(attribute));
            }
        }
        None
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosAttributeBoundControlledInputs {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let Some(attribute) = Self::raw_state_attribute(cx, expression) else {
            return;
        };
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        let ty = cx.tcx.typeck(owner).expr_ty(expression);
        if !ReactiveCapability::carries_write(cx, owner, ty) {
            return;
        }
        Violation {
            owner: expression.hir_id,
            span: expression.span.source_callsite(),
            attribute,
        }
        .emit(cx);
    }
}
