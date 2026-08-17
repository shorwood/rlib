extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Expr, ExprKind, HirId, PatKind, Stmt, StmtKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use strum::EnumMessage;

use crate::utils::diagnostic::LateViolation;
use crate::utils::result_loss_analysis::{ResultContract, ResultLossAnalyzer};

// -----------------------------------------------------------------------------
// ResultDiscard: Explicit failure erasure syntax
// -----------------------------------------------------------------------------

/// Authored syntax that consumes a complete Result without inspecting either branch.
#[derive(EnumMessage)]
enum ResultDiscard {
    /// Wildcard `let` binding that suppresses the Result's must-use contract.
    #[strum(message = "wildcard binding")]
    WildcardBinding,
    /// Underscore-prefixed binding that advertises the Result will not be used normally.
    #[strum(message = "underscore-prefixed binding")]
    UnderscoreBinding,
    /// Explicit call to standard `drop` with the Result as its argument.
    #[strum(message = "explicit `drop` call")]
    DropCall,
}

impl ResultDiscard {
    /// Describes the concrete syntax responsible for erasing the Result.
    fn description(&self) -> &'static str {
        self.get_message()
            .expect("every discard variant has a message")
    }
}

// -----------------------------------------------------------------------------
// Violation: Discarded result context
// -----------------------------------------------------------------------------

/// Result disposal carrying the precise error information that becomes unreachable.
struct Violation {
    /// Complete discard expression or initializer used as the primary location.
    span: Span,
    /// Result value labeled as the source of the erased failure information.
    result_span: Span,
    /// Concrete error type callers can no longer inspect after the discard.
    error_type: String,
    /// Authored disposal form used to tailor the primary explanation.
    discard: ResultDiscard,
}

impl Violation {
    /// Builds a complete violation from one resolved Result contract.
    fn from_contract(
        span: Span,
        result_span: Span,
        contract: ResultContract<'_>,
        discard: ResultDiscard,
    ) -> Self {
        Self {
            span,
            result_span,
            error_type: contract.error_name(),
            discard,
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "this {} discards a `Result` without handling `{}`",
            self.discard.description(),
            self.error_type
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "discarding the complete value erases whether the operation failed and makes `{}` unavailable for recovery, reporting, or context",
            self.error_type
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "propagate the Result, inspect its error branch, or translate the failure at an explicit match boundary",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DISCARDED_RESULTS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.result_span, "this Result is discarded in full");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// DiscardedResults: Failure preservation policy
// -----------------------------------------------------------------------------

/// Finds authored Result values consumed without an explicit success or error policy.
struct DiscardedResults;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DISCARDED_RESULTS,
    Warn,
    "rejects Result values discarded without an explicit failure policy",
    DiscardedResults
}

impl LateLintPass<'_> for DiscardedResults {
    fn check_stmt(&mut self, cx: &LateContext<'_>, statement: &Stmt<'_>) {
        let Some(violation) = Self::binding_violation(cx, statement) else {
            return;
        };
        violation.emit(cx);
    }

    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        let Some(violation) = Self::drop_violation(cx, expression) else {
            return;
        };
        violation.emit(cx);
    }
}

impl DiscardedResults {
    /// Returns whether an underscore-prefixed binding is subsequently read in its body.
    fn binding_is_used(cx: &LateContext<'_>, expression: &Expr<'_>, binding: HirId) -> bool {
        struct BindingUse<'analysis, 'tcx> {
            cx: &'analysis LateContext<'tcx>,
            binding: HirId,
            is_found: bool,
        }

        impl<'tcx> Visitor<'tcx> for BindingUse<'_, 'tcx> {
            fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
                if self.is_found {
                    return;
                }
                if let ExprKind::Path(path) = expression.kind
                    && matches!(
                        self.cx.qpath_res(&path, expression.hir_id),
                        Res::Local(binding) if binding == self.binding
                    )
                {
                    self.is_found = true;
                    return;
                }
                intravisit::walk_expr(self, expression);
            }

            fn visit_nested_body(&mut self, body: rustc_hir::BodyId) {
                self.visit_body(self.cx.tcx.hir_body(body));
            }
        }

        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        let mut usage = BindingUse {
            cx,
            binding,
            is_found: false,
        };
        usage.visit_body(cx.tcx.hir_body_owned_by(owner));
        usage.is_found
    }

    /// Classifies one authored wildcard or underscore-prefixed binding of a standard result.
    fn binding_violation(cx: &LateContext<'_>, statement: &Stmt<'_>) -> Option<Violation> {
        // Resolve the authored binding syntax and its initializer.
        if statement.span.from_expansion() {
            return None;
        }
        let StmtKind::Let(local) = statement.kind else {
            return None;
        };

        // Classify only bindings whose spelling explicitly advertises disposal.
        let (discard, binding) = match local.pat.kind {
            PatKind::Wild => (ResultDiscard::WildcardBinding, None),
            PatKind::Binding(_, binding, ident, None) if ident.name.as_str().starts_with('_') => {
                (ResultDiscard::UnderscoreBinding, Some(binding))
            }
            _ => return None,
        };
        let initializer = local.init?;
        if binding.is_some_and(|binding| Self::binding_is_used(cx, initializer, binding)) {
            return None;
        }

        // Require the initializer itself to carry a standard result contract.
        let analyzer = ResultLossAnalyzer::for_context(cx);
        let contract = analyzer.contract(initializer)?;

        // Capture the authored discard and its precise failure context.
        Some(Violation::from_contract(
            statement.span,
            initializer.span,
            contract,
            discard,
        ))
    }

    /// Classifies one authored standard drop call consuming a result.
    fn drop_violation(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<Violation> {
        // Resolve only authored calls to standard drop with one result argument.
        if expression.span.from_expansion() {
            return None;
        }
        let rustc_hir::ExprKind::Call(_, [result]) = expression.kind else {
            return None;
        };

        // Require the consumed argument to carry a standard result contract.
        let analyzer = ResultLossAnalyzer::for_context(cx);
        let contract = analyzer.dropped_result(expression)?;

        // Capture the authored discard and its precise failure context.
        Some(Violation::from_contract(
            expression.span,
            result.span,
            contract,
            ResultDiscard::DropCall,
        ))
    }
}
