extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::mem::discriminant;

use rstml::node::{Node, NodeFragment};
use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind, HirId};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{ExprMacro, Meta, Token, UnOp};

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Hydration shape mismatch
// -----------------------------------------------------------------------------

/// Environment branch whose server and browser markup shapes cannot hydrate safely.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Markup structure produced when the server-only branch is selected.
    server_shape: Vec<String>,
    /// Markup structure produced when the browser-only branch is selected.
    browser_shape: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("server and browser branches produce different initial view shapes")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "hydration expects the server DOM shape {:?} to match the browser shape {:?}",
            self.server_shape, self.browser_shape
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "render one stable initial element shape and defer browser-only changes until after hydration",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_HYDRATION_DIVERGENT_VIEWS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this environment branch changes the authored node shape",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosHydrationDivergentViews: Initial view parity policy
// -----------------------------------------------------------------------------

/// Environment selected when a compile-time predicate evaluates to true.
#[derive(Clone, Copy)]
enum TrueEnvironment {
    /// The browser/hydrate build selects the `then` branch.
    Browser,
    /// The server build selects the `then` branch.
    Server,
}

impl TrueEnvironment {
    /// Reverses the selected environment for a logical negation.
    const fn inverted(self) -> Self {
        match self {
            Self::Browser => Self::Server,
            Self::Server => Self::Browser,
        }
    }
}

/// Collects hydration-relevant structure from authored `view!` invocations.
#[derive(Default)]
struct ViewShape {
    /// Whether at least one Leptos view macro was parsed.
    is_found: bool,
    /// Preorder node stream with explicit element boundaries.
    nodes: Vec<String>,
}

impl ViewShape {
    /// Appends one parsed rstml node tree without inspecting literals as markup.
    fn collect_nodes(&mut self, nodes: &[Node]) {
        for node in nodes {
            match node {
                Node::Element(element) => {
                    let name = element.name().to_string();
                    self.nodes.push(format!("<{name}>"));
                    self.collect_nodes(&element.children);
                    self.nodes.push(format!("</{name}>"));
                }
                Node::Fragment(NodeFragment { children, .. }) => self.collect_nodes(children),
                Node::Text(_) | Node::RawText(_) => self.nodes.push("#text".to_owned()),
                Node::Block(_) => self.nodes.push("#dynamic".to_owned()),
                Node::Doctype(_) => self.nodes.push("#doctype".to_owned()),
                Node::Comment(_) | Node::Custom(_) => {}
            }
        }
    }
}

impl<'ast> Visit<'ast> for ViewShape {
    fn visit_expr_macro(&mut self, expression: &'ast ExprMacro) {
        let is_view = expression
            .mac
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "view");
        if is_view {
            let parser = rstml::Parser::new(rstml::ParserConfig::default().recover_block(true));
            let (nodes, errors) = parser
                .parse_recoverable(expression.mac.tokens.clone())
                .split_vec();
            if errors.is_empty() {
                self.is_found = true;
                self.collect_nodes(&nodes);
            }
        }
        visit::visit_expr_macro(self, expression);
    }
}

/// Rejects environment branches that produce incompatible hydration structures.
struct LeptosHydrationDivergentViews;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_HYDRATION_DIVERGENT_VIEWS,
    Warn,
    "rejects environment-dependent initial Leptos view shapes",
    LeptosHydrationDivergentViews
}

impl LeptosHydrationDivergentViews {
    /// Classifies one name-value `cfg!` predicate by its true environment.
    fn name_value_environment(name_value: &syn::MetaNameValue) -> Option<TrueEnvironment> {
        // Environment predicates require a literal comparison value.
        let syn::Expr::Lit(value) = &name_value.value else {
            return None;
        };

        // Only string-valued target and feature predicates are supported.
        let syn::Lit::Str(value) = &value.lit else {
            return None;
        };
        match (
            name_value
                .path
                .get_ident()
                .map(ToString::to_string)
                .as_deref(),
            value.value().as_str(),
        ) {
            (Some("target_arch"), "wasm32") | (Some("feature"), "hydrate") => {
                Some(TrueEnvironment::Browser)
            }
            (Some("feature"), "ssr") => Some(TrueEnvironment::Server),
            _ => None,
        }
    }

    /// Classifies one supported `cfg!` predicate by its true environment.
    fn environment_meta(meta: &Meta) -> Option<TrueEnvironment> {
        match meta {
            Meta::NameValue(name_value) => Self::name_value_environment(name_value),
            Meta::List(list) if list.path.is_ident("not") => {
                // Malformed negated metadata cannot identify an environment.
                let Ok(nested) = syn::parse2::<Meta>(list.tokens.clone()) else {
                    return None;
                };
                Self::environment_meta(&nested).map(TrueEnvironment::inverted)
            }
            Meta::List(list) if list.path.is_ident("all") || list.path.is_ident("any") => {
                // Malformed compound metadata cannot be classified consistently.
                let Ok(nested) =
                    Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())
                else {
                    return None;
                };
                let classified = nested
                    .iter()
                    .map(Self::environment_meta)
                    .collect::<Option<Vec<_>>>()?;
                let mut environments = classified.into_iter();
                let environment = environments.next()?;
                environments
                    .all(|candidate| discriminant(&candidate) == discriminant(&environment))
                    .then_some(environment)
            }
            _ => None,
        }
    }

    /// Recognizes a condition that selects server or browser execution.
    fn environment_condition(source: &str) -> Option<TrueEnvironment> {
        fn classify(expression: &syn::Expr) -> Option<TrueEnvironment> {
            match expression {
                syn::Expr::Macro(expression) if expression.mac.path.is_ident("cfg") => {
                    // Invalid configuration syntax has no reliable environment meaning.
                    let Ok(meta) = syn::parse2::<Meta>(expression.mac.tokens.clone()) else {
                        return None;
                    };
                    LeptosHydrationDivergentViews::environment_meta(&meta)
                }
                syn::Expr::Unary(expression) if matches!(expression.op, UnOp::Not(_)) => {
                    classify(&expression.expr).map(TrueEnvironment::inverted)
                }
                syn::Expr::Group(expression) => classify(&expression.expr),
                syn::Expr::Paren(expression) => classify(&expression.expr),
                _ => None,
            }
        }

        // Unparseable conditions cannot be matched to supported configuration forms.
        let Ok(expression) = syn::parse_str(source) else {
            return None;
        };
        classify(&expression)
    }

    /// Reduces one branch to the element and text structure relevant to hydration.
    fn view_shape(source: &str) -> Option<Vec<String>> {
        // An unparseable branch cannot provide a trustworthy view shape.
        let Ok(expression) = syn::parse_str::<syn::Expr>(source) else {
            return None;
        };
        let mut shape = ViewShape::default();
        shape.visit_expr(&expression);
        shape.is_found.then_some(shape.nodes)
    }
}

impl LateLintPass<'_> for LeptosHydrationDivergentViews {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Macro-generated branches are not authored hydration decisions.
        if expression.span.from_expansion() {
            return;
        }

        // Hydration divergence requires two explicit conditional branches.
        let ExprKind::If(condition, then_branch, Some(else_branch)) = expression.kind else {
            return;
        };
        let source_map = cx.sess().source_map();

        // Missing authored condition text prevents configuration classification.
        let Ok(condition_source) = source_map.span_to_snippet(condition.span) else {
            return;
        };

        // Ignore conditions that do not select a supported execution environment.
        let Some(true_environment) = Self::environment_condition(&condition_source) else {
            return;
        };

        // Missing authored then-branch text prevents shape comparison.
        let Ok(then_source) = source_map.span_to_snippet(then_branch.span) else {
            return;
        };

        // Missing authored else-branch text prevents shape comparison.
        let Ok(else_source) = source_map.span_to_snippet(else_branch.span) else {
            return;
        };

        // Both branches must contain a recognizable Leptos view.
        let (Some(then_shape), Some(else_shape)) = (
            Self::view_shape(&then_source),
            Self::view_shape(&else_source),
        ) else {
            return;
        };

        // Equal initial structures are safe to hydrate across environments.
        if then_shape == else_shape {
            return;
        }

        let (server_shape, browser_shape) = match true_environment {
            TrueEnvironment::Browser => (else_shape, then_shape),
            TrueEnvironment::Server => (then_shape, else_shape),
        };
        Violation {
            owner: expression.hir_id,
            span: expression.span,
            server_shape,
            browser_shape,
        }
        .emit(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::LeptosHydrationDivergentViews;

    #[test]
    fn compares_element_identity_without_text_content() {
        assert_eq!(
            LeptosHydrationDivergentViews::view_shape("view! { <main><ClientToolbar/></main> }")
                .unwrap(),
            ["<main>", "<ClientToolbar>", "</ClientToolbar>", "</main>"]
        );
    }

    #[test]
    fn preserves_nesting_and_ignores_markup_inside_literals() {
        assert_ne!(
            LeptosHydrationDivergentViews::view_shape("view! { <main><span/></main> }").unwrap(),
            LeptosHydrationDivergentViews::view_shape("view! { <main/><span/> }").unwrap()
        );
        assert_eq!(
            LeptosHydrationDivergentViews::view_shape(
                r#"view! { <main data-label="<NotAnElement/>"/> }"#
            )
            .unwrap(),
            ["<main>", "</main>"]
        );
    }
}
