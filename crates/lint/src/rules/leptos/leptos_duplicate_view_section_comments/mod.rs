extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashMap;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::rules::leptos::utils::view_structure::{LeptosViewStructureConfig, ViewCallSites};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Repeated section name
// -----------------------------------------------------------------------------

/// Later heading that repeats a name in the same direct sibling scope.
struct Violation {
    /// Expanded expression used to honor local lint attributes.
    owner: HirId,
    /// Exact authored span of the repeated heading.
    span: Span,
    /// Exact authored span of the first heading.
    original: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("this view section heading is duplicated in the same scope")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("repeated names make distinct interface regions difficult to tell apart")
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("rename this heading to describe the responsibility unique to its region")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_DUPLICATE_VIEW_SECTION_COMMENTS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "repeated heading");
                diag.span_label(self.original, "first used here");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosDuplicateViewSectionComments: Scope-local name policy
// -----------------------------------------------------------------------------

/// Late lint pass comparing canonical names within each rstml sibling scope.
struct LeptosDuplicateViewSectionComments {
    /// Shared canonical heading syntax.
    config: LeptosViewStructureConfig,
    /// Deduplicated rstml-backed authored view analysis.
    views: ViewCallSites,
}

impl LeptosDuplicateViewSectionComments {
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
    pub LEPTOS_DUPLICATE_VIEW_SECTION_COMMENTS,
    Warn,
    "rejects duplicate Leptos view section headings in one sibling scope",
    LeptosDuplicateViewSectionComments::new()
}

impl<'tcx> LateLintPass<'tcx> for LeptosDuplicateViewSectionComments {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let Some(view) = self.views.analyze(cx, expression) else {
            return;
        };
        for scope in &view.scopes {
            let mut first_by_name = HashMap::new();
            for section in scope.sections(&self.config) {
                let content = section
                    .heading
                    .canonical_content(&self.config)
                    .expect("sections have canonical headings");
                let normalized = content
                    .split_whitespace()
                    .map(str::to_lowercase)
                    .collect::<Vec<_>>()
                    .join(" ");
                if let Some(original) = first_by_name.get(&normalized) {
                    Violation {
                        owner: view.owner,
                        span: section.heading.span,
                        original: *original,
                    }
                    .emit(cx);
                } else {
                    first_by_name.insert(normalized, section.heading.span);
                }
            }
        }
    }
}
