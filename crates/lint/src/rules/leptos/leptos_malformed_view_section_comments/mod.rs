extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{Expr, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::rules::leptos::utils::view_structure::{
    LeptosViewStructureConfig, ViewCallSites, ViewHeading,
};
use crate::utils::diagnostic::LateViolation;
use crate::utils::function_layout_prose::FunctionLayoutProse;

// -----------------------------------------------------------------------------
// Violation: Malformed view heading diagnostic
// -----------------------------------------------------------------------------

/// Authored view-boundary comment that violates syntax or placement policy.
struct Violation {
    /// Expanded expression used to honor local lint attributes.
    owner: HirId,
    /// Exact authored comment span.
    span: Span,
    /// Most actionable formatting or placement failure.
    message: &'static str,
    /// Safe canonical replacement when only formatting is wrong.
    replacement: Option<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.message)
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "consistent headings make view boundaries predictable for readers and related lints",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("use concise sentence-style prose immediately before the first owned node")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_MALFORMED_VIEW_SECTION_COMMENTS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                if let Some(replacement) = self.replacement {
                    diag.span_suggestion(
                        self.span,
                        "render this view heading canonically",
                        replacement,
                        Applicability::MachineApplicable,
                    );
                } else {
                    diag.help(self.remediation_message().into_owned());
                }
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosMalformedViewSectionComments: Canonical heading policy
// -----------------------------------------------------------------------------

/// Late lint pass validating direct-boundary view comments.
struct LeptosMalformedViewSectionComments {
    /// Shared comment syntax.
    config: LeptosViewStructureConfig,
    /// Deduplicated rstml-backed authored view analysis.
    views: ViewCallSites,
}

impl LeptosMalformedViewSectionComments {
    /// Builds the pass from shared Leptos view configuration.
    fn new() -> Self {
        Self {
            config: LeptosViewStructureConfig::from_config(),
            views: ViewCallSites::default(),
        }
    }

    /// Returns canonical heading content after the configured marker.
    fn content<'heading>(&self, heading: &'heading ViewHeading) -> Option<&'heading str> {
        heading
            .text
            .strip_prefix(&self.config.view_section_comment_prefix)?
            .strip_prefix(' ')
    }

    /// Returns whether heading prose follows the canonical compact form.
    fn is_canonical(&self, heading: &ViewHeading) -> bool {
        self.content(heading).is_some_and(|content| {
            !content.ends_with([':', '.', ';', '!', '?', ',', '-'])
                && !content.starts_with(['-', '*', '#'])
                && FunctionLayoutProse::is_canonical(Some(content))
        })
    }

    /// Produces an unambiguous source-only repair when possible.
    fn replacement(&self, heading: &ViewHeading) -> Option<String> {
        if !heading.text.starts_with("//") || heading.text.contains('\n') {
            return None;
        }
        let content = heading
            .text
            .strip_prefix(&self.config.view_section_comment_prefix)
            .unwrap_or(heading.text.trim_start_matches('/'))
            .trim()
            .trim_start_matches(['-', '*', '#'])
            .trim_start()
            .trim_end_matches([':', '.', ';', '!', '?', ',', '-'])
            .trim_end();
        FunctionLayoutProse::replacement(Some(content), &self.config.view_section_comment_prefix)
    }

    /// Classifies the first failure for one direct-boundary comment.
    fn violation(&self, heading: &ViewHeading, owner: HirId) -> Option<Violation> {
        let Some(node) = heading.node else {
            return Some(Violation {
                owner,
                span: heading.span,
                message: "this view section comment must immediately precede its region",
                replacement: None,
            });
        };
        let placement_is_valid =
            heading.immediately_precedes && (node == 0 || heading.blank_before);
        let (message, replacement) = if !self.is_canonical(heading) {
            (
                "this view section comment is not canonical",
                placement_is_valid
                    .then(|| self.replacement(heading))
                    .flatten(),
            )
        } else if !heading.immediately_precedes {
            (
                "this view section comment must immediately precede its region",
                None,
            )
        } else if node != 0 && !heading.blank_before {
            (
                "this view section comment must be preceded by a blank line",
                None,
            )
        } else {
            return None;
        };
        Some(Violation {
            owner,
            span: heading.span,
            message,
            replacement,
        })
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_MALFORMED_VIEW_SECTION_COMMENTS,
    Warn,
    "rejects malformed or misplaced Leptos view section comments",
    LeptosMalformedViewSectionComments::new()
}

impl<'tcx> LateLintPass<'tcx> for LeptosMalformedViewSectionComments {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let Some(view) = self.views.analyze(cx, expression) else {
            return;
        };
        for heading in view.scopes.iter().flat_map(|scope| &scope.headings) {
            if let Some(violation) = self.violation(heading, view.owner) {
                violation.emit(cx);
            }
        }
    }
}
