extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::section_dividers::DividerAnalyzer;

// -----------------------------------------------------------------------------
// MalformedSectionDividers
// -----------------------------------------------------------------------------

struct MalformedSectionDividers {
    analyzer: DividerAnalyzer,
}

impl MalformedSectionDividers {
    fn new() -> Self {
        Self {
            analyzer: DividerAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Validates section-divider content and rendered width against the configured template.
    /// Content uses a PascalCase prefix with an optional sentence-case description after a colon.
    ///
    /// ### Why is this bad?
    ///
    /// Inconsistent syntax weakens dividers as navigation landmarks and makes generated edits
    /// unpredictable. Empty or overlong sections also hide rather than clarify source structure.
    ///
    /// For example, this divider has non-canonical spacing and description casing:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // Request : request handling
    /// // -----------------------------------------------------------------------------
    /// struct Request;
    /// ```
    ///
    /// Render the same content canonically:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // Request: Request handling
    /// // -----------------------------------------------------------------------------
    /// struct Request;
    /// ```
    pub MALFORMED_SECTION_DIVIDERS,
    Warn,
    "rejects malformed or overlong section dividers",
    MalformedSectionDividers::new()
}

impl<'tcx> LateLintPass<'tcx> for MalformedSectionDividers {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        for finding in self.analyzer.analyze(cx, module, hir_id).malformed {
            cx.emit_span_lint(
                MALFORMED_SECTION_DIVIDERS,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(finding.message);
                    if let Some(replacement) = finding.replacement {
                        diag.span_suggestion(
                            finding.span,
                            "render this divider canonically",
                            replacement,
                            Applicability::MachineApplicable,
                        );
                    } else {
                        diag.help(finding.help);
                    }
                }),
            );
        }
    }
}
