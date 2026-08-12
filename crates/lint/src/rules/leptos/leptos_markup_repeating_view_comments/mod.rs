extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use convert_case::{Case, Casing};
use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::rules::leptos::utils::view_structure::{
    LeptosViewStructureConfig, ViewCallSites, ViewNode,
};
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

/// Normalizes a short authored name without fuzzy matching.
fn normalize(value: &str) -> String {
    value
        .to_case(Case::Snake)
        .split('_')
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Returns whether one heading exactly translates its only structural node.
fn repeats_node(content: &str, node: &ViewNode) -> bool {
    let heading = normalize(content);
    let Some(name) = node.name.as_deref() else {
        return false;
    };
    let node_name = normalize(name);
    if heading == node_name {
        return true;
    }
    if matches!(
        (heading.as_str(), node_name.as_str()),
        ("navigation", "nav")
    ) {
        return true;
    }
    node_name == "button"
        && node.literal.as_deref().is_some_and(|literal| {
            let literal = normalize(literal);
            heading == format!("{literal} button") || heading == format!("button {literal}")
        })
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_MARKUP_REPEATING_VIEW_COMMENTS,
    Warn,
    "rejects one-node Leptos headings that merely restate markup",
    LeptosMarkupRepeatingViewComments::new()
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
            if repeats_node(content, node) {
                Violation {
                    owner: view.owner,
                    heading: section.heading.span,
                    node: node.span,
                }
                .emit(cx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::repeats_node;
    use super::rustc_span::DUMMY_SP;
    use crate::rules::leptos::utils::view_structure::ViewNode;

    fn node(name: &str, literal: Option<&str>) -> ViewNode {
        ViewNode {
            span: DUMMY_SP,
            range: 0..1,
            complexity: 1,
            name: Some(name.to_owned()),
            literal: literal.map(str::to_owned),
        }
    }

    #[test]
    fn recognizes_only_exact_structural_restatements() {
        assert!(repeats_node("Navigation", &node("Navigation", None)));
        assert!(repeats_node("Navigation", &node("nav", None)));
        assert!(repeats_node(
            "Submit button",
            &node("button", Some("Submit"))
        ));
        assert!(!repeats_node(
            "Account navigation",
            &node("Navigation", None)
        ));
        assert!(!repeats_node("Submit button", &node("button", None)));
    }
}
