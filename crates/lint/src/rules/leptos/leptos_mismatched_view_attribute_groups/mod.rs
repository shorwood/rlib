extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::config::leptos::LeptosViewStructureConfig;
use crate::config::store::ConfigStore;
use crate::rules::leptos::utils::view_structure::{ViewAttributeCategory, ViewCallSites};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: View attribute under a mismatched heading
// -----------------------------------------------------------------------------

/// View attribute placed under a heading for a different responsibility.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Attribute whose semantic category contradicts its group heading.
    attribute: String,
    /// Authored heading that claims this group.
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

// -----------------------------------------------------------------------------
// LeptosMismatchedViewAttributeGroups: Coherent attribute grouping policy
// -----------------------------------------------------------------------------

/// Verifies that each view attribute matches its authored responsibility heading.
struct LeptosMismatchedViewAttributeGroups {
    /// Validated project policy applied by this lint pass.
    config: LeptosViewStructureConfig,
    /// Parsed `view!` invocations awaiting heading-to-attribute comparison.
    views: ViewCallSites,
}

impl LeptosMismatchedViewAttributeGroups {
    /// Heading vocabulary mapped to the attribute category it declares.
    const DECLARED_CATEGORIES: &[DeclaredCategory] = &[
        DeclaredCategory {
            category: ViewAttributeCategory::Accessibility,
            words: &["accessibility", "accessible", "aria"],
        },
        DeclaredCategory {
            category: ViewAttributeCategory::Presentation,
            words: &["presentation", "visual", "style", "styling"],
        },
        DeclaredCategory {
            category: ViewAttributeCategory::Behavior,
            words: &[
                "behavior",
                "behaviors",
                "event",
                "events",
                "interaction",
                "interactions",
            ],
        },
        DeclaredCategory {
            category: ViewAttributeCategory::Identity,
            words: &["identity", "semantic", "semantics"],
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

    /// Starts view analysis with no parsed macro invocations.
    fn new() -> Self {
        Self {
            config: ConfigStore::get().leptos_view_structure.clone(),
            views: ViewCallSites::default(),
        }
    }

    /// Maps an authored group heading to the responsibility it declares.
    fn declared_category(heading: &str) -> Option<ViewAttributeCategory> {
        let heading = heading.to_ascii_lowercase();
        let words = heading
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();

        let matches = Self::DECLARED_CATEGORIES
            .iter()
            .filter_map(|declared| {
                declared
                    .words
                    .iter()
                    .any(|word| words.contains(word))
                    .then_some(declared.category)
            })
            .collect::<Vec<_>>();

        // Headings that name zero or multiple categories lack one declared responsibility.
        let [category] = matches.as_slice() else {
            return None;
        };
        Some(*category)
    }

    /// Returns whether an attribute belongs under the declared responsibility.
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

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_MISMATCHED_VIEW_ATTRIBUTE_GROUPS,
    Warn,
    "rejects Leptos attributes beneath contradictory group headings",
    LeptosMismatchedViewAttributeGroups::new()
}

impl<'tcx> LateLintPass<'tcx> for LeptosMismatchedViewAttributeGroups {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Expressions outside authored view macros contain no attribute groups.
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

// -----------------------------------------------------------------------------
// Tests: In-source tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{LeptosMismatchedViewAttributeGroups, ViewAttributeCategory};

    #[test]
    fn category_terms_do_not_match_inside_unrelated_words() {
        assert_eq!(
            LeptosMismatchedViewAttributeGroups::declared_category("Submission presentation"),
            Some(ViewAttributeCategory::Presentation)
        );
        assert_eq!(
            LeptosMismatchedViewAttributeGroups::declared_category("Database connection"),
            None
        );
        assert_eq!(
            LeptosMismatchedViewAttributeGroups::declared_category("Statement formatting"),
            None
        );
    }
}
