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
// Violation: Unnamed opening tag responsibilities
// -----------------------------------------------------------------------------

/// Element whose distinct attribute responsibilities lack group headings.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Element or attribute name involved in the view-structure finding.
    name: String,
    /// Direct authored complexity charged to this construct.
    complexity: usize,
    /// Consecutive attribute responsibilities lacking an authored group heading.
    categories: usize,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` has attribute complexity {} across {} responsibilities without named groups",
            self.name, self.complexity, self.categories
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "dense opening tags hide the element's identity, state, presentation, accessibility, and behavior contracts",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "separate stable responsibilities with concise `//` headings, or extract a component with a narrower interface",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_MISSING_VIEW_ATTRIBUTE_GROUP_COMMENTS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this opening tag mixes unnamed responsibilities");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosMissingViewAttributeGroupComments: Opening tag layout policy
// -----------------------------------------------------------------------------

/// Requires headings when one element's attributes serve several responsibilities.
struct LeptosMissingViewAttributeGroupComments {
    /// Validated project policy applied by this lint pass.
    config: LeptosViewStructureConfig,
    /// Parsed `view!` invocations awaiting attribute-group analysis.
    views: ViewCallSites,
}

impl LeptosMissingViewAttributeGroupComments {
    /// Minimum distinct attribute responsibilities that require authored grouping.
    const MINIMUM_ATTRIBUTE_CATEGORIES: usize = 3;

    /// Minimum authored groups that demonstrate meaningful organization.
    const MINIMUM_AUTHORED_GROUPS: usize = 2;

    /// Starts view analysis with the configured attribute-group threshold.
    fn new() -> Self {
        Self {
            config: LeptosViewStructureConfig::from_config(),
            views: ViewCallSites::default(),
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_MISSING_VIEW_ATTRIBUTE_GROUP_COMMENTS,
    Warn,
    "requires named responsibility groups in dense Leptos opening tags",
    LeptosMissingViewAttributeGroupComments::new()
}

impl<'tcx> LateLintPass<'tcx> for LeptosMissingViewAttributeGroupComments {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let Some(view) = self.views.analyze(cx, expression) else {
            return;
        };
        for element in &view.elements {
            let complexity = element.complexity();
            let categories = element.category_count();
            if complexity <= self.config.max_unnamed_view_attribute_complexity
                || categories < Self::MINIMUM_ATTRIBUTE_CATEGORIES
                || element.group_count(&self.config) >= Self::MINIMUM_AUTHORED_GROUPS
            {
                continue;
            }

            Violation {
                owner: view.owner,
                span: element.span,
                name: element.name.clone(),
                complexity,
                categories,
            }
            .emit(cx);
        }
    }
}
