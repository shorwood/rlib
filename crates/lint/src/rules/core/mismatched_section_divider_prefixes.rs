extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{HirId, Mod};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::source_organization::SectionAnalyzer;

// -----------------------------------------------------------------------------
// Violation: Mismatched section family diagnostic
// -----------------------------------------------------------------------------

/// Section whose prefix disagrees with the declarations it claims to organize.
struct Violation {
    /// Divider span used as the primary diagnostic site.
    span: Span,
    /// Analyzer-derived mismatch summary naming the governed declarations.
    primary_message: String,
    /// Naming-first correction derived from the section participants.
    remediation_message: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.primary_message)
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the divider advertises a naming family that the governed declarations do not consistently follow",
        )
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation_message)
    }
    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MISMATCHED_SECTION_DIVIDER_PREFIXES,
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
            Violation {
                span: finding.span,
                primary_message: finding.message,
                remediation_message: finding.help,
            }
            .emit(cx);
        }
    }
}
