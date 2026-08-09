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
// Violation: Overloaded declaration family diagnostic
// -----------------------------------------------------------------------------

/// Section whose declaration count exceeds the configured conceptual budget.
struct Violation {
    /// Overloaded divider span used as the primary diagnostic site.
    span: Span,
    /// Analyzer-derived summary containing the measured declaration count.
    primary_message: String,
    /// Naming-first decomposition guidance for the concrete section.
    remediation_message: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.primary_message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the broad section conceals smaller naming families and provides an easy bucket for unrelated declarations",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation_message)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            OVERLOADED_DECLARATION_SECTIONS,
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
// OverloadedDeclarationSections
// -----------------------------------------------------------------------------

/// Late lint pass that rejects declaration sections broad enough to obscure their concepts.
struct OverloadedDeclarationSections {
    /// Shared analyzer that parses and groups authored section dividers.
    analyzer: SectionAnalyzer,
}

impl OverloadedDeclarationSections {
    /// Builds the pass from configured divider syntax and declaration limits.
    fn new() -> Self {
        Self {
            analyzer: SectionAnalyzer::from_config(),
        }
    }
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Rejects a valid section divider when it governs more distinct declarations than the
    /// configured maximum. A nominal type and all of its implementation blocks count as one
    /// declaration, while separate types, free functions, constants, and statics count
    /// independently. The default maximum is eight declarations per section.
    ///
    /// ### Why is this bad?
    ///
    /// An oversized section usually means its prefix describes a file-wide topic rather than one
    /// followable concept. That broad bucket conceals smaller naming families and encourages new
    /// declarations to accumulate without an explicit owner. The remedy is naming-first: identify
    /// the concepts that already exist, give related declarations coherent names, and introduce a
    /// second section only when it represents an independent family.
    ///
    /// For example, a broad transport section can hide unrelated request and response concepts:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // Transport
    /// // -----------------------------------------------------------------------------
    /// struct Request;
    /// struct RequestBuilder;
    /// struct RequestHeaders;
    /// struct RequestBody;
    /// struct Response;
    /// struct ResponseBuilder;
    /// struct ResponseHeaders;
    /// struct ResponseBody;
    /// struct TransportError;
    /// ```
    ///
    /// Make the conceptual families visible in both names and sections:
    ///
    /// ```rust
    /// // -----------------------------------------------------------------------------
    /// // Request
    /// // -----------------------------------------------------------------------------
    /// struct Request;
    /// struct RequestBuilder;
    /// struct RequestHeaders;
    /// struct RequestBody;
    ///
    /// // -----------------------------------------------------------------------------
    /// // Response
    /// // -----------------------------------------------------------------------------
    /// struct Response;
    /// struct ResponseBuilder;
    /// struct ResponseHeaders;
    /// struct ResponseBody;
    /// ```
    ///
    /// Configure the threshold through
    /// `section_dividers.max_declarations_per_section`. Sections at the maximum are accepted;
    /// only sections above it are rejected.
    pub OVERLOADED_DECLARATION_SECTIONS,
    Warn,
    "rejects section dividers that govern too many distinct declarations",
    OverloadedDeclarationSections::new()
}

impl<'tcx> LateLintPass<'tcx> for OverloadedDeclarationSections {
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, hir_id: HirId) {
        for finding in self.analyzer.analyze(cx, module, hir_id).overloaded {
            Violation {
                span: finding.span,
                primary_message: finding.message,
                remediation_message: finding.help,
            }
            .emit(cx);
        }
    }
}
