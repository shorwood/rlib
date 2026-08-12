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
// Violation: Oversized named view region
// -----------------------------------------------------------------------------

/// Named view region whose direct navigation burden exceeds policy.
struct Violation {
    /// Expanded expression used to honor local lint attributes.
    owner: HirId,
    /// Exact authored heading span.
    span: Span,
    /// Measured direct complexity.
    complexity: usize,
    /// Configured maximum direct complexity.
    maximum: usize,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "this view section has complexity {}, exceeding the configured maximum of {}",
            self.complexity, self.maximum
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("large named regions still force readers to navigate too much markup at once")
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("split this region with another concise section heading")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_OVERSIZED_VIEW_SECTIONS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this heading owns the oversized region");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosOversizedViewSections: Named-region size policy
// -----------------------------------------------------------------------------

/// Late lint pass measuring sections projected from rstml sibling scopes.
struct LeptosOversizedViewSections {
    /// Shared comment syntax and complexity limit.
    config: LeptosViewStructureConfig,
    /// Deduplicated rstml-backed authored view analysis.
    views: ViewCallSites,
}

impl LeptosOversizedViewSections {
    /// Builds the pass from shared Leptos view configuration.
    fn new() -> Self {
        Self {
            config: LeptosViewStructureConfig::from_config(),
            views: ViewCallSites::default(),
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_OVERSIZED_VIEW_SECTIONS,
    Warn,
    "rejects Leptos view sections that own too much direct complexity",
    LeptosOversizedViewSections::new()
}

impl<'tcx> LateLintPass<'tcx> for LeptosOversizedViewSections {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let Some(view) = self.views.analyze(cx, expression) else {
            return;
        };
        for section in view
            .scopes
            .iter()
            .flat_map(|scope| scope.sections(&self.config))
        {
            let complexity = section.complexity();
            if complexity > self.config.max_view_section_complexity {
                Violation {
                    owner: view.owner,
                    span: section.heading.span,
                    complexity,
                    maximum: self.config.max_view_section_complexity,
                }
                .emit(cx);
            }
        }
    }
}
