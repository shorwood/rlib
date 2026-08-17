extern crate rustc_errors;
extern crate rustc_hir;

use std::borrow::Cow;
use std::mem::take;

use rustc_errors::DiagDecorator;
use rustc_hir::{AmbigArg, Expr, ImplItem, Item, Ty};
use rustc_lint::{LateContext, LateLintPass};

use crate::utils::callable_generic_analysis::CallableGenericAnalyzer;
use crate::utils::diagnostic::LateViolation;
use crate::utils::generic_abstraction_analysis::{
    GenericAbstractionAnalyzer, GenericAbstractionFinding,
};

// -----------------------------------------------------------------------------
// Violation: Unconsumed generic abstraction diagnostic
// -----------------------------------------------------------------------------

/// One generic parameter with only one observed concrete substitution.
struct Violation(
    /// Complete declaration and use evidence.
    GenericAbstractionFinding,
);

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "type parameter `{}` on {} `{}` has only one concrete substitution",
            self.0.parameter.name, self.0.declaration.kind, self.0.declaration.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        // Callable parameters need evidence phrased around resolved call sites.
        if self.0.declaration.is_callable {
            return Cow::Owned(format!(
                "every active authored call resolves `{}` to `{}`; no generic forwarding, callable escape, unnameable type, or second concrete substitution demonstrates that this parameter is consumed polymorphically",
                self.0.parameter.name, self.0.concrete_type
            ));
        }
        Cow::Owned(format!(
            "every active authored use supplies `{}`; no generic forwarding, inferred argument, projection, opaque type, trait object, alias, or second concrete substitution demonstrates that this parameter is consumed polymorphically",
            self.0.concrete_type
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        // Callable parameters require signature-specialization guidance.
        if self.0.declaration.is_callable {
            return Cow::Owned(format!(
                "specialize `{}` as `{}` in the signature and remove the generic parameter until another substitution is required",
                self.0.parameter.name, self.0.concrete_type
            ));
        }
        Cow::Owned(format!(
            "replace `{}` with `{}` until a second real substitution is required",
            self.0.parameter.name, self.0.concrete_type
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Keep representative substitutions adjacent to the parameter-level recommendation.
        cx.tcx.emit_node_span_lint(
            UNCONSUMED_GENERIC_ABSTRACTIONS,
            self.0.declaration.hir_id,
            self.0.parameter.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                for use_span in self.0.use_spans {
                    diag.span_label(
                        use_span,
                        if self.0.declaration.is_callable {
                            format!("`{}` resolves here", self.0.concrete_type)
                        } else {
                            format!("`{}` is supplied here", self.0.concrete_type)
                        },
                    );
                }
                diag.note(rationale);
                if self.0.is_closed_package_public {
                    diag.note(
                        "this public declaration is analyzed because the package declares `publish = false`",
                    );
                }
                diag.help(remediation);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// UnconsumedGenericAbstractions: Nominal substitution policy
// -----------------------------------------------------------------------------
/// Late pass collecting generic nominal declarations and active authored substitutions.
#[derive(Default)]
struct UnconsumedGenericAbstractions {
    /// Crate-wide nominal substitution analyzer.
    analyzer: GenericAbstractionAnalyzer,
    /// Crate-wide resolved callable substitution analyzer.
    callable_analyzer: CallableGenericAnalyzer,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub UNCONSUMED_GENERIC_ABSTRACTIONS,
    Warn,
    "questions generic parameters with only one concrete substitution",
    UnconsumedGenericAbstractions::default()
}

impl<'tcx> LateLintPass<'tcx> for UnconsumedGenericAbstractions {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.analyzer.record_item(item);
        self.callable_analyzer.record_item(cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        self.callable_analyzer.record_impl_item(cx, item);
    }

    fn check_ty(&mut self, cx: &LateContext<'tcx>, ty: &'tcx Ty<'tcx, AmbigArg>) {
        self.analyzer.record_ty(cx, ty);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        self.analyzer.record_expr(cx, expression);
        self.callable_analyzer.record_expression(cx, expression);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let mut findings = take(&mut self.analyzer).findings(cx);
        findings.extend(take(&mut self.callable_analyzer).findings(cx));
        findings.sort_by_key(|finding| finding.parameter.span.lo());
        for finding in findings {
            Violation(finding).emit(cx);
        }
    }
}
