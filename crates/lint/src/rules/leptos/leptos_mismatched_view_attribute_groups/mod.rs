extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::rules::leptos::utils::view_structure::{
    LeptosViewStructureConfig, ViewAttributeCategory, ViewCallSites,
};
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: HirId,
    span: Span,
    attribute: String,
    heading: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "attribute `{}` contradicts group heading `{}`",
            self.attribute, self.heading
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "attribute-group headings must accurately describe the responsibility they own",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "move this attribute to a matching group or rename a genuinely shared responsibility",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_MISMATCHED_VIEW_ATTRIBUTE_GROUPS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this attribute has a different responsibility");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct LeptosMismatchedViewAttributeGroups {
    config: LeptosViewStructureConfig,
    views: ViewCallSites,
}

impl LeptosMismatchedViewAttributeGroups {
    fn new() -> Self {
        Self {
            config: LeptosViewStructureConfig::from_config(),
            views: ViewCallSites::default(),
        }
    }

    fn declared_category(heading: &str) -> Option<ViewAttributeCategory> {
        let heading = heading.to_ascii_lowercase();
        let matches = [
            (
                ViewAttributeCategory::Accessibility,
                &["accessib", "aria"][..],
            ),
            (
                ViewAttributeCategory::Presentation,
                &["present", "visual", "style"][..],
            ),
            (
                ViewAttributeCategory::Behavior,
                &["behavior", "event", "interaction"][..],
            ),
            (
                ViewAttributeCategory::Identity,
                &["identity", "semantic"][..],
            ),
            (ViewAttributeCategory::State, &["state", "availability"][..]),
            (ViewAttributeCategory::Data, &["data"][..]),
            (ViewAttributeCategory::Integration, &["integration"][..]),
        ]
        .into_iter()
        .filter_map(|(category, words)| {
            words
                .iter()
                .any(|word| heading.contains(word))
                .then_some(category)
        })
        .collect::<Vec<_>>();
        let [category] = matches.as_slice() else {
            return None;
        };
        Some(*category)
    }

    fn compatible(declared: ViewAttributeCategory, actual: ViewAttributeCategory) -> bool {
        declared == actual
            || actual == ViewAttributeCategory::Other
            || matches!(
                (declared, actual),
                (
                    ViewAttributeCategory::State,
                    ViewAttributeCategory::Accessibility
                ) | (
                    ViewAttributeCategory::Accessibility,
                    ViewAttributeCategory::State
                )
            )
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_MISMATCHED_VIEW_ATTRIBUTE_GROUPS,
    Warn,
    "rejects Leptos attributes beneath contradictory group headings",
    LeptosMismatchedViewAttributeGroups::new()
}

impl<'tcx> LateLintPass<'tcx> for LeptosMismatchedViewAttributeGroups {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let Some(view) = self.views.analyze(cx, expression) else {
            return;
        };
        for element in &view.elements {
            for group in element.groups(&self.config) {
                let Some(content) = group.heading.canonical_content(&self.config) else {
                    continue;
                };
                let Some(declared) = Self::declared_category(content) else {
                    continue;
                };
                for attribute in group.attributes {
                    if Self::compatible(declared, attribute.category) {
                        continue;
                    }
                    Violation {
                        owner: view.owner,
                        span: attribute.span,
                        attribute: attribute.name.clone(),
                        heading: content.to_owned(),
                    }
                    .emit(cx);
                }
            }
        }
    }
}
