extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::source_organization::SectionAnalyzer;

// -----------------------------------------------------------------------------
// MismatchedSectionDividerPrefixes
// -----------------------------------------------------------------------------

/// Late lint pass that compares divider prefixes with declaration-family names.
struct MismatchedSectionDividerPrefixes {
    /// Shared analyzer that parses and groups authored section dividers.
    analyzer: SectionAnalyzer,
}

impl MismatchedSectionDividerPrefixes {
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
    /// Requires a divider prefix to equal the longest PascalCase word prefix shared by every
    /// participating type, free function, constant, and static in its section. Rust identifier
    /// conventions are normalized before comparison, so `request_parser` belongs to `Request`.
    /// A divider may instead match the containing module's name: the module then supplies the
    /// family namespace, allowing idiomatic APIs such as `identifier_case::words` without forcing
    /// the redundant name `identifier_case::identifier_words`.
    ///
    /// ### Why is this bad?
    ///
    /// A mismatched or vague prefix conceals inconsistent names. The preferred remedy is to rename
    /// related outliers into a followable family, not to create a section for every declaration.
    ///
    /// For example, Response does not belong to the declared Request family:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // Request
    /// // -----------------------------------------------------------------------------
    /// struct Request;
    /// struct Response;
    /// ```
    ///
    /// Rename a related declaration so the family is visible:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // Request
    /// // -----------------------------------------------------------------------------
    /// struct Request;
    /// struct RequestResponse;
    /// ```
    ///
    /// A module-owned namespace is also valid even when its declarations have different names:
    ///
    /// ```rust
    /// mod identifier_case {
    ///     // -----------------------------------------------------------------------------
    ///     // IdentifierCase
    ///     // -----------------------------------------------------------------------------
    ///     pub fn words(value: &str) -> Vec<String> {
    ///         todo!()
    ///     }
    ///
    ///     pub fn is_pascal(value: &str) -> bool {
    ///         todo!()
    ///     }
    /// }
    /// ```
    pub MISMATCHED_SECTION_DIVIDER_PREFIXES,
    Warn,
    "requires divider prefixes to match their declaration families",
    MismatchedSectionDividerPrefixes::new()
}

impl<'tcx> LateLintPass<'tcx> for MismatchedSectionDividerPrefixes {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        for finding in self.analyzer.analyze(cx, module, hir_id).mismatches {
            cx.emit_span_lint(
                MISMATCHED_SECTION_DIVIDER_PREFIXES,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(finding.message);
                    diag.help(finding.help);
                }),
            );
        }
    }
}
