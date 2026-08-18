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
use syn::ExprMacro;
use syn::spanned::Spanned;

use crate::config::leptos::LeptosViewStructureConfig;
use crate::utils::function_layout_prose::FunctionLayoutProse;

// -----------------------------------------------------------------------------
// View: Authored structure, attributes, and source locations
// -----------------------------------------------------------------------------

/// One direct child in an authored sibling scope.
#[derive(Clone)]
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

/// One ordinary comment positioned at a direct-child boundary.
#[derive(Clone)]
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
        _config: &LeptosViewStructureConfig,
    ) -> Option<&'heading str> {
        let content = self
            .text
            .strip_prefix(LeptosViewStructureConfig::VIEW_SECTION_COMMENT_PREFIX)?
            .strip_prefix(' ')?;

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

// -----------------------------------------------------------------------------
// ViewScope: Direct sibling-scope organization
// -----------------------------------------------------------------------------

/// One independently analyzed direct sibling list.
#[derive(Default)]
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
        let starts = self
            .headings
            .iter()
            .filter_map(|heading| {
                heading
                    .canonical_content(config)
                    .and_then(|_| heading.node.map(|node| (heading, node)))
            })
            .collect::<Vec<_>>();

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

// -----------------------------------------------------------------------------
// ViewStructure: Complete authored view model
// -----------------------------------------------------------------------------

/// Presentation context for one literal recovered from an authored view.
#[derive(Clone)]
pub enum LiteralContext {
    /// Text directly visible between tags.
    Text,
    /// A literal assigned to an attribute or component prop.
    Attribute(
        /// Authored attribute or property name.
        String,
    ),
}

/// One authored literal that may require localization.
#[derive(Clone)]
pub struct Literal {
    /// Exact authored literal span.
    pub(crate) span: Span,
    /// Decoded literal value.
    pub(crate) value: String,
    /// Markup context that determines whether the value is translatable.
    pub(crate) context: LiteralContext,
}

/// Parsed authored structure for one `view!` call.
pub struct Analysis {
    /// Expanded expression used to honor local lint attributes.
    pub(crate) owner: HirId,
    /// Direct sibling scopes found in the authored view.
    pub(crate) scopes: Vec<ViewScope>,
    /// Opening tags and their authored attributes.
    pub(crate) elements: Vec<ViewElement>,
    /// Visible text and translatable literal attributes found in the authored view.
    pub(crate) literals: Vec<Literal>,
}

impl Analysis {
    /// Parses the same RSX token tree as Leptos and overlays authored Rust comments.
    fn parse(span: Span, owner: HirId, source: &str) -> Option<Self> {
        // Invalid macro syntax cannot provide an authored view structure.
        let expression = match syn::parse_str::<ExprMacro>(source) {
            Ok(expression) => expression,
            // Parsing failure leaves no trustworthy macro body to inspect.
            Err(_error) => return None,
        };
        let bounds = body_range(&expression);
        let tokens = without_global_class(expression.mac.tokens);
        let parser = rstml::Parser::new(rstml::ParserConfig::default().recover_block(true));

        let (nodes, errors) = parser.parse_recoverable(tokens).split_vec();

        // Recovered parser errors make the structural model unreliable.
        if !errors.is_empty() {
            return None;
        }

        // Build structural scopes exclusively from rstml's Leptos-compatible node tree.
        let comments = ViewSourceComment::collect(source);

        let mut builder = ViewScopeBuilder {
            source,
            span,
            comments: &comments,
            scopes: Vec::new(),
            elements: Vec::new(),
            literals: Vec::new(),
        };

        builder.collect_scope(&nodes, bounds);
        Some(Self {
            owner,
            scopes: builder.scopes,
            elements: builder.elements,
            literals: builder.literals,
        })
    }
}

// -----------------------------------------------------------------------------
// ViewAttribute: Attribute categories and source evidence
// -----------------------------------------------------------------------------

/// Stable semantic category for an authored attribute or component prop.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ViewAttributeCategory {
    /// Element identity and naming attributes.
    Identity,
    /// Accessibility semantics such as roles and ARIA state.
    Accessibility,
    /// Form and interactive state reflected in markup.
    State,
    /// Styling and visual presentation attributes.
    Presentation,
    /// Application data attached to an element.
    Data,
    /// Event handlers and client-side behavior.
    Behavior,
    /// Framework integration attributes such as refs and directives.
    Integration,
    /// Attributes outside the recognized responsibility vocabulary.
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
        // Event bindings represent client-side behavior rather than static state.
        if name.starts_with("on:") || name.starts_with("on_") {
            return Self::Behavior;
        }

        // Data attributes carry application data independently from presentation.
        if name.starts_with("data-") || name.starts_with("data:") {
            return Self::Data;
        }

        // Framework directives and references integrate the element with Leptos behavior.
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

/// One direct attribute in an authored opening tag.
#[derive(Clone)]
pub struct ViewAttribute {
    /// Element or attribute name involved in the view-structure finding.
    pub(crate) name: String,
    /// Authored declaration or expression range used as the diagnostic anchor.
    pub(crate) span: Span,
    /// Source offsets covered by this contiguous attribute group.
    range: Range<usize>,
    /// Responsibility inferred from the group's attributes.
    pub(crate) category: ViewAttributeCategory,
    /// Direct authored complexity charged to this construct.
    complexity: usize,
}

// -----------------------------------------------------------------------------
// ViewElement: Opening-tag structure
// -----------------------------------------------------------------------------

/// One opening tag with its direct attributes and boundary comments.
pub struct ViewElement {
    /// Element or attribute name involved in the view-structure finding.
    pub(crate) name: String,
    /// Authored declaration or expression range used as the diagnostic anchor.
    pub(crate) span: Span,
    /// Attributes in authored order with their inferred responsibilities.
    attributes: Vec<ViewAttribute>,
    /// Authored group headings and their source positions.
    headings: Vec<ViewHeading>,
}

impl ViewElement {
    /// Computes the direct authored complexity of this construct.
    pub(crate) fn complexity(&self) -> usize {
        self.attributes
            .iter()
            .map(|attribute| attribute.complexity)
            .sum()
    }

    /// Counts attributes assigned to one semantic responsibility.
    pub(crate) fn category_count(&self) -> usize {
        self.attributes
            .iter()
            .map(|attribute| attribute.category)
            .collect::<HashSet<_>>()
            .len()
    }

    /// Counts attributes covered by one authored group heading.
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

    /// Partitions attributes according to authored heading boundaries.
    pub(crate) fn groups(&self, config: &LeptosViewStructureConfig) -> Vec<ViewAttributeGroup<'_>> {
        let starts = self
            .headings
            .iter()
            .filter_map(|heading| {
                heading
                    .canonical_content(config)
                    .and_then(|_| heading.node.map(|attribute| (heading, attribute)))
            })
            .collect::<Vec<_>>();

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

// -----------------------------------------------------------------------------
// ViewSection: Heading-owned sibling nodes
// -----------------------------------------------------------------------------

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

// -----------------------------------------------------------------------------
// ViewSourceRange: Expanded-call deduplication coordinates
// -----------------------------------------------------------------------------

/// Stable source coordinates used to deduplicate expanded view calls.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ViewSourceRange {
    /// Inclusive low byte position.
    lo: u32,
    /// Exclusive high byte position.
    hi: u32,
}

// -----------------------------------------------------------------------------
// ViewCallSites: Per-pass view-call deduplication
// -----------------------------------------------------------------------------

/// Stateful deduplication shared by individual view lint passes.
#[derive(Default)]
pub struct ViewCallSites {
    /// Source positions already analyzed for this pass.
    seen: HashSet<ViewSourceRange>,
}

impl ViewCallSites {
    /// Parses an authored `view!` call once for the current lint pass.
    pub(crate) fn analyze(
        &mut self,
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
    ) -> Option<Analysis> {
        let span = expression.span.macro_backtrace().find_map(|expansion| {
            matches!(expansion.kind, ExpnKind::Macro(MacroKind::Bang, name) if name.as_str() == "view")
                .then_some(expansion.call_site)
        })?;

        // Analyze each authored macro call only once across its expanded expressions.
        if !self.seen.insert(ViewSourceRange {
            lo: span.lo().0,
            hi: span.hi().0,
        }) {
            return None;
        }

        // Missing authored macro text prevents overlaying comments and source ranges.
        let source = match cx.sess().source_map().span_to_snippet(span) {
            Ok(source) => source,
            // Source recovery failure leaves no text or comment positions to model.
            Err(_error) => return None,
        };
        Analysis::parse(span, expression.hir_id, &source)
    }
}

// -----------------------------------------------------------------------------
// ViewScopeBuilder: Rstml-to-policy model projection
// -----------------------------------------------------------------------------

/// Projects rstml nodes into the small stable model used by the lint family.
struct ViewScopeBuilder<'source> {
    /// Complete authored macro invocation.
    source: &'source str,
    /// Rustc span corresponding to source byte zero.
    span: Span,
    /// Ordinary Rust comments retained separately from the token stream.
    comments: &'source [ViewSourceComment],
    /// Completed sibling scopes.
    scopes: Vec<ViewScope>,
    /// Completed authored opening tags.
    elements: Vec<ViewElement>,
    /// Presentation literals collected across every descendant scope.
    literals: Vec<Literal>,
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
        let range = node.span().byte_range();

        // Invalid or out-of-bounds parser ranges cannot map back to authored source.
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
            // Nonparticipant nodes do not contribute direct navigation complexity.
            Node::Comment(_)
            | Node::Doctype(_)
            | Node::Text(_)
            | Node::RawText(_)
            | Node::Fragment(_)
            | Node::Custom(_) => return None,
        };

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
        let attributes = element
            .attributes()
            .iter()
            .filter_map(|attribute| {
                let range = attribute.span().byte_range();

                // Invalid attribute ranges cannot contribute reliable source evidence.
                if range.start >= range.end || range.end > self.source.len() {
                    return None;
                }
                let (name, category, complexity) = match attribute {
                    NodeAttribute::Attribute(attribute) => {
                        let name = attribute.key.to_string();
                        if let Some(value) = attribute.value_literal_string() {
                            self.literals.push(Literal {
                                span: self.to_rustc_span(&range),
                                value,
                                context: LiteralContext::Attribute(name.clone()),
                            });
                        }
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

        // Opening tags without direct attributes need no attribute-group model.
        if attributes.is_empty() {
            return;
        }
        let opening = element.open_tag.span().byte_range();
        let bounds = opening.clone();

        let headings = self.collect_headings_for_ranges(
            &attributes
                .iter()
                .map(|attribute| attribute.range.clone())
                .collect::<Vec<_>>(),
            &bounds,
        );

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

        for node in &direct {
            let (value, range) = match node {
                Node::Text(text) => (text.value_string(), text.span().byte_range()),
                Node::RawText(text) => (
                    text.to_source_text(false)
                        .unwrap_or_else(|| text.to_token_stream_string()),
                    text.span().byte_range(),
                ),
                _ => continue,
            };
            if range.start >= range.end || range.end > self.source.len() {
                continue;
            }
            self.literals.push(Literal {
                span: self.to_rustc_span(&range),
                value,
                context: LiteralContext::Text,
            });
        }

        // Every element child list is an independent direct sibling scope.
        for node in direct {
            // Only elements own nested sibling scopes and opening-tag attributes.
            let Node::Element(element) = node else {
                continue;
            };
            self.collect_element(element);

            // Empty elements have no descendant scope to analyze.
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

// -----------------------------------------------------------------------------
// ViewAttributeGroup: Heading-owned attribute runs
// -----------------------------------------------------------------------------

/// Consecutive attributes introduced by one canonical heading.
pub struct ViewAttributeGroup<'element> {
    /// Authored heading that claims this group.
    pub(crate) heading: &'element ViewHeading,
    /// Attribute names and source positions recovered from one element.
    pub(crate) attributes: &'element [ViewAttribute],
}

impl ViewAttributeGroup<'_> {
    /// Computes the direct authored complexity of this construct.
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
    let terminal = name
        .rsplit([':', '/'])
        .find(|segment| !segment.trim().is_empty())
        .map_or(name, str::trim);
    matches!(terminal, "For" | "Show" | "Suspense" | "ErrorBoundary")
}

/// Returns whether an opening tag contains an authored Leptos event binding.
fn has_event_handler(element: &NodeElement<rstml::Infallible>) -> bool {
    element.attributes().iter().any(|attribute| {
        matches!(attribute, NodeAttribute::Attribute(attribute) if attribute.key.to_string().starts_with("on:"))
    })
}

/// Extracts one immediately owned quoted text node.
fn direct_literal(element: &NodeElement<rstml::Infallible>) -> Option<String> {
    // Mixed or nested children are not one directly owned literal.
    let [Node::Text(text)] = element.children.as_slice() else {
        return None;
    };
    Some(text.value_string())
}

// -----------------------------------------------------------------------------
// ViewSourceComment: Authored comment recovery
// -----------------------------------------------------------------------------

/// Ordinary Rust line comment omitted from proc-macro token streams.
struct ViewSourceComment {
    /// Source-relative byte range.
    range: Range<usize>,
    /// Complete authored token text.
    text: String,
}

impl ViewSourceComment {
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
    use std::str::FromStr;

    use super::Analysis;
    use super::rustc_hir::CRATE_HIR_ID;
    use super::rustc_span::{BytePos, Span};

    impl FromStr for Analysis {
        type Err = &'static str;

        fn from_str(source: &str) -> Result<Self, Self::Err> {
            Self::parse(
                Span::with_root_ctxt(
                    BytePos(0),
                    BytePos(u32::try_from(source.len()).expect("test view fits in a source span")),
                ),
                CRATE_HIR_ID,
                source,
            )
            .ok_or("representative Leptos view should parse")
        }
    }

    #[test]
    fn delegates_nested_elements_blocks_and_attributes_to_rstml() {
        let parsed =
            "view! { <main><Header/><Show when=yes on:click=run><Body/></Show>{value}</main> }"
                .parse::<Analysis>()
                .expect("representative view should parse");
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
        let parsed = "view! { <>// Account navigation\n<Nav/><Crumbs/></> }"
            .parse::<Analysis>()
            .expect("representative view should parse");
        assert_eq!(parsed.scopes[0].nodes.len(), 2);
        assert_eq!(parsed.scopes[0].headings[0].node, Some(0));
    }

    #[test]
    fn retains_stranded_direct_boundary_comments() {
        let parsed = "view! { <main><Content/> // Stranded heading\n</main> }"
            .parse::<Analysis>()
            .expect("representative view should parse");
        let scope = &parsed.scopes[1];
        assert_eq!(scope.headings.len(), 1);
        assert_eq!(scope.headings[0].node, None);
        assert!(!scope.headings[0].is_immediately_preceding);
    }

    #[test]
    fn rust_blocks_can_contain_markup_like_strings_and_nested_macros() {
        let parsed = r#"view! { {format!("<Fake/>")} <Real value=move || call()/> }"#
            .parse::<Analysis>()
            .expect("representative view should parse");
        assert_eq!(parsed.scopes[0].nodes.len(), 2);
        assert_eq!(parsed.scopes[0].nodes[1].name.as_deref(), Some("Real"));
    }

    #[test]
    fn mirrors_global_class_prelude_and_preserves_source_spans() {
        let source = r#"view! { class="shell", <main><Child/></main> }"#;
        let parsed = source
            .parse::<Analysis>()
            .expect("representative view should parse");
        let main = &parsed.scopes[0].nodes[0];
        assert_eq!(&source[main.range.clone()], "<main><Child/></main>");
    }
}
