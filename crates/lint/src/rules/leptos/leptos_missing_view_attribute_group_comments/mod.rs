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

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `name` value used by this analysis.
    name: String,
    /// Stores the `complexity` value used by this analysis.
    complexity: usize,
    /// Stores the `categories` value used by this analysis.
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

/// Carries the `LeptosMissingViewAttributeGroupComments` state used by this analysis.
struct LeptosMissingViewAttributeGroupComments {
    /// Stores the `config` value used by this analysis.
    config: LeptosViewStructureConfig,
    /// Stores the `views` value used by this analysis.
    views: ViewCallSites,
}

impl LeptosMissingViewAttributeGroupComments {
    /// Minimum distinct attribute responsibilities that require authored grouping.
    const MINIMUM_ATTRIBUTE_CATEGORIES: usize = 3;

    /// Minimum authored groups that demonstrate meaningful organization.
    const MINIMUM_AUTHORED_GROUPS: usize = 2;

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
            // Prepare the values used by this stage.
            let complexity = element.complexity();
            let categories = element.category_count();
            if complexity <= self.config.max_unnamed_view_attribute_complexity
                || categories < Self::MINIMUM_ATTRIBUTE_CATEGORIES
                || element.group_count(&self.config) >= Self::MINIMUM_AUTHORED_GROUPS
            {
                continue;
            }

            // Perform the next step of the analysis.
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
