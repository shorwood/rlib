extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::section_analysis::SectionAnalyzer;

// -----------------------------------------------------------------------------
// Violation: Missing declaration family boundary diagnostic
// -----------------------------------------------------------------------------

/// Declaration family lacking the configured authored divider.
struct Violation {
    /// Unsectioned declaration-family span.
    span: Span,
    /// Analyzer-derived summary of the uncovered declarations.
    primary_message: String,
    /// Exact configured divider content proposed for the family.
    remediation_message: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.primary_message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "without an explicit boundary, neighboring declarations do not reveal whether they intentionally form one naming family",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation_message)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MISSING_SECTION_DIVIDERS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// MissingSectionDividers
// -----------------------------------------------------------------------------

/// Late lint pass that requires authored dividers for declaration families.
struct MissingSectionDividers {
    /// Shared analyzer that parses and groups authored section dividers.
    analyzer: SectionAnalyzer,
}

impl MissingSectionDividers {
    /// Builds the pass from configured divider syntax.
    fn new() -> Self {
        Self {
            analyzer: SectionAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Requires module-level declaration groups to be covered by a configured section divider.
    /// Nominal types and their direct impls always participate. Free functions, constants, and
    /// statics also participate when they form a group or occur inside an authored section; an
    /// isolated value declaration does not require a divider by itself.
    ///
    /// ### Why is this bad?
    ///
    /// A divider makes the intended naming family explicit. Without one, agents cannot tell
    /// whether neighboring declarations are deliberately related or merely accumulated in the
    /// same file. Ignoring free helpers also lets a nominally valid section conceal inconsistent
    /// vocabulary.
    ///
    /// For example, these declarations have no stated family:
    ///
    /// ```rust
    /// struct Request;
    /// impl Request {}
    /// ```
    ///
    /// A divider establishes the naming contract:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // Request: Request model and behavior
    /// // -----------------------------------------------------------------------------
    ///
    /// struct Request;
    /// impl Request {}
    /// ```
    pub MISSING_SECTION_DIVIDERS,
    Warn,
    "requires section dividers for module-level declaration groups",
    MissingSectionDividers::new()
}

impl<'tcx> LateLintPass<'tcx> for MissingSectionDividers {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        for finding in self.analyzer.analyze(cx, module, hir_id).missing {
            Violation {
                span: finding.span,
                primary_message: finding.message,
                remediation_message: finding.help,
            }
            .emit(cx);
        }
    }
}
