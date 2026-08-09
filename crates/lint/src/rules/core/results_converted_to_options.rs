extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::result_loss_analysis::{ResolvedResultCall, ResultLossAnalyzer, ResultOperation};

// -----------------------------------------------------------------------------
// Violation: Result to option information loss
// -----------------------------------------------------------------------------

/// Result-to-Option conversion carrying the success and error types needed for remediation.
struct Violation {
    /// Complete `ok` invocation used as the primary diagnostic site.
    span: Span,
    /// Result receiver labeled as the source of erased information.
    result_span: Span,
    /// Value retained when the operation succeeds.
    success_type: String,
    /// Concrete failure collapsed into absence.
    error_type: String,
}

impl Violation {
    /// Captures the concrete types and spans from one resolved result conversion.
    fn from_call(span: Span, call: &ResolvedResultCall<'_, '_>) -> Self {
        let contract = call.contract();
        Self {
            span,
            result_span: call.receiver().span,
            success_type: contract.success_name(),
            error_type: contract.error_name(),
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "this conversion turns `{}` failures into an indistinguishable absence",
            self.error_type
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the resulting `Option<{}>` cannot distinguish an absent value from a failed operation or explain the lost `{}`",
            self.success_type, self.error_type
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "preserve the Result, or match both variants explicitly where failure is intentionally translated into absence",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            RESULTS_CONVERTED_TO_OPTIONS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.result_span,
                    "this Result still carries failure context",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// ResultsConvertedToOptions: Explicit absence boundary policy
// -----------------------------------------------------------------------------

/// Finds standard Result `ok` calls that silently collapse failure into absence.
struct ResultsConvertedToOptions;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds calls to the standard `Result::ok` operation in method or UFCS form. Calls through
    /// type aliases are resolved semantically, while custom methods named `ok`, non-Result
    /// receivers, explicit matches, and macro-generated code remain valid.
    ///
    /// ### Why is this bad?
    ///
    /// `Result::ok` compresses two distinct states into `None`: domain absence and operational
    /// failure. The concise call hides both the erased error type and the location where the
    /// application chose to stop treating failure as failure. Agents can then propagate the
    /// optional value through unrelated code without enough context to recover the original
    /// policy.
    ///
    /// ```rust
    /// # fn load() -> Result<String, std::io::Error> { Ok(String::new()) }
    /// let cached = load().ok();
    /// ```
    ///
    /// Keep the Result intact, or make the translation reviewable in authored control flow:
    ///
    /// ```rust
    /// # fn load() -> Result<String, std::io::Error> { Ok(String::new()) }
    /// let cached = match load() {
    ///     Ok(value) => Some(value),
    ///     Err(error) => {
    ///         eprintln!("cache unavailable: {error}");
    ///         None
    ///     }
    /// };
    /// ```
    ///
    /// No automatic fix is offered because propagation, reporting, and intentional absence have
    /// different types and ownership requirements.
    pub RESULTS_CONVERTED_TO_OPTIONS,
    Warn,
    "rejects Result failures silently converted into Option absence",
    ResultsConvertedToOptions
}

impl LateLintPass<'_> for ResultsConvertedToOptions {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Resolve only authored calls to the inherent result conversion operation.
        if expression.span.from_expansion() {
            return;
        }
        let analyzer = ResultLossAnalyzer::for_context(cx);
        let Some(call) = analyzer.call(expression, ResultOperation::Ok) else {
            return;
        };
        if !call.arguments().is_empty() {
            return;
        }

        Violation::from_call(expression.span, &call).emit(cx);
    }
}
