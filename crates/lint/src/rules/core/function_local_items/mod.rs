extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::DefKind;
use rustc_hir::{Item, Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::SpanProvenanceExt;

// -----------------------------------------------------------------------------
// Violation: Function-local item diagnostic
// -----------------------------------------------------------------------------

/// Authored item declared inside an executable body.
struct Violation {
    /// Item node used to honor its local lint level.
    hir_id: rustc_hir::HirId,
    /// Complete declaration receiving the diagnostic.
    span: Span,
    /// Best available authored name for the declaration.
    name: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "item `{}` is declared inside a function body",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "function-local items hide reusable structure inside executable control flow and make ownership harder to discover",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "move the item to module scope or its nearest semantic owner and pass required state explicitly",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            FUNCTION_LOCAL_ITEMS,
            self.hir_id,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this declaration is nested in executable code");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// FunctionLocalItems: Executable-body declaration policy
// -----------------------------------------------------------------------------

/// Late lint pass rejecting authored item statements in executable bodies.
struct FunctionLocalItems;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub FUNCTION_LOCAL_ITEMS,
    Warn,
    "rejects item declarations inside functions, methods, and closures",
    FunctionLocalItems
}

impl FunctionLocalItems {
    /// Returns whether the statement belongs to an executable callable body.
    fn is_callable_body(cx: &LateContext<'_>, statement: &Stmt<'_>) -> bool {
        let owner = cx.tcx.hir_enclosing_body_owner(statement.hir_id);
        matches!(
            cx.tcx.def_kind(owner),
            DefKind::Fn | DefKind::AssocFn | DefKind::Closure
        )
    }

    /// Produces a stable name for an item without requiring kind-specific diagnostics.
    fn name(cx: &LateContext<'_>, item: &Item<'_>) -> String {
        item.kind.ident().map_or_else(
            || cx.tcx.def_path_str(item.owner_id.def_id),
            |ident| ident.name.to_string(),
        )
    }
}

impl LateLintPass<'_> for FunctionLocalItems {
    fn check_stmt(&mut self, cx: &LateContext<'_>, statement: &Stmt<'_>) {
        // Ordinary executable statements contain no nested item declaration.
        let StmtKind::Item(item_id) = statement.kind else {
            return;
        };

        // Items in non-callable bodies are governed by their enclosing declaration scope.
        if !Self::is_callable_body(cx, statement) {
            return;
        }
        let item = cx.tcx.hir_item(item_id);

        // Generated items are not authored local-structure decisions.
        if item.span.in_external_macro(cx.sess().source_map()) || item.span.is_build_generated(cx) {
            return;
        }
        Violation {
            hir_id: item.hir_id(),
            span: item.span,
            name: Self::name(cx, item),
        }
        .emit(cx);
    }
}
