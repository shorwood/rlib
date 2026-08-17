extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::rules::leptos::utils::view_structure::{LeptosViewStructureConfig, ViewCallSites};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Unnamed view region diagnostic
// -----------------------------------------------------------------------------

/// Complex direct sibling scope without a heading for its first region.
struct Violation {
    /// Expanded expression used to honor local lint attributes.
    owner: HirId,
    /// Direct scope highlighted by the diagnostic.
    span: Span,
    /// Measured navigation complexity.
    complexity: usize,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "this view region has direct complexity {} without a section heading",
            self.complexity
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "unnamed visual and behavioral regions make a large declarative view difficult to scan",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "name the stable regions with concise `//` comments, or extract a component whose name carries the responsibility",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_MISSING_VIEW_SECTION_COMMENTS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "the first region starts without a heading");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosMissingViewSectionComments: View navigation policy
// -----------------------------------------------------------------------------

/// Late lint pass requiring named regions in complex authored `view!` scopes.
struct LeptosMissingViewSectionComments {
    /// Shared configurable complexity policy.
    config: LeptosViewStructureConfig,
    /// Deduplicated rstml-backed authored view analysis.
    views: ViewCallSites,
}

impl LeptosMissingViewSectionComments {
    /// Builds the pass from the shared Leptos view configuration.
    fn new() -> Self {
        Self {
            config: LeptosViewStructureConfig::from_config(),
            views: ViewCallSites::default(),
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_MISSING_VIEW_SECTION_COMMENTS,
    Warn,
    "requires section headings in complex Leptos view scopes",
    LeptosMissingViewSectionComments::new()
}

impl<'tcx> LateLintPass<'tcx> for LeptosMissingViewSectionComments {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Expressions outside authored view macros contain no sections to name.
        let Some(view) = self.views.analyze(cx, expression) else {
            return;
        };
        for scope in &view.scopes {
            let complexity = scope.complexity();
            let first_is_named = scope.headings.iter().any(|heading| {
                heading.node == Some(0) && heading.canonical_content(&self.config).is_some()
            });
            if complexity <= self.config.max_unnamed_view_complexity || first_is_named {
                continue;
            }

            let (Some(first), Some(last)) = (scope.nodes.first(), scope.nodes.last()) else {
                continue;
            };

            Violation {
                owner: view.owner,
                span: first.span.with_hi(last.span.hi()),
                complexity,
            }
            .emit(cx);
        }
    }
}
