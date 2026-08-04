extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::source_organization::FamilyNameAnalyzer;

// -----------------------------------------------------------------------------
// IncoherentTypeFamilyNames
// -----------------------------------------------------------------------------

struct IncoherentTypeFamilyNames {
    analyzer: FamilyNameAnalyzer,
}

impl IncoherentTypeFamilyNames {
    fn new() -> Self {
        Self {
            analyzer: FamilyNameAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds type names that repeat the module or lint-pass name instead of expressing the smaller
    /// concept that connects those declarations. It analyzes valid sections and unsectioned module
    /// declarations, combining normalized Rust identifier words with compiler-resolved
    /// dependencies and source proximity.
    ///
    /// ### Why is this bad?
    ///
    /// Names should determine the useful sections, not be lengthened merely to satisfy a divider.
    /// Repeating broad organizational context hides roles, makes related helpers harder to scan,
    /// and encourages agents to solve naming problems by creating more singleton sections.
    ///
    /// For example, these helpers repeat the enclosing lint name:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // MethodLikeFreeFunctions
    /// // -----------------------------------------------------------------------------
    ///
    /// struct MethodLikeFreeFunctions;
    /// struct MethodLikeFreeFunctionsSourceEdits;
    /// struct MethodLikeFreeFunctionsMigrationBuilder {
    ///     edits: MethodLikeFreeFunctionsSourceEdits,
    /// }
    /// ```
    ///
    /// Naming the smaller concept first produces a followable family:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // Migration
    /// // -----------------------------------------------------------------------------
    ///
    /// struct MigrationEdits;
    /// struct MigrationBuilder {
    ///     edits: MigrationEdits,
    /// }
    /// ```
    pub INCOHERENT_TYPE_FAMILY_NAMES,
    Warn,
    "detects organizational prefixes that obscure coherent type-family names",
    IncoherentTypeFamilyNames::new()
}

impl<'tcx> LateLintPass<'tcx> for IncoherentTypeFamilyNames {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        for finding in self.analyzer.analyze(cx, module, hir_id) {
            cx.emit_span_lint(
                INCOHERENT_TYPE_FAMILY_NAMES,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(finding.message);
                    for label in finding.labels {
                        diag.span_label(label.span, label.message);
                    }
                    diag.help(finding.help);
                }),
            );
        }
    }
}
