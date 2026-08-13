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
// Violation: View attribute group too broad to scan
// -----------------------------------------------------------------------------

/// Attribute group too large to communicate one coherent responsibility.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Direct authored complexity charged to this construct.
    complexity: usize,
    /// Configured maximum attributes allowed under one responsibility heading.
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

// -----------------------------------------------------------------------------
// LeptosOversizedViewAttributeGroups: Focused attribute-group policy
// -----------------------------------------------------------------------------

/// Rejects attribute groups too large to communicate one coherent responsibility.
struct LeptosOversizedViewAttributeGroups {
    /// Validated project policy applied by this lint pass.
    config: LeptosViewStructureConfig,
    /// Parsed `view!` invocations awaiting attribute-group sizing.
    views: ViewCallSites,
}

impl LeptosOversizedViewAttributeGroups {
    /// Starts view analysis with the configured group-size limit.
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
            let complexity = group.complexity();
            if complexity <= self.config.max_view_attribute_group_complexity {
                continue;
            }
            let Some(last) = group.attributes.last() else {
                continue;
            };

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
