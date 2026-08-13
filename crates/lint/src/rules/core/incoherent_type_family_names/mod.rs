extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::family_name_analysis::FamilyNameAnalyzer;

// -----------------------------------------------------------------------------
// Violation: Incoherent declaration family naming diagnostic
// -----------------------------------------------------------------------------

/// One declaration-specific explanation captured from naming analysis.
struct ViolationLabel {
    /// Declaration span annotated by the diagnostic.
    span: Span,
    /// Inferred semantic role obscured by the authored name.
    message: String,
}

/// Declaration family whose shared organizational prefix obscures semantic roles.
struct Violation {
    /// Section or declaration span used as the primary diagnostic site.
    span: Span,
    /// Analyzer-derived summary naming the incoherent prefix.
    primary_message: String,
    /// Confidence-aware rename guidance derived from occupied names and dependencies.
    remediation_message: String,
    /// Declaration-specific explanations for proposed semantic roles.
    labels: Vec<ViolationLabel>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.primary_message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the repeated organizational prefix makes distinct roles harder to identify and encourages new names to mirror file structure instead of domain concepts",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation_message)
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary_message = self.primary_message().into_owned();
        let rationale_message = self.rationale_message().into_owned();
        let remediation_message = self.remediation_message().into_owned();
        cx.emit_span_lint(
            INCOHERENT_TYPE_FAMILY_NAMES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary_message);
                for label in self.labels {
                    diag.span_label(label.span, label.message);
                }
                diag.note(rationale_message);
                diag.help(remediation_message);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// IncoherentTypeFamilyNames
// -----------------------------------------------------------------------------

/// Late lint pass that detects organizational context embedded in type names.
struct IncoherentTypeFamilyNames {
    /// Shared semantic analyzer for declaration-family naming.
    analyzer: FamilyNameAnalyzer,
}

impl IncoherentTypeFamilyNames {
    /// Builds the pass from configured source-organization policy.
    fn new() -> Self {
        Self {
            analyzer: FamilyNameAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub INCOHERENT_TYPE_FAMILY_NAMES,
    Warn,
    "detects organizational prefixes that obscure coherent type-family names",
    IncoherentTypeFamilyNames::new()
}

impl<'tcx> LateLintPass<'tcx> for IncoherentTypeFamilyNames {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        for finding in self.analyzer.analyze(cx, module, hir_id) {
            // Convert analyzer-owned labels into private violation evidence.
            let labels = finding
                .labels
                .into_iter()
                .map(|label| ViolationLabel {
                    span: label.span,
                    message: label.message,
                })
                .collect();

            // Emit the finding only after all label context has crossed the boundary.
            Violation {
                span: finding.span,
                primary_message: finding.message,
                remediation_message: finding.help,
                labels,
            }
            .emit(cx);
        }
    }
}
