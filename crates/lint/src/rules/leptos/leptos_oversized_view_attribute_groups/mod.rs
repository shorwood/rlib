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

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `complexity` value used by this analysis.
    complexity: usize,
    /// Stores the `maximum` value used by this analysis.
    maximum: usize,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "attribute group complexity {} exceeds configured maximum {}",
            self.complexity, self.maximum
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "oversized groups make a heading decorative instead of a bounded navigation aid",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "split this group by stable responsibility or extract a narrower component interface",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_OVERSIZED_VIEW_ATTRIBUTE_GROUPS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this named group still owns too much complexity");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Carries the `LeptosOversizedViewAttributeGroups` state used by this analysis.
struct LeptosOversizedViewAttributeGroups {
    /// Stores the `config` value used by this analysis.
    config: LeptosViewStructureConfig,
    /// Stores the `views` value used by this analysis.
    views: ViewCallSites,
}

impl LeptosOversizedViewAttributeGroups {
    /// Performs the `new` operation for this value.
    fn new() -> Self {
        Self {
            config: LeptosViewStructureConfig::from_config(),
            views: ViewCallSites::default(),
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_OVERSIZED_VIEW_ATTRIBUTE_GROUPS,
    Warn,
    "rejects oversized named Leptos attribute groups",
    LeptosOversizedViewAttributeGroups::new()
}

impl<'tcx> LateLintPass<'tcx> for LeptosOversizedViewAttributeGroups {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let Some(view) = self.views.analyze(cx, expression) else {
            return;
        };
        for group in view
            .elements
            .iter()
            .flat_map(|element| element.groups(&self.config))
        {
            // Prepare the values used by this stage.
            let complexity = group.complexity();
            if complexity <= self.config.max_view_attribute_group_complexity {
                continue;
            }
            let Some(last) = group.attributes.last() else {
                continue;
            };

            // Perform the next step of the analysis.
            Violation {
                owner: view.owner,
                span: group.heading.span.with_hi(last.span.hi()),
                complexity,
                maximum: self.config.max_view_attribute_group_complexity,
            }
            .emit(cx);
        }
    }
}
