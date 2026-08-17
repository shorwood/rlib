extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use convert_case::{Case, Casing};
use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::rules::leptos::utils::view_structure::{LeptosViewStructureConfig, ViewCallSites};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Heading restates its only node
// -----------------------------------------------------------------------------

/// One-node section whose heading adds no information beyond its markup.
struct Violation {
    /// Expanded expression used to honor local lint attributes.
    owner: HirId,
    /// Exact authored heading span.
    heading: Span,
    /// Exact authored node span.
    node: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("this view section heading only repeats its markup")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("a heading should explain the region's role rather than translate its tag")
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("remove the heading, or name the user-facing responsibility of this region")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_MARKUP_REPEATING_VIEW_COMMENTS,
            self.owner,
            self.heading,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.heading, "repeats this node");
                diag.span_label(self.node, "the markup already says this");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Authored node text relevant to heading repetition.
#[derive(Clone, Copy)]
struct NodeText<'a> {
    /// Opening-tag name.
    name: &'a str,
    /// Directly owned literal text.
    literal: Option<&'a str>,
}

/// Normalizes a short authored name without fuzzy matching.
fn heading_normalize(value: &str) -> String {
    value
        .to_case(Case::Snake)
        .split('_')
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Returns whether one heading exactly translates its only structural node.
fn heading_repeats_node(content: &str, node: NodeText<'_>) -> bool {
    let heading = heading_normalize(content);
    let name = node.name;
    let terminal_name = name
        .rsplit([':', '/'])
        .find(|segment| !segment.trim().is_empty())
        .unwrap_or(name);
    let node_names = [heading_normalize(name), heading_normalize(terminal_name)];

    if node_names.contains(&heading) {
        return true;
    }

    if node_names.iter().any(|node_name| {
        matches!(
            (heading.as_str(), node_name.as_str()),
            ("navigation", "nav")
        )
    }) {
        return true;
    }

    node.literal.is_some_and(|literal| {
        let literal = heading_normalize(literal);
        heading == literal
            || node_names.iter().any(|node_name| {
                heading == format!("{literal} {node_name}")
                    || heading == format!("{node_name} {literal}")
            })
    })
}

// -----------------------------------------------------------------------------
// LeptosMarkupRepeatingViewComments: Informative heading policy
// -----------------------------------------------------------------------------

/// Late lint pass comparing one-node section names with rstml node semantics.
struct LeptosMarkupRepeatingViewComments {
    /// Shared canonical heading syntax.
    config: LeptosViewStructureConfig,
    /// Deduplicated rstml-backed authored view analysis.
    views: ViewCallSites,
}

impl LeptosMarkupRepeatingViewComments {
    /// Builds the pass from shared Leptos view configuration.
    fn new() -> Self {
        Self {
            config: LeptosViewStructureConfig::from_config(),
            views: ViewCallSites::default(),
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosMarkupRepeatingViewComments {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let Some(view) = self.views.analyze(cx, expression) else {
            return;
        };
        for section in view
            .scopes
            .iter()
            .flat_map(|scope| scope.sections(&self.config))
        {
            let [node] = section.nodes else {
                continue;
            };
            let content = section
                .heading
                .canonical_content(&self.config)
                .expect("sections have canonical headings");

            let Some(name) = node.name.as_deref() else {
                continue;
            };
            if !heading_repeats_node(
                content,
                NodeText {
                    name,
                    literal: node.literal.as_deref(),
                },
            ) {
                continue;
            }

            Violation {
                owner: view.owner,
                heading: section.heading.span,
                node: node.span,
            }
            .emit(cx);
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_MARKUP_REPEATING_VIEW_COMMENTS,
    Warn,
    "rejects one-node Leptos headings that merely restate markup",
    LeptosMarkupRepeatingViewComments::new()
}

#[cfg(test)]
mod tests {
    use super::{NodeText, heading_repeats_node};

    #[test]
    fn recognizes_only_exact_structural_restatements() {
        assert!(heading_repeats_node(
            "Navigation",
            NodeText {
                name: "Navigation",
                literal: None,
            }
        ));
        assert!(heading_repeats_node(
            "Navigation",
            NodeText {
                name: "nav",
                literal: None,
            }
        ));
        assert!(heading_repeats_node(
            "Submit button",
            NodeText {
                name: "button",
                literal: Some("Submit"),
            }
        ));
        assert!(heading_repeats_node(
            "Navigation",
            NodeText {
                name: "components::Navigation",
                literal: None,
            }
        ));
        assert!(heading_repeats_node(
            "Account status",
            NodeText {
                name: "h2",
                literal: Some("Account status"),
            }
        ));
        assert!(!heading_repeats_node(
            "Account navigation",
            NodeText {
                name: "Navigation",
                literal: None,
            }
        ));
        assert!(!heading_repeats_node(
            "Submit button",
            NodeText {
                name: "button",
                literal: None,
            }
        ));
    }
}
