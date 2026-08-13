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
use crate::utils::function_layout_prose::FunctionLayoutProse;

#[derive(Clone, Deserialize)]
/// Complexity and comment syntax shared by the Leptos view-section lint family.
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct LeptosViewStructureConfig {
    /// Ordinary line-comment prefix introducing a view section.
    pub(crate) view_section_comment_prefix: String,
    /// Maximum direct complexity permitted without named sections.
    pub(crate) max_unnamed_view_complexity: usize,
    /// Maximum direct complexity permitted in one named section.
    pub(crate) max_view_section_complexity: usize,
    /// Maximum direct attribute complexity permitted without named groups.
    pub(crate) max_unnamed_view_attribute_complexity: usize,
    /// Maximum complexity permitted in one named attribute group.
    pub(crate) max_view_attribute_group_complexity: usize,
}

impl Default for LeptosViewStructureConfig {
    fn default() -> Self {
        Self {
            view_section_comment_prefix: "//".to_owned(),
            max_unnamed_view_complexity: 4,
            max_view_section_complexity: 4,
            max_unnamed_view_attribute_complexity: 6,
            max_view_attribute_group_complexity: 4,
        }
    }
}

impl LeptosViewStructureConfig {
    /// Rejects ineffective limits and prefixes that are not ordinary comments.
    fn validate(&self) -> Result<(), String> {
        // Reject inputs that do not satisfy this stage.
        if self.max_unnamed_view_complexity == 0
            || self.max_view_section_complexity == 0
            || self.max_unnamed_view_attribute_complexity == 0
            || self.max_view_attribute_group_complexity == 0
        {
            return Err("Leptos view structure complexity limits must be greater than zero".into());
        }

        // Prepare the values used by this stage.
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

    /// Loads and validates the configured view-structure policy.
    pub(crate) fn from_config() -> Self {
        let config = LibraryConfig::load().leptos_view_structure;
        config.validate().unwrap_or_else(|message| {
            panic!("invalid Leptos view structure configuration: {message}")
        });
        config
    }
}

#[derive(Clone)]
/// One direct child in an authored sibling scope.
pub struct ViewNode {
    /// Exact authored source range of the construct.
    pub(crate) span: Span,
    /// Source-relative range used to associate comments.
    range: Range<usize>,
    /// Direct navigation complexity contributed by the construct.
    complexity: usize,
    /// Authored opening-tag name, when this is an element or component.
    pub(crate) name: Option<String>,
    /// Visible literal text directly owned by the node.
    pub(crate) literal: Option<String>,
}

#[cfg(test)]
impl ViewNode {
    /// Builds a minimal node for lint behavior tests.
    pub(crate) fn for_test(name: &str, literal: Option<&str>) -> Self {
        Self {
            span: rustc_span::DUMMY_SP,
            range: 0..1,
            complexity: 1,
            name: Some(name.to_owned()),
            literal: literal.map(str::to_owned),
        }
    }
}

#[derive(Clone)]
/// One ordinary comment positioned at a direct-child boundary.
pub struct ViewHeading {
    /// Exact source range of the comment.
    pub(crate) span: Span,
    /// Complete authored comment token.
    pub(crate) text: String,
    /// Index of the direct node this comment precedes, if attached.
    pub(crate) node: Option<usize>,
    /// Whether a blank source line separates this heading from prior content.
    pub(crate) has_blank_before: bool,
    /// Whether the comment immediately precedes its node.
    pub(crate) is_immediately_preceding: bool,
}

impl ViewHeading {
    /// Returns canonical section prose after the configured comment marker.
    pub(crate) fn canonical_content<'heading>(
        &'heading self,
        config: &LeptosViewStructureConfig,
    ) -> Option<&'heading str> {
        // Prepare the values used by this stage.
        let content = self
            .text
            .strip_prefix(&config.view_section_comment_prefix)?
            .strip_prefix(' ')?;

        // Perform the next step of the analysis.
        (self.text.starts_with("//")
            && !self.text.contains('\n')
            && !content.ends_with([':', '.', ';', '!', '?', ',', '-'])
            && !content.starts_with(['-', '*', '#'])
            && FunctionLayoutProse::is_canonical(Some(content))
            && self.node.is_some()
            && self.is_immediately_preceding
            && (self.node == Some(0) || self.has_blank_before))
            .then_some(content)
    }
}

#[derive(Default)]
/// One independently analyzed direct sibling list.
pub struct ViewScope {
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

    /// Partitions canonical named regions without reinterpreting rstml structure.
    pub(crate) fn sections(&self, config: &LeptosViewStructureConfig) -> Vec<ViewSection<'_>> {
        // Prepare the values used by this stage.
        let starts = self
            .headings
            .iter()
            .filter_map(|heading| {
                heading
                    .canonical_content(config)
                    .and_then(|_| heading.node.map(|node| (heading, node)))
            })
            .collect::<Vec<_>>();

        // Perform the next step of the analysis.
        starts
            .iter()
            .enumerate()
            .map(|(index, (heading, start))| {
                let end = starts
                    .get(index + 1)
                    .map_or(self.nodes.len(), |(_, node)| *node);
                ViewSection {
                    heading,
                    nodes: &self.nodes[*start..end],
                }
            })
            .collect()
    }
}

/// Parsed authored structure for one `view!` call.
pub struct Analysis {
    /// Expanded expression used to honor local lint attributes.
    pub(crate) owner: HirId,
    /// Direct sibling scopes found in the authored view.
    pub(crate) scopes: Vec<ViewScope>,
    /// Opening tags and their authored attributes.
    pub(crate) elements: Vec<ViewElement>,
}

impl Analysis {
    /// Parses the same RSX token tree as Leptos and overlays authored Rust comments.
    fn parse(span: Span, owner: HirId, source: &str) -> Option<Self> {
        // Prepare the values used by this stage.
        let expression = match syn::parse_str::<ExprMacro>(source) {
            Ok(expression) => expression,
            Err(_error) => return None,
        };
        let bounds = body_range(&expression);
        let tokens = without_global_class(expression.mac.tokens);
        let parser = rstml::Parser::new(rstml::ParserConfig::default().recover_block(true));

        // Prepare the values used by this stage.
        let (nodes, errors) = parser.parse_recoverable(tokens).split_vec();
        if !errors.is_empty() {
            return None;
        }

        // Build structural scopes exclusively from rstml's Leptos-compatible node tree.
        let comments = SourceComment::collect(source);

        // Prepare the values used by this stage.
        let mut builder = ViewScopeBuilder {
            source,
            span,
            comments: &comments,
            scopes: Vec::new(),
            elements: Vec::new(),
        };

        // Perform the next step of the analysis.
        builder.collect_scope(&nodes, bounds);
        Some(Self {
            owner,
            scopes: builder.scopes,
            elements: builder.elements,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// Stable semantic category for an authored attribute or component prop.
pub enum ViewAttributeCategory {
    /// Represents the `Identity` case.
    Identity,
    /// Represents the `Accessibility` case.
    Accessibility,
    /// Represents the `State` case.
    State,
    /// Represents the `Presentation` case.
    Presentation,
    /// Represents the `Data` case.
    Data,
    /// Represents the `Behavior` case.
    Behavior,
    /// Represents the `Integration` case.
    Integration,
    /// Represents the `Other` case.
    Other,
}

impl ViewAttributeCategory {
    /// Classifies stable Leptos/HTML attribute namespaces without guessing value semantics.
    fn attribute_category(name: &str) -> Self {
        // Recognize accessibility attributes.
        if name == "role" || name == "alt" || name.starts_with("aria-") || name.starts_with("aria:")
        {
            return Self::Accessibility;
        }

        // Recognize visual presentation attributes.
        if name == "class"
            || name == "style"
            || name.starts_with("class:")
            || name.starts_with("style:")
        {
            return Self::Presentation;
        }

        // Recognize behavior, data, and integration namespaces.
        if name.starts_with("on:") || name.starts_with("on_") {
            return Self::Behavior;
        }
        if name.starts_with("data-") || name.starts_with("data:") {
            return Self::Data;
        }
        if name == "node_ref" || name.starts_with("use:") || name.starts_with("attr:") {
            return Self::Integration;
        }

        // Recognize controlled state attributes.
        if matches!(
            name,
            "value" | "checked" | "disabled" | "hidden" | "selected" | "readonly" | "required"
        ) || name.starts_with("prop:")
        {
            return Self::State;
        }

        // Classify stable identity attributes, leaving unknown names unclassified.
        if matches!(
            name,
            "id" | "name" | "type" | "href" | "action" | "form" | "method"
        ) {
            Self::Identity
        } else {
            Self::Other
        }
    }
}

#[derive(Clone)]
/// One direct attribute in an authored opening tag.
pub struct ViewAttribute {
    /// Stores the `name` value used by this analysis.
    pub(crate) name: String,
    /// Stores the `span` value used by this analysis.
    pub(crate) span: Span,
    /// Stores the `range` value used by this analysis.
    range: Range<usize>,
    /// Stores the `category` value used by this analysis.
    pub(crate) category: ViewAttributeCategory,
    /// Stores the `complexity` value used by this analysis.
    complexity: usize,
}

/// One opening tag with its direct attributes and boundary comments.
pub struct ViewElement {
    /// Stores the `name` value used by this analysis.
    pub(crate) name: String,
    /// Stores the `span` value used by this analysis.
    pub(crate) span: Span,
    /// Stores the `attributes` value used by this analysis.
    attributes: Vec<ViewAttribute>,
    /// Stores the `headings` value used by this analysis.
    headings: Vec<ViewHeading>,
}

impl ViewElement {
    /// Performs the `complexity` operation for this value.
    pub(crate) fn complexity(&self) -> usize {
        self.attributes
            .iter()
            .map(|attribute| attribute.complexity)
            .sum()
    }

    /// Performs the `category_count` operation for this value.
    pub(crate) fn category_count(&self) -> usize {
        self.attributes
            .iter()
            .map(|attribute| attribute.category)
            .collect::<HashSet<_>>()
            .len()
    }

    /// Performs the `group_count` operation for this value.
    pub(crate) fn group_count(&self, config: &LeptosViewStructureConfig) -> usize {
        self.headings
            .iter()
            .filter_map(|heading| {
                heading
                    .canonical_content(config)
                    .and_then(|_| heading.node.map(|attribute| (heading, attribute)))
            })
            .count()
    }

    /// Performs the `groups` operation for this value.
    pub(crate) fn groups(&self, config: &LeptosViewStructureConfig) -> Vec<ViewAttributeGroup<'_>> {
        // Prepare the values used by this stage.
        let starts = self
            .headings
            .iter()
            .filter_map(|heading| {
                heading
                    .canonical_content(config)
                    .and_then(|_| heading.node.map(|attribute| (heading, attribute)))
            })
            .collect::<Vec<_>>();

        // Perform the next step of the analysis.
        starts
            .iter()
            .enumerate()
            .map(|(index, (heading, start))| {
                let end = starts
                    .get(index + 1)
                    .map_or(self.attributes.len(), |(_, attribute)| *attribute);
                ViewAttributeGroup {
                    heading,
                    attributes: &self.attributes[*start..end],
                }
            })
            .collect()
    }
}

/// Nodes introduced by one canonical heading in a direct sibling scope.
pub struct ViewSection<'scope> {
    /// Canonical heading that names this section.
    pub(crate) heading: &'scope ViewHeading,
    /// Consecutive direct nodes owned by the heading.
    pub(crate) nodes: &'scope [ViewNode],
}

impl ViewSection<'_> {
    /// Returns the direct-navigation complexity owned by this section.
    pub(crate) fn complexity(&self) -> usize {
        self.nodes.iter().map(|node| node.complexity).sum()
    }
}

/// Stable source coordinates used to deduplicate expanded view calls.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SourceRange {
    /// Inclusive low byte position.
    lo: u32,
    /// Exclusive high byte position.
    hi: u32,
}

#[derive(Default)]
/// Stateful deduplication shared by individual view lint passes.
pub struct ViewCallSites {
    /// Source positions already analyzed for this pass.
    seen: HashSet<SourceRange>,
}

impl ViewCallSites {
    /// Parses an authored `view!` call once for the current lint pass.
    pub(crate) fn analyze(
        &mut self,
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
    ) -> Option<Analysis> {
        // Prepare the values used by this stage.
        let span = expression.span.macro_backtrace().find_map(|expansion| {
            matches!(expansion.kind, ExpnKind::Macro(MacroKind::Bang, name) if name.as_str() == "view")
                .then_some(expansion.call_site)
        })?;
        if !self.seen.insert(SourceRange {
            lo: span.lo().0,
            hi: span.hi().0,
        }) {
            return None;
        }

        // Prepare the values used by this stage.
        let source = match cx.sess().source_map().span_to_snippet(span) {
            Ok(source) => source,
            Err(_error) => return None,
        };
        Analysis::parse(span, expression.hir_id, &source)
    }
}

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
    /// Completed authored opening tags.
    elements: Vec<ViewElement>,
}

impl ViewScopeBuilder<'_> {
    /// Maps one proc-macro source range back to the compiler's macro call-site span.
    fn to_rustc_span(&self, range: &Range<usize>) -> Span {
        let lo = BytePos(u32::try_from(range.start).expect("source offset fits"));
        let hi = BytePos(u32::try_from(range.end).expect("source offset fits"));
        self.span
            .with_lo(self.span.lo() + lo)
            .with_hi(self.span.lo() + hi)
    }

    /// Converts one structural rstml node into a direct-view participant.
    fn project_node(&self, node: &Node) -> Option<ViewNode> {
        // Prepare the values used by this stage.
        let range = node.span().byte_range();
        if range.start >= range.end || range.end > self.source.len() {
            return None;
        }

        // Prepare the values used by this stage.
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
            Node::Comment(_)
            | Node::Doctype(_)
            | Node::Text(_)
            | Node::RawText(_)
            | Node::Fragment(_)
            | Node::Custom(_) => return None,
        };

        // Return the completed analysis result.
        Some(ViewNode {
            span: self.to_rustc_span(&range),
            range,
            complexity,
            name,
            literal,
        })
    }

    /// Associates ordinary comments with the next authored range in one structural scope.
    fn collect_headings_for_ranges(
        &self,
        ranges: &[Range<usize>],
        bounds: &Range<usize>,
    ) -> Vec<ViewHeading> {
        self.comments
            .iter()
            .filter(|comment| {
                comment.range.start >= bounds.start && comment.range.end <= bounds.end
            })
            .filter_map(|comment| {
                let node = ranges
                    .iter()
                    .position(|range| comment.range.end <= range.start);
                let previous_end = node.map_or_else(
                    || ranges.last().map_or(bounds.start, |range| range.end),
                    |node| {
                        node.checked_sub(1)
                            .map_or(bounds.start, |previous| ranges[previous].end)
                    },
                );
                (comment.range.start >= previous_end).then(|| {
                    let before = &self.source[previous_end..comment.range.start];
                    let next_start = node.map_or(bounds.end, |node| ranges[node].start);
                    let after = &self.source[comment.range.end..next_start];
                    ViewHeading {
                        span: self.to_rustc_span(&comment.range),
                        text: comment.text.clone(),
                        node: node.filter(|_| after.trim().is_empty()),
                        has_blank_before: node == Some(0) || before.matches('\n').count() >= 2,
                        is_immediately_preceding: node.is_some()
                            && after.trim().is_empty()
                            && after.matches('\n').count() <= 1,
                    }
                })
            })
            .collect()
    }

    /// Projects one opening tag into the shared attribute-group model.
    fn collect_element(&mut self, element: &NodeElement<rstml::Infallible>) {
        // Prepare the values used by this stage.
        let attributes = element
            .attributes()
            .iter()
            .filter_map(|attribute| {
                let range = attribute.span().byte_range();
                if range.start >= range.end || range.end > self.source.len() {
                    return None;
                }
                let (name, category, complexity) = match attribute {
                    NodeAttribute::Attribute(attribute) => {
                        let name = attribute.key.to_string();
                        let category = ViewAttributeCategory::attribute_category(&name);
                        let complexity =
                            1 + usize::from(category == ViewAttributeCategory::Behavior);
                        (name, category, complexity)
                    }
                    NodeAttribute::Block(_) => {
                        ("spread".to_owned(), ViewAttributeCategory::Integration, 2)
                    }
                };
                Some(ViewAttribute {
                    name,
                    span: self.to_rustc_span(&range),
                    range,
                    category,
                    complexity,
                })
            })
            .collect::<Vec<_>>();

        // Reject inputs that do not satisfy this stage.
        if attributes.is_empty() {
            return;
        }
        let opening = element.open_tag.span().byte_range();
        let bounds = opening.clone();

        // Prepare the values used by this stage.
        let headings = self.collect_headings_for_ranges(
            &attributes
                .iter()
                .map(|attribute| attribute.range.clone())
                .collect::<Vec<_>>(),
            &bounds,
        );

        // Update the accumulated analysis state.
        self.elements.push(ViewElement {
            name: element.name().to_string(),
            span: self.to_rustc_span(&opening),
            attributes,
            headings,
        });
    }

    /// Associates ordinary comments found in direct-node gaps with the following node.
    fn collect_headings(&self, nodes: &[ViewNode], bounds: &Range<usize>) -> Vec<ViewHeading> {
        self.collect_headings_for_ranges(
            &nodes
                .iter()
                .map(|node| node.range.clone())
                .collect::<Vec<_>>(),
            bounds,
        )
    }

    /// Collects one sibling scope, flattening fragments but not element descendants.
    fn collect_scope(&mut self, nodes: &[Node], bounds: Range<usize>) {
        // Prepare the values used by this stage.
        let mut direct = Vec::new();
        flatten_fragments(nodes, &mut direct);
        let projected = direct
            .iter()
            .filter_map(|node| self.project_node(node))
            .collect::<Vec<_>>();
        let headings = self.collect_headings(&projected, &bounds);

        // Update the accumulated analysis state.
        self.scopes.push(ViewScope {
            nodes: projected,
            headings,
        });

        // Every element child list is an independent direct sibling scope.
        for node in direct {
            let Node::Element(element) = node else {
                continue;
            };
            self.collect_element(element);
            if element.children.is_empty() {
                continue;
            }
            let start = element.open_tag.span().byte_range().end;
            let end = element
                .close_tag
                .as_ref()
                .map_or(start, |tag| tag.span().byte_range().start);
            self.collect_scope(&element.children, start..end);
        }
    }
}

/// Consecutive attributes introduced by one canonical heading.
pub struct ViewAttributeGroup<'element> {
    /// Stores the `heading` value used by this analysis.
    pub(crate) heading: &'element ViewHeading,
    /// Stores the `attributes` value used by this analysis.
    pub(crate) attributes: &'element [ViewAttribute],
}

impl ViewAttributeGroup<'_> {
    /// Performs the `complexity` operation for this value.
    pub(crate) fn complexity(&self) -> usize {
        self.attributes
            .iter()
            .map(|attribute| attribute.complexity)
            .sum()
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
    // Prepare the values used by this stage.
    let mut tokens = tokens.into_iter();
    let first = tokens.next();
    let second = tokens.next();
    let third = tokens.next();
    let fourth = tokens.next();

    // Prepare the values used by this stage.
    let has_global_class = matches!(
        (&first, &second, &fourth),
        (Some(TokenTree::Ident(name)), Some(TokenTree::Punct(eq)), Some(TokenTree::Punct(comma)))
            if name == "class" && eq.as_char() == '=' && comma.as_char() == ','
    );

    // Reject inputs that do not satisfy this stage.
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

#[cfg(test)]
mod tests {
    use super::rustc_hir::CRATE_HIR_ID;
    use super::rustc_span::{BytePos, Span};
    use super::{Analysis, LeptosViewStructureConfig};

    fn parse(source: &str) -> Analysis {
        Analysis::parse(
            Span::with_root_ctxt(
                BytePos(0),
                BytePos(u32::try_from(source.len()).expect("test view fits in a source span")),
            ),
            CRATE_HIR_ID,
            source,
        )
        .expect("representative Leptos view should parse")
    }

    #[test]
    fn delegates_nested_elements_blocks_and_attributes_to_rstml() {
        let parsed = parse(
            "view! { <main><Header/><Show when=yes on:click=run><Body/></Show>{value}</main> }",
        );
        let complexities = parsed
            .scopes
            .iter()
            .map(super::ViewScope::complexity)
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
        assert!(!scope.headings[0].is_immediately_preceding);
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
