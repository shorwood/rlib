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

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `attribute` value used by this analysis.
    attribute: String,
    /// Stores the `heading` value used by this analysis.
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

/// Connects heading vocabulary to the attribute category it declares.
struct DeclaredCategory {
    /// Category declared by a matching heading.
    category: ViewAttributeCategory,
    /// Word fragments that identify the category.
    words: &'static [&'static str],
}

/// Carries the `LeptosMismatchedViewAttributeGroups` state used by this analysis.
struct LeptosMismatchedViewAttributeGroups {
    /// Stores the `config` value used by this analysis.
    config: LeptosViewStructureConfig,
    /// Stores the `views` value used by this analysis.
    views: ViewCallSites,
}

impl LeptosMismatchedViewAttributeGroups {
    /// Heading vocabulary mapped to the attribute category it declares.
    const DECLARED_CATEGORIES: &[DeclaredCategory] = &[
        DeclaredCategory {
            category: ViewAttributeCategory::Accessibility,
            words: &["accessib", "aria"],
        },
        DeclaredCategory {
            category: ViewAttributeCategory::Presentation,
            words: &["present", "visual", "style"],
        },
        DeclaredCategory {
            category: ViewAttributeCategory::Behavior,
            words: &["behavior", "event", "interaction"],
        },
        DeclaredCategory {
            category: ViewAttributeCategory::Identity,
            words: &["identity", "semantic"],
        },
        DeclaredCategory {
            category: ViewAttributeCategory::State,
            words: &["state", "availability"],
        },
        DeclaredCategory {
            category: ViewAttributeCategory::Data,
            words: &["data"],
        },
        DeclaredCategory {
            category: ViewAttributeCategory::Integration,
            words: &["integration"],
        },
    ];

    /// Performs the `new` operation for this value.
    fn new() -> Self {
        Self {
            config: LeptosViewStructureConfig::from_config(),
            views: ViewCallSites::default(),
        }
    }

    /// Performs the `declared_category` operation for this value.
    fn declared_category(heading: &str) -> Option<ViewAttributeCategory> {
        // Prepare the values used by this stage.
        let heading = heading.to_ascii_lowercase();

        // Prepare the values used by this stage.
        let matches = Self::DECLARED_CATEGORIES
            .iter()
            .filter_map(|declared| {
                declared
                    .words
                    .iter()
                    .any(|word| heading.contains(word))
                    .then_some(declared.category)
            })
            .collect::<Vec<_>>();

        // Prepare the values used by this stage.
        let [category] = matches.as_slice() else {
            return None;
        };
        Some(*category)
    }

    /// Performs the `compatible` operation for this value.
    fn compatible(declared: ViewAttributeCategory, actual: ViewAttributeCategory) -> bool {
        // Perform the next step of the analysis.
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
                    // Reject inputs that do not satisfy this stage.
                    if Self::compatible(declared, attribute.category) {
                        continue;
                    }

                    // Perform the next step of the analysis.
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
