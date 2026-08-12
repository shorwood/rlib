extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::HashSet;
use std::ops::Range;

use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use rstml::node::{Node, NodeAttribute, NodeElement, NodeFragment};
use rustc_hir::{Expr, HirId};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::hygiene::{ExpnKind, MacroKind};
use rustc_span::{BytePos, Span};
use serde::Deserialize;
use syn::ExprMacro;
use syn::spanned::Spanned;

use crate::utils::config::LibraryConfig;

// -----------------------------------------------------------------------------
// LeptosViewStructureConfig: Authored view layout policy
// -----------------------------------------------------------------------------

/// Complexity and comment syntax shared by the Leptos view-section lint family.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LeptosViewStructureConfig {
    /// Ordinary line-comment prefix introducing a view section.
    pub(crate) view_section_comment_prefix: String,
    /// Maximum direct complexity permitted without named sections.
    pub(crate) max_unnamed_view_complexity: usize,
    /// Maximum direct complexity permitted in one named section.
    pub(crate) max_view_section_complexity: usize,
}

impl Default for LeptosViewStructureConfig {
    fn default() -> Self {
        Self {
            view_section_comment_prefix: "//".to_owned(),
            max_unnamed_view_complexity: 4,
            max_view_section_complexity: 4,
        }
    }
}

impl LeptosViewStructureConfig {
    /// Loads and validates the configured view-structure policy.
    pub(crate) fn from_config() -> Self {
        let config = LibraryConfig::load().leptos_view_structure;
        config.validate().unwrap_or_else(|message| {
            panic!("invalid Leptos view structure configuration: {message}")
        });
        config
    }

    /// Rejects ineffective limits and prefixes that are not ordinary comments.
    fn validate(&self) -> Result<(), String> {
        if self.max_unnamed_view_complexity == 0 || self.max_view_section_complexity == 0 {
            return Err("Leptos view structure complexity limits must be greater than zero".into());
        }
        let prefix = &self.view_section_comment_prefix;
        if prefix.trim() != prefix
            || prefix.contains(['\n', '\r'])
            || !prefix.starts_with("//")
            || prefix.starts_with("///")
            || prefix.starts_with("//!")
        {
            return Err(
                "leptos_view_structure.view_section_comment_prefix must be one trimmed ordinary `//` comment prefix"
                    .into(),
            );
        }
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// ViewStructureAnalysis: Authored view scopes and boundaries
// -----------------------------------------------------------------------------

/// One direct child in an authored sibling scope.
#[derive(Clone)]
pub(crate) struct ViewNode {
    /// Exact authored source range of the construct.
    pub(crate) span: Span,
    /// Source-relative range used to associate comments.
    range: Range<usize>,
    /// Direct navigation complexity contributed by the construct.
    pub(crate) complexity: usize,
    /// Authored opening-tag name, when this is an element or component.
    pub(crate) name: Option<String>,
    /// Visible literal text directly owned by the node.
    pub(crate) literal: Option<String>,
}

/// One ordinary comment positioned at a direct-child boundary.
#[derive(Clone)]
pub(crate) struct ViewHeading {
    /// Exact source range of the comment.
    pub(crate) span: Span,
    /// Complete authored comment token.
    pub(crate) text: String,
    /// Index of the direct node this comment precedes, if attached.
    pub(crate) node: Option<usize>,
    /// Whether a blank source line separates this heading from prior content.
    pub(crate) blank_before: bool,
    /// Whether the comment immediately precedes its node.
    pub(crate) immediately_precedes: bool,
}

/// One independently analyzed direct sibling list.
#[derive(Default)]
pub(crate) struct ViewScope {
    /// Direct children in authored order.
    pub(crate) nodes: Vec<ViewNode>,
    /// Direct-boundary ordinary comments in authored order.
    pub(crate) headings: Vec<ViewHeading>,
}

impl ViewScope {
    /// Returns the total direct complexity of this scope.
    pub(crate) fn complexity(&self) -> usize {
        self.nodes.iter().map(|node| node.complexity).sum()
    }
}

/// Parsed authored structure for one `view!` call.
pub(crate) struct ViewStructureAnalysis {
    /// Expanded expression used to honor local lint attributes.
    pub(crate) owner: HirId,
    /// Direct sibling scopes found in the authored view.
    pub(crate) scopes: Vec<ViewScope>,
}

/// Stateful deduplication shared by individual view lint passes.
#[derive(Default)]
pub(crate) struct ViewCallSites {
    /// Source positions already analyzed for this pass.
    seen: HashSet<(u32, u32)>,
}

impl ViewCallSites {
    /// Parses an authored `view!` call once for the current lint pass.
    pub(crate) fn analyze(
        &mut self,
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
    ) -> Option<ViewStructureAnalysis> {
        let span = expression.span.macro_backtrace().find_map(|expansion| {
            matches!(expansion.kind, ExpnKind::Macro(MacroKind::Bang, name) if name.as_str() == "view")
                .then_some(expansion.call_site)
        })?;
        if !self.seen.insert((span.lo().0, span.hi().0)) {
            return None;
        }
        let source = cx.sess().source_map().span_to_snippet(span).ok()?;
        ViewStructureAnalysis::parse(span, expression.hir_id, &source)
    }
}

impl ViewStructureAnalysis {
    /// Parses the same RSX token tree as Leptos and overlays authored Rust comments.
    fn parse(span: Span, owner: HirId, source: &str) -> Option<Self> {
        let expression = syn::parse_str::<ExprMacro>(source).ok()?;
        let bounds = body_range(&expression);
        let tokens = without_global_class(expression.mac.tokens);
        let parser = rstml::Parser::new(rstml::ParserConfig::default().recover_block(true));
        let (nodes, errors) = parser.parse_recoverable(tokens).split_vec();
        if !errors.is_empty() {
            return None;
        }

        // Build structural scopes exclusively from rstml's Leptos-compatible node tree.
        let comments = SourceComment::collect(source);
        let mut builder = ViewScopeBuilder {
            source,
            span,
            comments: &comments,
            scopes: Vec::new(),
        };
        builder.collect_scope(&nodes, bounds);
        Some(Self {
            owner,
            scopes: builder.scopes,
        })
    }
}

// -----------------------------------------------------------------------------
// ViewScopeBuilder: rstml tree projection
// -----------------------------------------------------------------------------

/// Projects rstml nodes into the small stable model used by the lint family.
struct ViewScopeBuilder<'source> {
    /// Complete authored macro invocation.
    source: &'source str,
    /// Rustc span corresponding to source byte zero.
    span: Span,
    /// Ordinary Rust comments retained separately from the token stream.
    comments: &'source [SourceComment],
    /// Completed sibling scopes.
    scopes: Vec<ViewScope>,
}

impl ViewScopeBuilder<'_> {
    /// Collects one sibling scope, flattening fragments but not element descendants.
    fn collect_scope(&mut self, nodes: &[Node], bounds: Range<usize>) {
        let mut direct = Vec::new();
        flatten_fragments(nodes, &mut direct);
        let projected = direct
            .iter()
            .filter_map(|node| self.project_node(node))
            .collect::<Vec<_>>();
        let headings = self.collect_headings(&projected, &bounds);
        self.scopes.push(ViewScope {
            nodes: projected,
            headings,
        });

        // Every element child list is an independent direct sibling scope.
        for node in direct {
            if let Node::Element(element) = node
                && !element.children.is_empty()
            {
                let start = element.open_tag.span().byte_range().end;
                let end = element
                    .close_tag
                    .as_ref()
                    .map_or(start, |tag| tag.span().byte_range().start);
                self.collect_scope(&element.children, start..end);
            }
        }
    }

    /// Converts one structural rstml node into a direct-view participant.
    fn project_node(&self, node: &Node) -> Option<ViewNode> {
        let range = node.span().byte_range();
        if range.start >= range.end || range.end > self.source.len() {
            return None;
        }
        let (complexity, name, literal) = match node {
            Node::Element(element) => {
                let name = element.name().to_string();
                let complexity = usize::from(is_control_component(&name))
                    + 1
                    + usize::from(has_event_handler(element))
                    + usize::from(element.attributes().len() >= 6);
                (complexity, Some(name), direct_literal(element))
            }
            Node::Block(block) => {
                let source = block.to_token_stream().to_string();
                let complexity = usize::from(
                    source.trim_start().starts_with("if ")
                        || source.trim_start().starts_with("match "),
                ) + 1;
                (complexity, None, None)
            }
            Node::Comment(_) | Node::Doctype(_) | Node::Text(_) | Node::RawText(_) => return None,
            Node::Fragment(_) | Node::Custom(_) => return None,
        };
        Some(ViewNode {
            span: self.to_rustc_span(&range),
            range,
            complexity,
            name,
            literal,
        })
    }

    /// Associates ordinary comments found in direct-node gaps with the following node.
    fn collect_headings(&self, nodes: &[ViewNode], bounds: &Range<usize>) -> Vec<ViewHeading> {
        self.comments
            .iter()
            .filter_map(|comment| {
                let node = nodes
                    .iter()
                    .position(|node| comment.range.end <= node.range.start);
                let previous_end = node.map_or_else(
                    || nodes.last().map_or(bounds.start, |node| node.range.end),
                    |node| {
                        node.checked_sub(1)
                            .map_or(bounds.start, |previous| nodes[previous].range.end)
                    },
                );
                (comment.range.start >= previous_end).then(|| {
                    let before = &self.source[previous_end..comment.range.start];
                    let next_start = node.map_or(bounds.end, |node| nodes[node].range.start);
                    let after = &self.source[comment.range.end..next_start];
                    ViewHeading {
                        span: self.to_rustc_span(&comment.range),
                        text: comment.text.clone(),
                        node: node.filter(|_| after.trim().is_empty()),
                        blank_before: node == Some(0) || before.matches('\n').count() >= 2,
                        immediately_precedes: node.is_some()
                            && after.trim().is_empty()
                            && after.matches('\n').count() <= 1,
                    }
                })
            })
            .collect()
    }

    /// Maps one proc-macro source range back to the compiler's macro call-site span.
    fn to_rustc_span(&self, range: &Range<usize>) -> Span {
        let lo = BytePos(u32::try_from(range.start).expect("source offset fits"));
        let hi = BytePos(u32::try_from(range.end).expect("source offset fits"));
        self.span
            .with_lo(self.span.lo() + lo)
            .with_hi(self.span.lo() + hi)
    }
}

/// Recursively exposes fragment children in their containing sibling scope.
fn flatten_fragments<'node>(nodes: &'node [Node], direct: &mut Vec<&'node Node>) {
    for node in nodes {
        if let Node::Fragment(NodeFragment { children, .. }) = node {
            flatten_fragments(children, direct);
        } else {
            direct.push(node);
        }
    }
}

/// Mirrors Leptos's optional `class=value,` prelude before rstml parsing.
fn without_global_class(tokens: TokenStream) -> TokenStream {
    let mut tokens = tokens.into_iter();
    let first = tokens.next();
    let second = tokens.next();
    let third = tokens.next();
    let fourth = tokens.next();
    let has_global_class = matches!(
        (&first, &second, &fourth),
        (Some(TokenTree::Ident(name)), Some(TokenTree::Punct(eq)), Some(TokenTree::Punct(comma)))
            if name == "class" && eq.as_char() == '=' && comma.as_char() == ','
    );
    if has_global_class {
        tokens.collect()
    } else {
        [first, second, third, fourth]
            .into_iter()
            .flatten()
            .chain(tokens)
            .collect()
    }
}

/// Returns the source range inside the macro's outer delimiter.
fn body_range(expression: &ExprMacro) -> Range<usize> {
    let delimiter = expression.mac.delimiter.span();
    delimiter.open().byte_range().end..delimiter.close().byte_range().start
}

/// Gives control-flow components their higher direct-navigation weight.
fn is_control_component(name: &str) -> bool {
    matches!(name, "For" | "Show" | "Suspense" | "ErrorBoundary")
}

/// Returns whether an opening tag contains an authored Leptos event binding.
fn has_event_handler(element: &NodeElement<rstml::Infallible>) -> bool {
    element.attributes().iter().any(|attribute| {
        matches!(attribute, NodeAttribute::Attribute(attribute) if attribute.key.to_string().starts_with("on:"))
    })
}

/// Extracts one immediately owned quoted text node.
fn direct_literal(element: &NodeElement<rstml::Infallible>) -> Option<String> {
    let [Node::Text(text)] = element.children.as_slice() else {
        return None;
    };
    Some(text.value_string())
}

// -----------------------------------------------------------------------------
// SourceComment: Rust trivia overlay
// -----------------------------------------------------------------------------

/// Ordinary Rust line comment omitted from proc-macro token streams.
struct SourceComment {
    /// Source-relative byte range.
    range: Range<usize>,
    /// Complete authored token text.
    text: String,
}

impl SourceComment {
    /// Lexes comments without interpreting any RSX grammar.
    fn collect(source: &str) -> Vec<Self> {
        let mut comments = Vec::new();
        let mut offset = 0_usize;
        for token in tokenize(source, FrontmatterAllowed::No) {
            let length = usize::try_from(token.len).expect("token length fits");
            let end = offset + length;
            if matches!(
                token.kind,
                TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
            ) {
                comments.push(Self {
                    range: offset..end,
                    text: source[offset..end].to_owned(),
                });
            }
            offset = end;
        }
        comments
    }
}

// -----------------------------------------------------------------------------
// Tests: rstml projection and configuration
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::rustc_hir::CRATE_HIR_ID;
    use super::rustc_span::{BytePos, Span};
    use super::{LeptosViewStructureConfig, ViewStructureAnalysis};

    fn parse(source: &str) -> ViewStructureAnalysis {
        ViewStructureAnalysis::parse(
            Span::with_root_ctxt(BytePos(0), BytePos(source.len() as u32)),
            CRATE_HIR_ID,
            source,
        )
        .expect("representative Leptos view should parse")
    }

    #[test]
    fn delegates_nested_elements_blocks_and_attributes_to_rstml() {
        let parsed = parse(
            r#"view! { <main><Header/><Show when=yes on:click=run><Body/></Show>{value}</main> }"#,
        );
        let complexities = parsed
            .scopes
            .iter()
            .map(|scope| scope.complexity())
            .collect::<Vec<_>>();
        assert_eq!(parsed.scopes[0].complexity(), 1, "{complexities:?}");
        assert_eq!(parsed.scopes[1].complexity(), 5);
        assert_eq!(parsed.scopes[2].complexity(), 1);
    }

    #[test]
    fn fragments_are_transparent_and_comments_attach_to_nodes() {
        let parsed = parse("view! { <>// Account navigation\n<Nav/><Crumbs/></> }");
        assert_eq!(parsed.scopes[0].nodes.len(), 2);
        assert_eq!(parsed.scopes[0].headings[0].node, Some(0));
    }

    #[test]
    fn retains_stranded_direct_boundary_comments() {
        let parsed = parse("view! { <main><Content/> // Stranded heading\n</main> }");
        let scope = &parsed.scopes[1];
        assert_eq!(scope.headings.len(), 1);
        assert_eq!(scope.headings[0].node, None);
        assert!(!scope.headings[0].immediately_precedes);
    }

    #[test]
    fn rust_blocks_can_contain_markup_like_strings_and_nested_macros() {
        let parsed = parse(r#"view! { {format!("<Fake/>")} <Real value=move || call()/> }"#);
        assert_eq!(parsed.scopes[0].nodes.len(), 2);
        assert_eq!(parsed.scopes[0].nodes[1].name.as_deref(), Some("Real"));
    }

    #[test]
    fn mirrors_global_class_prelude_and_preserves_source_spans() {
        let source = r#"view! { class="shell", <main><Child/></main> }"#;
        let parsed = parse(source);
        let main = &parsed.scopes[0].nodes[0];
        assert_eq!(&source[main.range.clone()], "<main><Child/></main>");
    }

    #[test]
    fn validates_configured_limits_and_prefixes() {
        assert!(LeptosViewStructureConfig::default().validate().is_ok());
        assert!(
            LeptosViewStructureConfig {
                max_unnamed_view_complexity: 0,
                ..LeptosViewStructureConfig::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            LeptosViewStructureConfig {
                view_section_comment_prefix: "///".into(),
                ..LeptosViewStructureConfig::default()
            }
            .validate()
            .is_err()
        );
    }
}
