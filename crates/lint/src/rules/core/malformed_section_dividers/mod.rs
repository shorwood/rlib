extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::section_analysis::{SectionAnalyzer, SectionFinding};

// -----------------------------------------------------------------------------
// Remediation: Malformed divider repair
// -----------------------------------------------------------------------------

/// Actionable repair available for one malformed divider.
enum Remediation {
    /// Plain-language guidance when no safe edit can be synthesized.
    Help {
        /// Actionable guidance rendered through `diag.help`.
        message: String,
    },
    /// Complete canonical divider text suitable for rustfix.
    Replacement {
        /// Canonical source text that replaces the malformed divider.
        source: String,
    },
}

// -----------------------------------------------------------------------------
// Violation: Malformed divider diagnostic
// -----------------------------------------------------------------------------

/// One malformed divider with its diagnostic text and safest remediation.
struct Violation {
    /// Complete authored divider extent.
    span: Span,
    /// Explanation of the syntax, placement, or width failure.
    primary_message: String,
    /// Guidance or canonical replacement selected by the analyzer.
    remediation: Remediation,
}

impl From<SectionFinding> for Violation {
    /// Converts shared section analysis into this lint's diagnostic vocabulary.
    fn from(finding: SectionFinding) -> Self {
        // Select the strongest remediation supported by the analyzer's source evidence.
        let remediation = finding.replacement.map_or_else(
            || Remediation::Help {
                message: finding.help,
            },
            |source| Remediation::Replacement { source },
        );

        // Preserve the finding location and explanation beside the selected repair.
        Self {
            span: finding.span,
            primary_message: finding.message,
            remediation,
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.primary_message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "well-formed, nonempty, consistently sized dividers give readers and tools predictable navigation landmarks",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        match &self.remediation {
            Remediation::Help { message } => Cow::Borrowed(message),
            Remediation::Replacement { .. } => Cow::Borrowed("render this divider canonically"),
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Materialize borrowed diagnostic text before consuming the remediation payload.
        let primary_message = self.primary_message().into_owned();
        let rationale_message = self.rationale_message().into_owned();
        let remediation_message = self.remediation_message().into_owned();

        // Render either guidance or the safe canonical replacement selected during analysis.
        cx.emit_span_lint(
            MALFORMED_SECTION_DIVIDERS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary_message);
                diag.note(rationale_message);
                match self.remediation {
                    Remediation::Help { .. } => diag.help(remediation_message),
                    Remediation::Replacement { source } => diag.span_suggestion(
                        self.span,
                        remediation_message,
                        source,
                        Applicability::MachineApplicable,
                    ),
                };
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// MalformedSectionDividers: Canonical divider syntax policy
// -----------------------------------------------------------------------------

/// Late lint pass that validates configured section-divider syntax and width.
struct MalformedSectionDividers {
    /// Shared analyzer that parses and groups authored section dividers.
    analyzer: SectionAnalyzer,
}

impl MalformedSectionDividers {
    /// Builds the pass from configured divider syntax.
    fn new() -> Self {
        Self {
            analyzer: SectionAnalyzer::from_config(),
        }
    }
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MALFORMED_SECTION_DIVIDERS,
    Warn,
    "rejects malformed or overlong section dividers",
    MalformedSectionDividers::new()
}

impl<'tcx> LateLintPass<'tcx> for MalformedSectionDividers {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        for finding in self.analyzer.analyze(cx, module, hir_id).malformed {
            Violation::from(finding).emit(cx);
        }
    }
}
