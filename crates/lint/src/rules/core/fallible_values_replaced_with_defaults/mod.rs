extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use strum::EnumMessage;

use crate::utils::diagnostic::LateViolation;
use crate::utils::result_loss_analysis::{ResolvedResultCall, ResultLossAnalyzer, ResultOperation};

// -----------------------------------------------------------------------------
// DefaultFallback: Recognized default producing operation
// -----------------------------------------------------------------------------

/// Standard Result operation that replaces its error branch with a default success value.
#[derive(EnumMessage)]
enum DefaultFallback {
    /// Dedicated `unwrap_or_default` operation.
    #[strum(message = "`unwrap_or_default`")]
    Dedicated,
    /// Eager default passed to `unwrap_or`.
    #[strum(message = "`unwrap_or` with a default value")]
    Eager,
    /// Error-ignoring default closure passed to `unwrap_or_else`.
    #[strum(message = "`unwrap_or_else` with an error-ignoring default closure")]
    Lazy,
}

impl DefaultFallback {
    /// Describes the fallback syntax responsible for hiding the failure.
    fn description(&self) -> &'static str {
        self.get_message()
            .expect("every fallback variant has a message")
    }
}

// -----------------------------------------------------------------------------
// DefaultFinding: Classified result fallback
// -----------------------------------------------------------------------------

/// Resolved result call paired with the default-producing behavior it selected.
struct DefaultFinding<'hir, 'tcx> {
    /// Standard result operation consuming the failure branch.
    call: ResolvedResultCall<'hir, 'tcx>,
    /// Exact default-producing syntax selected by the call.
    fallback: DefaultFallback,
}

// -----------------------------------------------------------------------------
// Violation: Defaulted failure context
// -----------------------------------------------------------------------------

/// Default-producing Result fallback with the precise erased failure contract.
struct Violation {
    /// Complete fallback call used as the primary diagnostic location.
    span: Span,
    /// Result receiver labeled as the source of the erased error.
    result_span: Span,
    /// Success type whose default is substituted for failure.
    success_type: String,
    /// Concrete error type hidden by the fallback.
    error_type: String,
    /// Recognized fallback form used to explain the operation.
    fallback: DefaultFallback,
}

impl Violation {
    /// Captures one classified fallback and its concrete result contract.
    fn from_finding(span: Span, finding: DefaultFinding<'_, '_>) -> Self {
        // Preserve the concrete success and error identities before moving the finding.
        let contract = finding.call.contract();
        let result_span = finding.call.receiver().span;
        let success_type = contract.success_name();
        let error_type = contract.error_name();

        // Build the complete diagnostic context around the classified fallback.
        Self {
            span,
            result_span,
            success_type,
            error_type,
            fallback: finding.fallback,
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "this {} replaces `{}` failures with `{}`'s default value",
            self.fallback.description(),
            self.error_type,
            self.success_type
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "callers receive an ordinary `{}` and cannot tell whether it was produced successfully or substituted after `{}` failed",
            self.success_type, self.error_type
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "propagate or handle the error; when a default is deliberate, expose that policy in an explicit match that can inspect the failure",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            FALLIBLE_VALUES_REPLACED_WITH_DEFAULTS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.result_span,
                    "this Result carries the failure being hidden",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// FallibleValuesReplacedWithDefaults: Failure visibility policy
// -----------------------------------------------------------------------------

/// Finds standard Result fallbacks that replace errors with unmarked default values.
struct FallibleValuesReplacedWithDefaults;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub FALLIBLE_VALUES_REPLACED_WITH_DEFAULTS,
    Warn,
    "rejects Result failures replaced with unmarked default values",
    FallibleValuesReplacedWithDefaults
}

impl LateLintPass<'_> for FallibleValuesReplacedWithDefaults {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Classify only authored default producing operations on standard result.
        if expression.span.from_expansion() {
            return;
        }
        let Some(finding) = Self::classify(cx, expression) else {
            return;
        };

        Violation::from_finding(expression.span, finding).emit(cx);
    }
}

impl FallibleValuesReplacedWithDefaults {
    /// Classifies the supported standard default-producing Result operations.
    fn classify<'hir, 'tcx>(
        cx: &LateContext<'tcx>,
        expression: &'hir Expr<'hir>,
    ) -> Option<DefaultFinding<'hir, 'tcx>> {
        // Bind every supported operation to one semantic result analyzer.
        let analyzer = ResultLossAnalyzer::for_context(cx);

        // Option defaults are absence conversions owned by `results_converted_to_options`.
        if analyzer.is_option_value(expression) {
            return None;
        }

        // Recognize the dedicated default-producing operation first.
        if let Some(call) = analyzer.call(expression, ResultOperation::UnwrapOrDefault)
            && call.arguments().is_empty()
        {
            return Some(DefaultFinding {
                call,
                fallback: DefaultFallback::Dedicated,
            });
        }

        // Recognize an eagerly constructed standard default value.
        if let Some(call) = analyzer.call(expression, ResultOperation::UnwrapOr)
            && let [fallback] = call.arguments()
            && analyzer.is_default_value(fallback)
        {
            return Some(DefaultFinding {
                call,
                fallback: DefaultFallback::Eager,
            });
        }

        // Recognize a direct error-ignoring closure that yields a default.
        let call = analyzer.call(expression, ResultOperation::UnwrapOrElse)?;
        let [fallback] = call.arguments() else {
            return None;
        };

        // Require the fallback to ignore its error and yield only a default value.
        analyzer
            .is_defaulting_closure(fallback)
            .then_some(DefaultFinding {
                call,
                fallback: DefaultFallback::Lazy,
            })
    }
}
