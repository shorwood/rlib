use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use convert_case::{Case, Casing};
use proc_macro2::{LineColumn, Span as TokenSpan, TokenStream, TokenTree};
use raffia::ast::{AtRulePrelude, QualifiedRule, Statement, Stylesheet};
use raffia::{Parser, Syntax};
use rstml::node::{KeyedAttribute, Node, NodeAttribute, NodeElement};
use syn::parse::Parser as _;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, ExprPath, Lit, Macro, Token};

use crate::rules::leptos::utils::authored_files::SourceDocument;

// -----------------------------------------------------------------------------
// Findings: Source-level component styling evidence
// -----------------------------------------------------------------------------

/// One source-oriented policy violation anchored in a Rust file.
pub struct RustFinding {
    /// Authored byte range receiving the diagnostic.
    pub(crate) range: Range<usize>,
    /// Policy-specific explanation attached to the range.
    pub(crate) message: String,
}

/// One external stylesheet declaration in a Rust module.
pub struct StyleSheetSource {
    /// Authored macro invocation range.
    pub(crate) range: Range<usize>,
    /// Local stylesheet module alias, when parsed.
    pub(crate) alias: Option<String>,
    /// Resolved stylesheet path, when declared as a string literal.
    pub(crate) declared: Option<PathBuf>,
}

/// Complete authored styling evidence for one Rust source file.
pub struct ComponentStyleAnalysis {
    /// Rust source document from which the evidence was collected.
    pub(crate) document: SourceDocument,
    /// Whether the source contains at least one Leptos view invocation.
    pub(crate) has_view: bool,
    /// Whether the view contains any class or style mechanism.
    pub(crate) is_styled: bool,
    /// External stylesheet declarations in authored order.
    pub(crate) style_sheets: Vec<StyleSheetSource>,
    /// Inline stylesheet macro ranges in authored order.
    pub(crate) inline_style_sheets: Vec<Range<usize>>,
    /// Generated local class constants referenced by the component.
    pub(crate) class_references: BTreeSet<String>,
    /// Untyped class uses found in the component view.
    pub(crate) class_findings: Vec<RustFinding>,
    /// Inline property declarations found in the component view.
    pub(crate) inline_style_findings: Vec<RustFinding>,
}

impl ComponentStyleAnalysis {
    /// Parses one Rust file and projects only its authored style contracts.
    pub(crate) fn analyze(document: SourceDocument) -> Option<Self> {
        // Invalid Rust source cannot provide a trustworthy authored styling model.
        let Ok(syntax) = syn::parse_file(&document.source) else {
            // Parsing failure leaves no syntax tree from which to collect style contracts.
            return None;
        };
        let offsets = SourceOffsets::from(document.source.as_str());
        let rust_path = document.path.clone();
        let mut collector = RustStyleCollector {
            rust_path: &rust_path,
            offsets: &offsets,
            has_view: false,
            is_styled: false,
            style_sheets: Vec::new(),
            inline_style_sheets: Vec::new(),
            class_references: BTreeSet::new(),
            class_findings: Vec::new(),
            inline_style_findings: Vec::new(),
        };
        collector.visit_file(&syntax);
        Some(Self {
            document,
            has_view: collector.has_view,
            is_styled: collector.is_styled,
            style_sheets: collector.style_sheets,
            inline_style_sheets: collector.inline_style_sheets,
            class_references: collector.class_references,
            class_findings: collector.class_findings,
            inline_style_findings: collector.inline_style_findings,
        })
    }

    /// Returns the mandatory colocated stylesheet path for this Rust module.
    pub(crate) fn paired_css_path(&self) -> PathBuf {
        self.document.path.with_extension("css")
    }

    /// Returns the single paired source declaration, if the contract is satisfied.
    pub(crate) fn paired_style_sheet(&self) -> Option<&StyleSheetSource> {
        // Colocation requires exactly one external stylesheet declaration.
        let [style_sheet] = self.style_sheets.as_slice() else {
            return None;
        };
        let declared = style_sheet.declared.as_ref()?;
        same_file(declared, &self.paired_css_path()).then_some(style_sheet)
    }
}

// -----------------------------------------------------------------------------
// RustStyleCollector: Syn and rstml authored source projection
// -----------------------------------------------------------------------------

/// Collects styling constructs from one parsed Rust source file.
struct RustStyleCollector<'source> {
    /// Physical Rust path used to resolve stylesheet declarations.
    rust_path: &'source Path,
    /// Mapping from proc-macro line/column spans to source byte offsets.
    offsets: &'source SourceOffsets,
    /// Whether a Leptos view invocation was encountered.
    has_view: bool,
    /// Whether any view attribute participates in styling.
    is_styled: bool,
    /// External stylesheet declarations in authored order.
    style_sheets: Vec<StyleSheetSource>,
    /// Inline stylesheet invocation ranges in authored order.
    inline_style_sheets: Vec<Range<usize>>,
    /// Generated local class constants referenced from expressions.
    class_references: BTreeSet<String>,
    /// Untyped class uses found while parsing views.
    class_findings: Vec<RustFinding>,
    /// Inline property uses found while parsing views.
    inline_style_findings: Vec<RustFinding>,
}

impl RustStyleCollector<'_> {
    /// Parses one external stylesheet macro invocation.
    fn collect_style_sheet(&mut self, mac: &Macro) {
        // Malformed arguments yield an incomplete declaration rather than false parsed evidence.
        let expressions = Punctuated::<Expr, Token![,]>::parse_terminated
            .parse2(mac.tokens.clone())
            .ok();
        let alias = expressions
            .as_ref()
            .and_then(|expressions| expressions.first())
            .and_then(path_expression_terminal);
        let declared = expressions
            .as_ref()
            .and_then(|expressions| expressions.iter().nth(1))
            .and_then(string_expression)
            .map(|path| resolve_style_path(self.rust_path, &path));
        self.style_sheets.push(StyleSheetSource {
            range: self.offsets.range(mac.span()),
            alias,
            declared,
        });
    }

    /// Records one ordinary class attribute and its generated constant references.
    fn collect_class_attribute(&mut self, attribute: &KeyedAttribute, range: Range<usize>) {
        self.is_styled = true;

        // A valueless class attribute cannot name a generated stylesheet constant.
        let Some(value) = attribute.value() else {
            self.class_findings.push(RustFinding {
                range,
                message: "class attributes must use local generated constants".to_owned(),
            });
            return;
        };

        // Reject class expressions containing raw or foreign class identities.
        if !collect_class_expression(value, &mut self.class_references) {
            self.class_findings.push(RustFinding {
                range,
                message: "class value is not composed from local `style::CONSTANT` values"
                    .to_owned(),
            });
        }
    }

    /// Records one ordinary style attribute and validates custom-property-only use.
    fn collect_style_attribute(&mut self, attribute: &KeyedAttribute, range: Range<usize>) {
        self.is_styled = true;
        let accepted = attribute.value().is_some_and(is_custom_property_binding);

        // Ordinary style attributes may expose only a custom-property value to CSS.
        if !accepted {
            self.inline_style_findings.push(RustFinding {
                range,
                message: "inline style must be a custom-property binding such as `style=(\"--width\", value)`"
                    .to_owned(),
            });
        }
    }

    /// Classifies one named view attribute by its styling contract.
    fn collect_named_attribute(&mut self, attribute: &KeyedAttribute, range: Range<usize>) {
        let name = attribute.key.to_string().replace(' ', "");
        match name.as_str() {
            "class" => self.collect_class_attribute(attribute, range),
            name if name.starts_with("class:") => {
                self.is_styled = true;
                self.class_findings.push(RustFinding {
                    range,
                    message: "class directives bypass generated stylesheet constants".to_owned(),
                });
            }
            "style" => self.collect_style_attribute(attribute, range),
            name if name.starts_with("style:") => {
                self.is_styled = true;
                self.inline_style_findings.push(RustFinding {
                    range,
                    message: "style directives bypass the colocated stylesheet".to_owned(),
                });
            }
            _ => {}
        }
    }

    /// Collects styling evidence from one direct opening-tag attribute.
    fn collect_attribute(&mut self, attribute: &NodeAttribute) {
        let range = self.offsets.range(attribute.span());
        match attribute {
            NodeAttribute::Block(_) => {
                self.is_styled = true;
                self.class_findings.push(RustFinding {
                    range,
                    message: "opaque attribute spreads can introduce untyped classes".to_owned(),
                });
            }
            NodeAttribute::Attribute(attribute) => {
                self.collect_named_attribute(attribute, range);
                if let Some(value) = attribute.value() {
                    self.visit_expr(value);
                }
            }
        }
    }

    /// Collects one element's attributes and recursively visits its children.
    fn collect_element(&mut self, element: &NodeElement<rstml::Infallible>) {
        for attribute in element.attributes() {
            self.collect_attribute(attribute);
        }
        for child in &element.children {
            self.collect_node(child);
        }
    }

    /// Recursively analyzes element attributes while preserving authored spans.
    fn collect_node(&mut self, node: &Node) {
        match node {
            Node::Element(element) => self.collect_element(element),
            Node::Fragment(fragment) => {
                for child in &fragment.children {
                    self.collect_node(child);
                }
            }
            Node::Block(block) => {
                if let Some(block) = block.try_block() {
                    self.visit_block(block);
                }
            }
            Node::Comment(_)
            | Node::Doctype(_)
            | Node::Text(_)
            | Node::RawText(_)
            | Node::Custom(_) => {}
        }
    }

    /// Parses a Leptos view through the same rstml grammar used by existing lints.
    fn collect_view(&mut self, mac: &Macro) {
        self.has_view = true;
        let parser = rstml::Parser::new(rstml::ParserConfig::default().recover_block(true));
        let (nodes, errors) = parser.parse_recoverable(mac.tokens.clone()).split_vec();

        // Recovered markup errors make attribute contracts and source ranges unreliable.
        if !errors.is_empty() {
            return;
        }
        for node in &nodes {
            self.collect_node(node);
        }
    }
}

impl<'ast> Visit<'ast> for RustStyleCollector<'_> {
    fn visit_macro(&mut self, mac: &'ast Macro) {
        collect_style_token_references(&mac.tokens, &mut self.class_references);

        // A terminal macro name is required to select a styling operation.
        let Some(name) = mac
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
        else {
            return;
        };
        match name.as_str() {
            "style_sheet" => self.collect_style_sheet(mac),
            "inline_style_sheet" => {
                self.inline_style_sheets
                    .push(self.offsets.range(mac.span()));
            }
            "view" => self.collect_view(mac),
            _ => {}
        }
        visit::visit_macro(self, mac);
    }

    fn visit_expr_path(&mut self, expression: &'ast ExprPath) {
        if let Some(reference) = local_style_constant(expression) {
            self.class_references.insert(reference);
        }
        visit::visit_expr_path(self, expression);
    }
}

// -----------------------------------------------------------------------------
// StylesheetFacts: Raffia-backed selector analysis
// -----------------------------------------------------------------------------

/// One class declared by a stylesheet selector.
#[derive(Clone)]
pub struct CssClass {
    /// Authored class name without the selector prefix.
    pub(crate) name: String,
    /// Byte range of the complete class selector token.
    pub(crate) range: Range<usize>,
}

/// Structural stylesheet facts used by scope and usage lints.
pub struct StylesheetFacts {
    /// Distinct class declarations in stable source order.
    pub(crate) classes: Vec<CssClass>,
    /// Selector-scope violations found while traversing the stylesheet.
    pub(crate) scope_findings: Vec<RustFinding>,
}

/// Whether a nested selector may inherit a component class anchor from its parent.
#[derive(Clone, Copy)]
enum SelectorScope {
    /// No enclosing selector guarantees component-local scope.
    Unanchored,
    /// Every enclosing selector branch carries a component class anchor.
    Anchored,
}

impl SelectorScope {
    /// Converts branch-wide anchoring evidence into a nested selector scope.
    const fn from_anchored(is_anchored: bool) -> Self {
        if is_anchored {
            Self::Anchored
        } else {
            Self::Unanchored
        }
    }

    /// Returns whether parent-reference selectors may inherit a local anchor.
    const fn is_anchored(self) -> bool {
        matches!(self, Self::Anchored)
    }
}

impl StylesheetFacts {
    /// Parses valid CSS and recursively checks every ordinary selector branch.
    pub(crate) fn parse(source: &str) -> Option<Self> {
        let mut parser = Parser::new(source, Syntax::Css);

        // Invalid CSS cannot provide reliable selector or class evidence.
        let Ok(stylesheet) = parser.parse::<Stylesheet<'_>>() else {
            // Parsing failure leaves no selector tree from which to collect facts.
            return None;
        };
        let mut facts = Self {
            classes: Vec::new(),
            scope_findings: Vec::new(),
        };
        facts.collect_statements(source, &stylesheet.statements, SelectorScope::Unanchored);

        let mut unique = BTreeMap::new();
        for class in facts.classes.drain(..) {
            unique.entry(class.name.clone()).or_insert(class);
        }
        facts.classes = unique.into_values().collect();
        Some(facts)
    }

    /// Collects one qualified rule and propagates its complete anchoring state.
    fn collect_qualified_rule(
        &mut self,
        source: &str,
        rule: &QualifiedRule<'_>,
        parent_scope: SelectorScope,
    ) {
        let mut every_branch_anchored = true;
        for selector in &rule.selector.selectors {
            let range = selector.span.start..selector.span.end;
            let selector_source = source.get(range.clone()).unwrap_or_default();
            let classes = css_classes(selector_source, range.start);
            let is_anchored = !classes.is_empty()
                || (parent_scope.is_anchored() && selector_source.contains('&'));
            if !is_anchored {
                every_branch_anchored = false;
                self.scope_findings.push(RustFinding {
                    range,
                    message:
                        "selector branch is not anchored by a class from this component stylesheet"
                            .to_owned(),
                });
            }
            self.classes.extend(classes);
        }
        self.collect_statements(
            source,
            &rule.block.statements,
            SelectorScope::from_anchored(every_branch_anchored),
        );
    }

    /// Walks rules through conditional at-rules while preserving nesting scope.
    fn collect_statements(
        &mut self,
        source: &str,
        statements: &[Statement<'_>],
        parent_scope: SelectorScope,
    ) {
        for statement in statements {
            match statement {
                Statement::QualifiedRule(rule) => {
                    self.collect_qualified_rule(source, rule, parent_scope);
                }
                Statement::AtRule(rule) => {
                    if matches!(rule.prelude, Some(AtRulePrelude::Import(_)))
                        || rule.name.name.eq_ignore_ascii_case("import")
                    {
                        self.scope_findings.push(RustFinding {
                            range: rule.span.start..rule.span.end,
                            message: "component stylesheets must not import other stylesheets"
                                .to_owned(),
                        });
                    }
                    let is_keyframes = matches!(rule.prelude, Some(AtRulePrelude::Keyframes(_)))
                        || rule.name.name.eq_ignore_ascii_case("keyframes");
                    if !is_keyframes && let Some(block) = &rule.block {
                        self.collect_statements(source, &block.statements, parent_scope);
                    }
                }
                Statement::Declaration(_)
                | Statement::KeyframeBlock(_)
                | Statement::LessConditionalQualifiedRule(_)
                | Statement::LessExtendRule(_)
                | Statement::LessFunctionCall(_)
                | Statement::LessMixinCall(_)
                | Statement::LessMixinDefinition(_)
                | Statement::LessVariableCall(_)
                | Statement::LessVariableDeclaration(_)
                | Statement::SassIfAtRule(_)
                | Statement::SassVariableDeclaration(_)
                | Statement::UnknownSassAtRule(_) => {}
            }
        }
    }
}

/// Converts a Turf class name into its generated associated constant.
pub fn generated_constant(class: &str) -> String {
    class.to_case(Case::UpperSnake)
}

// -----------------------------------------------------------------------------
// Helpers: Paths, expressions, source offsets, and selector tokens
// -----------------------------------------------------------------------------

/// Returns the terminal identifier of a direct path expression.
fn path_expression_terminal(expression: &Expr) -> Option<String> {
    // Other expression forms cannot name a stylesheet module alias directly.
    let Expr::Path(path) = expression else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

/// Returns the value of a direct string-literal expression.
fn string_expression(expression: &Expr) -> Option<String> {
    // Other expression forms do not provide a static stylesheet path.
    let Expr::Lit(literal) = expression else {
        return None;
    };

    // Nonstring literals cannot encode a filesystem path.
    let Lit::Str(value) = &literal.lit else {
        return None;
    };
    Some(value.value())
}

/// Resolves an authored stylesheet path relative to its package manifest.
fn resolve_style_path(rust_path: &Path, declared: &str) -> PathBuf {
    let declared = PathBuf::from(declared);

    // Absolute declarations need no package-relative resolution.
    if declared.is_absolute() {
        return declared.canonicalize().unwrap_or(declared);
    }
    let manifest = rust_path
        .ancestors()
        .find(|ancestor| ancestor.join("Cargo.toml").is_file())
        .unwrap_or_else(|| rust_path.parent().unwrap_or_else(|| Path::new(".")));
    let path = manifest.join(declared);
    path.canonicalize().unwrap_or(path)
}

/// Returns whether two paths identify the same canonical filesystem entry.
fn same_file(left: &Path, right: &Path) -> bool {
    let left = left.canonicalize().unwrap_or_else(|_| left.to_path_buf());
    let right = right.canonicalize().unwrap_or_else(|_| right.to_path_buf());
    left == right
}

/// Extracts a generated class constant from a local style-module path.
fn local_style_constant(expression: &ExprPath) -> Option<String> {
    let segments = expression.path.segments.iter().collect::<Vec<_>>();

    // Generated class references use exactly the local module and constant segments.
    let [namespace, constant] = segments.as_slice() else {
        return None;
    };
    (namespace.ident == "style")
        .then(|| constant.ident.to_string())
        .filter(|constant| {
            constant
                .chars()
                .any(|character| character.is_ascii_uppercase())
        })
}

/// Finds local generated constants inside arbitrary macro token streams.
fn collect_style_token_references(tokens: &TokenStream, references: &mut BTreeSet<String>) {
    let tokens = tokens.clone().into_iter().collect::<Vec<_>>();
    for window in tokens.windows(4) {
        let [
            TokenTree::Ident(namespace),
            TokenTree::Punct(first),
            TokenTree::Punct(second),
            TokenTree::Ident(constant),
        ] = window
        else {
            continue;
        };
        if namespace == "style"
            && first.as_char() == ':'
            && second.as_char() == ':'
            && constant
                .to_string()
                .chars()
                .any(|character| character.is_ascii_uppercase())
        {
            references.insert(constant.to_string());
        }
    }
    for token in tokens {
        if let TokenTree::Group(group) = token {
            collect_style_token_references(&group.stream(), references);
        }
    }
}

/// Validates a class expression and records every generated constant it references.
fn collect_class_expression(expression: &Expr, references: &mut BTreeSet<String>) -> bool {
    match expression {
        Expr::Path(path) => local_style_constant(path)
            .map(|reference| references.insert(reference))
            .is_some(),
        Expr::Lit(literal) => {
            matches!(&literal.lit, Lit::Str(value) if value.value().trim().is_empty())
        }
        Expr::Paren(expression) => collect_class_expression(&expression.expr, references),
        Expr::Group(expression) => collect_class_expression(&expression.expr, references),
        Expr::Closure(closure) => collect_class_expression(&closure.body, references),
        Expr::Block(block) => block
            .block
            .stmts
            .last()
            .and_then(|statement| match statement {
                syn::Stmt::Expr(expression, _) => Some(expression),
                _ => None,
            })
            .is_some_and(|expression| collect_class_expression(expression, references)),
        Expr::If(expression) => {
            let then_ok = expression
                .then_branch
                .stmts
                .last()
                .and_then(|statement| match statement {
                    syn::Stmt::Expr(expression, _) => Some(expression),
                    _ => None,
                })
                .is_some_and(|expression| collect_class_expression(expression, references));
            let else_ok = expression
                .else_branch
                .as_ref()
                .is_none_or(|(_, expression)| collect_class_expression(expression, references));
            then_ok && else_ok
        }
        Expr::Match(expression) => expression
            .arms
            .iter()
            .all(|arm| collect_class_expression(&arm.body, references)),
        Expr::Tuple(tuple) => {
            // An empty tuple contributes no untyped class value.
            let Some(first) = tuple.elems.first() else {
                return true;
            };
            if tuple.elems.len() == 2 && collect_class_expression(first, references) {
                true
            } else {
                tuple
                    .elems
                    .iter()
                    .all(|element| collect_class_expression(element, references))
            }
        }
        Expr::Array(array) => array
            .elems
            .iter()
            .all(|element| collect_class_expression(element, references)),
        _ => false,
    }
}

/// Recognizes an inline binding that assigns one CSS custom property.
fn is_custom_property_binding(expression: &Expr) -> bool {
    // Custom-property bindings use tuple attribute syntax.
    let Expr::Tuple(tuple) = expression else {
        return false;
    };

    // The first tuple element must statically name the property.
    let Some(Expr::Lit(literal)) = tuple.elems.first() else {
        return false;
    };
    matches!(&literal.lit, Lit::Str(name) if name.value().starts_with("--") && name.value().len() > 2)
}

/// Extracts class selector tokens and maps them to stylesheet byte ranges.
fn css_classes(selector: &str, base: usize) -> Vec<CssClass> {
    let bytes = selector.as_bytes();
    let mut classes = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'.'
            || bytes
                .get(index.wrapping_sub(1))
                .is_some_and(u8::is_ascii_digit)
        {
            index += 1;
            continue;
        }
        let start = index + 1;
        let Some(first) = bytes.get(start) else {
            break;
        };
        if !(first.is_ascii_alphabetic() || *first == b'_' || *first == b'-') {
            index += 1;
            continue;
        }
        let mut end = start + 1;
        while bytes
            .get(end)
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'_' | b'-'))
        {
            end += 1;
        }
        classes.push(CssClass {
            name: selector[start..end].to_owned(),
            range: (base + index)..(base + end),
        });
        index = end;
    }
    classes
}

/// Maps proc-macro line and column coordinates into source byte ranges.
struct SourceOffsets {
    /// Byte offset at which each source line begins.
    line_starts: Vec<usize>,
    /// Total source length used to clamp invalid external positions.
    source_len: usize,
}

impl From<&str> for SourceOffsets {
    /// Indexes the line starts of one authored source document.
    fn from(source: &str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            source
                .bytes()
                .enumerate()
                .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
        );
        Self {
            line_starts,
            source_len: source.len(),
        }
    }
}

impl SourceOffsets {
    /// Converts one proc-macro position into a clamped source byte offset.
    fn offset(&self, position: LineColumn) -> usize {
        self.line_starts
            .get(position.line.saturating_sub(1))
            .copied()
            .unwrap_or(self.source_len)
            .saturating_add(position.column)
            .min(self.source_len)
    }

    /// Converts a proc-macro span into an authored source byte range.
    fn range(&self, span: TokenSpan) -> Range<usize> {
        self.offset(span.start())..self.offset(span.end())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use syn::{Expr, Macro};

    use super::{
        StylesheetFacts, collect_class_expression, collect_style_token_references,
        generated_constant,
    };

    #[test]
    fn maps_turf_class_names_to_generated_constants() {
        assert_eq!(generated_constant("table-wrap"), "TABLE_WRAP");
        assert_eq!(generated_constant("HTTPStatus"), "HTTP_STATUS");
    }

    #[test]
    fn accepts_local_constants_and_conditional_empty_fallbacks() {
        let expression: Expr =
            syn::parse_quote!(move || if selected { style::SELECTED } else { "" });
        let mut references = BTreeSet::new();
        assert!(collect_class_expression(&expression, &mut references));
        assert_eq!(references, BTreeSet::from(["SELECTED".to_owned()]));
    }

    #[test]
    fn rejects_raw_and_foreign_class_values() {
        for expression in [
            syn::parse_quote!("selected"),
            syn::parse_quote!(shared::SELECTED),
            syn::parse_quote!(classes()),
        ] {
            assert!(!collect_class_expression(&expression, &mut BTreeSet::new()));
        }
    }

    #[test]
    fn finds_generated_constants_inside_nested_macro_tokens() {
        let macro_: Macro = syn::parse_quote! {
            view! {
                {items.map(|_| view! { <span class=style::ITEM></span> })}
                {format!(".{}", style::QUERY_TARGET)}
            }
        };
        let mut references = BTreeSet::new();
        collect_style_token_references(&macro_.tokens, &mut references);
        assert_eq!(
            references,
            BTreeSet::from(["ITEM".to_owned(), "QUERY_TARGET".to_owned()])
        );
    }

    #[test]
    fn traverses_nested_rules_and_rejects_classless_branches_and_imports() {
        let source = r#"
            @import "foreign.css";
            @media (width > 20rem) {
                .card, button { color: red; }
            }
            .panel { &:hover { color: blue; } }
            @keyframes reveal { from { opacity: 0; } to { opacity: 1; } }
        "#;
        let facts = StylesheetFacts::parse(source).expect("representative CSS should parse");
        assert_eq!(
            facts
                .classes
                .iter()
                .map(|class| class.name.as_str())
                .collect::<Vec<_>>(),
            ["card", "panel"]
        );
        assert_eq!(facts.scope_findings.len(), 2);
    }
}
