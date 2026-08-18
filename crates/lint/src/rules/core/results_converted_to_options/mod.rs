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
// OptionConversion: Recognized failure to absence operation
// -----------------------------------------------------------------------------

/// Standard Result operation that directly replaces failure with Option absence.
#[derive(EnumMessage)]
enum OptionConversion {
    /// Dedicated Result-to-Option conversion.
    #[strum(message = "`ok` conversion")]
    Ok,
    /// Eager absence supplied to `map_or`.
    #[strum(message = "`map_or` absence fallback")]
    MapOr,
    /// Error-ignoring absence closure supplied to `map_or_else`.
    #[strum(message = "`map_or_else` absence fallback")]
    MapOrElse,
    /// Eager absence supplied to `unwrap_or`.
    #[strum(message = "`unwrap_or` absence fallback")]
    UnwrapOr,
    /// Error-ignoring absence closure supplied to `unwrap_or_else`.
    #[strum(message = "`unwrap_or_else` absence fallback")]
    UnwrapOrElse,
    /// Option's default absence supplied by `unwrap_or_default`.
    #[strum(message = "`unwrap_or_default` Option fallback")]
    UnwrapOrDefault,
}

impl OptionConversion {
    /// Describes the precise syntax collapsing failure into absence.
    fn description(&self) -> &'static str {
        self.get_message()
            .expect("every option conversion variant has a message")
    }
}

// -----------------------------------------------------------------------------
// OptionFinding: Classified result conversion
// -----------------------------------------------------------------------------

/// Resolved result call paired with the failure-to-absence operation it selected.
struct OptionFinding<'hir, 'tcx> {
    /// Standard Result operation consuming the failure branch.
    call: ResolvedResultCall<'hir, 'tcx>,
    /// Exact absence-producing operation selected by the call.
    conversion: OptionConversion,
}

// -----------------------------------------------------------------------------
// Violation: Result to option information loss
// -----------------------------------------------------------------------------

/// Result-to-Option conversion carrying the success and error types needed for remediation.
struct Violation {
    /// Complete `ok` invocation used as the primary diagnostic site.
    span: Span,
    /// Result receiver labeled as the source of erased information.
    result_span: Span,
    /// Actual Option type produced by the complete conversion expression.
    option_type: String,
    /// Concrete failure collapsed into absence.
    error_type: String,
    /// Recognized operation used to tailor the diagnostic.
    conversion: OptionConversion,
}

impl Violation {
    /// Captures the concrete types and spans from one resolved result conversion.
    fn from_finding(
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
        finding: OptionFinding<'_, '_>,
    ) -> Self {
        // Preserve the concrete result contract before moving its classified finding.
        let contract = finding.call.contract();

        // Assemble the complete diagnostic context around the classified conversion.
        Self {
            span: expression.span,
            result_span: finding.call.receiver().span,
            option_type: cx.typeck_results().expr_ty(expression).to_string(),
            error_type: contract.error_name(),
            conversion: finding.conversion,
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "this {} turns `{}` failures into an indistinguishable absence",
            self.conversion.description(),
            self.error_type,
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the resulting `{}` cannot distinguish an absent value from a failed operation or explain the lost `{}`",
            self.option_type, self.error_type
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

/// Finds standard Result operations that silently collapse failure into absence.
struct ResultsConvertedToOptions;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub RESULTS_CONVERTED_TO_OPTIONS,
    Warn,
    "rejects Result failures silently converted into Option absence",
    ResultsConvertedToOptions
}

impl LateLintPass<'_> for ResultsConvertedToOptions {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Resolve only authored calls to inherent result conversion operations.
        if expression.span.from_expansion() {
            return;
        }

        // Expressions outside supported result-to-option operations preserve no failure evidence here.
        let Some(finding) = Self::classify(cx, expression) else {
            return;
        };

        Violation::from_finding(cx, expression, finding).emit(cx);
    }
}

impl ResultsConvertedToOptions {
    /// Classifies every direct standard Result operation that yields Option absence on failure.
    fn classify<'hir, 'tcx>(
        cx: &LateContext<'tcx>,
        expression: &'hir Expr<'hir>,
    ) -> Option<OptionFinding<'hir, 'tcx>> {
        // Bind every supported operation to one semantic result analyzer.
        let analyzer = ResultLossAnalyzer::for_context(cx);

        // Recognize the dedicated Result-to-Option conversion.
        if let Some(call) = analyzer.call(expression, ResultOperation::Ok)
            && call.arguments().is_empty()
        {
            return Some(OptionFinding {
                call,
                conversion: OptionConversion::Ok,
            });
        }

        // Recognize eager absence supplied while mapping a successful value.
        if let Some(call) = analyzer.call(expression, ResultOperation::MapOr)
            && let [fallback, _mapper] = call.arguments()
            && analyzer.is_option_value(expression)
            && analyzer.is_option_absence(fallback)
        {
            return Some(OptionFinding {
                call,
                conversion: OptionConversion::MapOr,
            });
        }

        // Recognize a side-effect-free absence closure used while mapping success.
        if let Some(call) = analyzer.call(expression, ResultOperation::MapOrElse)
            && let [fallback, _mapper] = call.arguments()
            && analyzer.is_option_value(expression)
            && analyzer.is_option_absence_closure(fallback)
        {
            return Some(OptionFinding {
                call,
                conversion: OptionConversion::MapOrElse,
            });
        }

        // Recognize eager absence replacing a failed Option-producing operation.
        if let Some(call) = analyzer.call(expression, ResultOperation::UnwrapOr)
            && let [fallback] = call.arguments()
            && analyzer.is_option_value(expression)
            && analyzer.is_option_absence(fallback)
        {
            return Some(OptionFinding {
                call,
                conversion: OptionConversion::UnwrapOr,
            });
        }

        // Recognize a side effect free absence closure replacing a failed option operation.
        if let Some(call) = analyzer.call(expression, ResultOperation::UnwrapOrElse)
            && let [fallback] = call.arguments()
            && analyzer.is_option_value(expression)
            && analyzer.is_option_absence_closure(fallback)
        {
            return Some(OptionFinding {
                call,
                conversion: OptionConversion::UnwrapOrElse,
            });
        }

        // Recognize option's dedicated default, which is always absence.
        let call = analyzer.call(expression, ResultOperation::UnwrapOrDefault)?;
        (call.arguments().is_empty() && analyzer.is_option_value(expression)).then_some(
            OptionFinding {
                call,
                conversion: OptionConversion::UnwrapOrDefault,
            },
        )
    }
}
