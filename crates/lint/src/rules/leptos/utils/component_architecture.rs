extern crate rustc_span;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ops::Range;

use proc_macro2::{LineColumn, Span as TokenSpan};
use quote::ToTokens;
use rstml::node::{Node, NodeAttribute, NodeElement};
use rustc_span::Span;
use serde::Deserialize;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, ExprCall, ExprClosure, FnArg, Item, ItemFn, ItemMod, Macro, Pat, Stmt};

use super::authored_files::SourceDocument;
use crate::utils::config::LibraryConfig;

// -----------------------------------------------------------------------------
// LeptosArchitectureConfig: Authored component architecture policy
// -----------------------------------------------------------------------------

/// Thresholds shared by the source-oriented Leptos architecture lint family.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct LeptosArchitectureConfig {
    pub(crate) max_setup_statements: usize,
    pub(crate) max_reactive_primitives: usize,
    pub(crate) max_handler_statements: usize,
    pub(crate) max_handler_control_flow_depth: usize,
    pub(crate) max_view_nesting_depth: usize,
    pub(crate) max_view_control_depth: usize,
    pub(crate) max_component_composition_depth: usize,
    pub(crate) max_component_props: usize,
    pub(crate) max_components_per_module: usize,
    pub(crate) min_repeated_fragment_nodes: usize,
    pub(crate) min_repeated_fragment_occurrences: usize,
}

impl Default for LeptosArchitectureConfig {
    fn default() -> Self {
        Self {
            max_setup_statements: 8,
            max_reactive_primitives: 4,
            max_handler_statements: 3,
            max_handler_control_flow_depth: 1,
            max_view_nesting_depth: 7,
            max_view_control_depth: 3,
            max_component_composition_depth: 10,
            max_component_props: 6,
            max_components_per_module: 8,
            min_repeated_fragment_nodes: 6,
            min_repeated_fragment_occurrences: 2,
        }
    }
}

impl LeptosArchitectureConfig {
    /// Loads the configured architecture policy and rejects inert zero limits.
    pub(crate) fn from_config() -> Self {
        let config = LibraryConfig::load().leptos_architecture;
        let values = [
            config.max_setup_statements,
            config.max_reactive_primitives,
            config.max_handler_statements,
            config.max_handler_control_flow_depth,
            config.max_view_nesting_depth,
            config.max_view_control_depth,
            config.max_component_composition_depth,
            config.max_component_props,
            config.max_components_per_module,
            config.min_repeated_fragment_nodes,
            config.min_repeated_fragment_occurrences,
        ];
        assert!(
            values.into_iter().all(|value| value > 0),
            "invalid Leptos architecture configuration: every limit must be greater than zero"
        );
        config
    }
}

// -----------------------------------------------------------------------------
// ArchitectureAnalysis: Shared authored-source model
// -----------------------------------------------------------------------------

/// One event handler found in a view attribute or `Callback::new` expression.
pub struct HandlerAnalysis {
    pub(crate) span: Span,
    pub(crate) statements: usize,
    pub(crate) control_depth: usize,
}

/// One normalized view subtree that is large enough to become a clone candidate.
pub struct FragmentAnalysis {
    pub(crate) span: Span,
    pub(crate) fingerprint: String,
    pub(crate) nodes: usize,
    pub(crate) owner: String,
    range: Range<usize>,
}

/// Source facts about one authored function.
pub struct FunctionAnalysis {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) is_component: bool,
    pub(crate) is_composable: bool,
    pub(crate) setup_statements: usize,
    pub(crate) reactive_primitives: Vec<Span>,
    pub(crate) handlers: Vec<HandlerAnalysis>,
    pub(crate) spawned_tasks: Vec<Span>,
    pub(crate) max_view_depth: usize,
    pub(crate) max_view_control_depth: usize,
    pub(crate) component_calls: BTreeSet<String>,
    pub(crate) function_calls: BTreeSet<String>,
    pub(crate) props: usize,
}

impl FunctionAnalysis {
    pub(crate) const fn is_reactive_owner(&self) -> bool {
        self.is_component || self.is_composable
    }
}

/// A module containing more authored component entry points than policy permits.
pub struct ModuleAnalysis {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) components: usize,
}

/// One helper that performs reactive work without composable naming.
pub struct UnnamedComposableAnalysis {
    pub(crate) name: String,
    pub(crate) span: Span,
}

/// One root component whose local composition chain exceeds policy.
pub struct CompositionAnalysis {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) depth: usize,
}

/// One repeated maximal view fragment after literal and binding normalization.
pub struct RepeatedFragmentAnalysis {
    pub(crate) span: Span,
    pub(crate) first_span: Span,
    pub(crate) nodes: usize,
    pub(crate) occurrences: usize,
}

/// Complete crate-wide evidence reused by every architecture policy.
pub struct ArchitectureAnalysis {
    pub(crate) functions: Vec<FunctionAnalysis>,
    pub(crate) modules: Vec<ModuleAnalysis>,
    fragments: Vec<FragmentAnalysis>,
}

impl ArchitectureAnalysis {
    /// Parses all authored files into a deterministic crate-wide architecture model.
    pub(crate) fn analyze(documents: Vec<SourceDocument>) -> Self {
        let mut functions = Vec::new();
        let mut modules = Vec::new();
        let mut fragments = Vec::new();
        for document in documents {
            let Ok(file) = syn::parse_file(&document.source) else {
                continue;
            };
            let offsets = SourceOffsets::from(document.source.as_str());
            let aliases = spawn_aliases(&file.items);
            let root = document
                .path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("crate")
                .to_owned();
            collect_items(
                &document,
                &offsets,
                &aliases,
                &file.items,
                &root,
                &mut functions,
                &mut modules,
                &mut fragments,
            );
        }
        Self {
            functions,
            modules,
            fragments,
        }
    }

    /// Resolves the local component graph and reports only roots of overlong chains.
    pub(crate) fn excessive_compositions(&self, maximum: usize) -> Vec<CompositionAnalysis> {
        let components = unique_function_indices(
            self.functions
                .iter()
                .enumerate()
                .filter(|(_, function)| function.is_component),
        );
        let mut incoming = vec![0usize; self.functions.len()];
        for function in &self.functions {
            for call in &function.component_calls {
                if let Some(index) = components.get(call) {
                    incoming[*index] += 1;
                }
            }
        }
        let mut memo = HashMap::new();
        let mut findings = Vec::new();
        for (index, function) in self.functions.iter().enumerate() {
            if !function.is_component {
                continue;
            }
            let depth = composition_depth(
                index,
                &self.functions,
                &components,
                &mut memo,
                &mut HashSet::new(),
            );
            if depth > maximum && incoming[index] == 0 {
                findings.push(CompositionAnalysis {
                    name: function.name.clone(),
                    span: function.span,
                    depth,
                });
            }
        }
        // A closed cycle has no root; retain one deterministic representative when it exceeds policy.
        if findings.is_empty()
            && let Some((index, function, depth)) = self
                .functions
                .iter()
                .enumerate()
                .filter(|(_, function)| function.is_component)
                .map(|(index, function)| {
                    let depth = composition_depth(
                        index,
                        &self.functions,
                        &components,
                        &mut memo,
                        &mut HashSet::new(),
                    );
                    (index, function, depth)
                })
                .filter(|(_, _, depth)| *depth > maximum)
                .min_by_key(|(_, function, _)| &function.name)
        {
            let _ = index;
            findings.push(CompositionAnalysis {
                name: function.name.clone(),
                span: function.span,
                depth,
            });
        }
        findings
    }

    /// Finds directly consumed reactive helpers whose names hide their lifecycle role.
    pub(crate) fn unnamed_composables(&self) -> Vec<UnnamedComposableAnalysis> {
        let indices = unique_function_indices(self.functions.iter().enumerate());
        let mut reactive = self
            .functions
            .iter()
            .map(|function| function.is_composable || !function.reactive_primitives.is_empty())
            .collect::<Vec<_>>();
        loop {
            let prior = reactive.clone();
            for (index, function) in self.functions.iter().enumerate() {
                reactive[index] |= function
                    .function_calls
                    .iter()
                    .filter_map(|call| indices.get(call))
                    .any(|callee| prior[*callee]);
            }
            if reactive == prior {
                break;
            }
        }
        let called_by_owner = self
            .functions
            .iter()
            .filter(|function| function.is_reactive_owner())
            .flat_map(|function| function.function_calls.iter())
            .collect::<BTreeSet<_>>();
        self.functions
            .iter()
            .enumerate()
            .filter(|(index, function)| {
                reactive[*index]
                    && !function.is_component
                    && !function.is_composable
                    && called_by_owner.contains(&function.name)
            })
            .map(|(_, function)| UnnamedComposableAnalysis {
                name: function.name.clone(),
                span: function.span,
            })
            .collect()
    }

    /// Groups maximal nonoverlapping normalized RSX clones and reports later occurrences.
    pub(crate) fn repeated_fragments(
        &self,
        minimum_nodes: usize,
        minimum_occurrences: usize,
    ) -> Vec<RepeatedFragmentAnalysis> {
        let mut groups: BTreeMap<&str, Vec<&FragmentAnalysis>> = BTreeMap::new();
        for fragment in &self.fragments {
            if fragment.nodes >= minimum_nodes {
                groups
                    .entry(&fragment.fingerprint)
                    .or_default()
                    .push(fragment);
            }
        }
        let mut repeated = groups
            .into_values()
            .filter(|group| group.len() >= minimum_occurrences)
            .collect::<Vec<_>>();
        repeated.sort_by(|left, right| {
            right[0]
                .nodes
                .cmp(&left[0].nodes)
                .then_with(|| left[0].fingerprint.cmp(&right[0].fingerprint))
        });
        let mut claimed: HashMap<&str, Vec<Range<usize>>> = HashMap::new();
        let mut findings = Vec::new();
        for group in repeated {
            let available = group
                .into_iter()
                .filter(|occurrence| {
                    !claimed
                        .get(occurrence.owner.as_str())
                        .is_some_and(|ranges| {
                            ranges.iter().any(|range| {
                                range.start < occurrence.range.end
                                    && occurrence.range.start < range.end
                            })
                        })
                })
                .collect::<Vec<_>>();
            if available.len() < minimum_occurrences {
                continue;
            }
            let first = available[0];
            for occurrence in available.iter().skip(1) {
                findings.push(RepeatedFragmentAnalysis {
                    span: occurrence.span,
                    first_span: first.span,
                    nodes: occurrence.nodes,
                    occurrences: available.len(),
                });
            }
            for occurrence in available {
                claimed
                    .entry(occurrence.owner.as_str())
                    .or_default()
                    .push(occurrence.range.clone());
            }
        }
        findings
    }
}

fn unique_function_indices<'function>(
    functions: impl Iterator<Item = (usize, &'function FunctionAnalysis)>,
) -> BTreeMap<String, usize> {
    let mut grouped: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, function) in functions {
        grouped
            .entry(function.name.clone())
            .or_default()
            .push(index);
    }
    grouped
        .into_iter()
        .filter_map(|(name, indices)| {
            let [index] = indices.as_slice() else {
                return None;
            };
            Some((name, *index))
        })
        .collect()
}

fn composition_depth(
    index: usize,
    functions: &[FunctionAnalysis],
    components: &BTreeMap<String, usize>,
    memo: &mut HashMap<usize, usize>,
    visiting: &mut HashSet<usize>,
) -> usize {
    if let Some(depth) = memo.get(&index) {
        return *depth;
    }
    if !visiting.insert(index) {
        return 1;
    }
    let child = functions[index]
        .component_calls
        .iter()
        .filter_map(|name| components.get(name))
        .map(|child| composition_depth(*child, functions, components, memo, visiting))
        .max()
        .unwrap_or(0);
    visiting.remove(&index);
    let depth = child + 1;
    memo.insert(index, depth);
    depth
}

// -----------------------------------------------------------------------------
// Source collection
// -----------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn collect_items(
    document: &SourceDocument,
    offsets: &SourceOffsets,
    aliases: &BTreeSet<String>,
    items: &[Item],
    module: &str,
    functions: &mut Vec<FunctionAnalysis>,
    modules: &mut Vec<ModuleAnalysis>,
    fragments: &mut Vec<FragmentAnalysis>,
) {
    let mut module_components = Vec::new();
    for item in items {
        match item {
            Item::Fn(function) => {
                let analysis =
                    analyze_function(document, offsets, aliases, function, module, fragments);
                if analysis.is_component {
                    module_components.push(analysis.span);
                }
                functions.push(analysis);
            }
            Item::Mod(ItemMod {
                ident,
                content: Some((_, nested)),
                ..
            }) => {
                let nested_module = format!("{module}::{ident}");
                collect_items(
                    document,
                    offsets,
                    aliases,
                    nested,
                    &nested_module,
                    functions,
                    modules,
                    fragments,
                );
            }
            _ => {}
        }
    }
    if let Some(span) = module_components.first() {
        modules.push(ModuleAnalysis {
            name: module.to_owned(),
            span: *span,
            components: module_components.len(),
        });
    }
}

fn analyze_function(
    document: &SourceDocument,
    offsets: &SourceOffsets,
    aliases: &BTreeSet<String>,
    function: &ItemFn,
    module: &str,
    fragments: &mut Vec<FragmentAnalysis>,
) -> FunctionAnalysis {
    let name = function.sig.ident.to_string();
    let is_component = function.attrs.iter().any(|attribute| {
        attribute.path().segments.last().is_some_and(|segment| {
            matches!(segment.ident.to_string().as_str(), "component" | "island")
        })
    });
    let is_composable = name.starts_with("use_");
    let closures = named_closures(&function.block.stmts);
    let owner = format!("{module}::{name}");
    let mut collector = FunctionCollector {
        document,
        offsets,
        aliases,
        closures,
        owner: &owner,
        has_view: false,
        reactive_primitives: Vec::new(),
        handlers: Vec::new(),
        handler_ranges: BTreeSet::new(),
        spawned_tasks: Vec::new(),
        max_view_depth: 0,
        max_view_control_depth: 0,
        component_calls: BTreeSet::new(),
        function_calls: BTreeSet::new(),
        fragments,
    };
    collector.visit_block(&function.block);
    let setup_statements = function
        .block
        .stmts
        .len()
        .saturating_sub(usize::from(matches!(
            function.block.stmts.last(),
            Some(Stmt::Expr(_, None))
        )));
    let props = function
        .sig
        .inputs
        .iter()
        .filter(|argument| match argument {
            FnArg::Receiver(_) => false,
            FnArg::Typed(argument) => {
                let ty = argument.ty.to_token_stream().to_string().replace(' ', "");
                let name = match argument.pat.as_ref() {
                    Pat::Ident(ident) => ident.ident.to_string(),
                    _ => String::new(),
                };
                name != "children"
                    && !ty
                        .split("::")
                        .last()
                        .is_some_and(|ty| ty.starts_with("Children"))
            }
        })
        .count();
    FunctionAnalysis {
        name,
        span: document.span(offsets.range(function.sig.ident.span())),
        is_component,
        is_composable,
        setup_statements,
        reactive_primitives: collector.reactive_primitives,
        handlers: collector.handlers,
        spawned_tasks: collector.spawned_tasks,
        max_view_depth: collector.max_view_depth,
        max_view_control_depth: collector.max_view_control_depth,
        component_calls: collector.component_calls,
        function_calls: collector.function_calls,
        props,
    }
}

fn named_closures(statements: &[Stmt]) -> BTreeMap<String, ExprClosure> {
    statements
        .iter()
        .filter_map(|statement| {
            let Stmt::Local(local) = statement else {
                return None;
            };
            let Pat::Ident(binding) = &local.pat else {
                return None;
            };
            let Expr::Closure(closure) = local.init.as_ref()?.expr.as_ref() else {
                return None;
            };
            Some((binding.ident.to_string(), closure.clone()))
        })
        .collect()
}

struct FunctionCollector<'analysis> {
    document: &'analysis SourceDocument,
    offsets: &'analysis SourceOffsets,
    aliases: &'analysis BTreeSet<String>,
    closures: BTreeMap<String, ExprClosure>,
    owner: &'analysis str,
    has_view: bool,
    reactive_primitives: Vec<Span>,
    handlers: Vec<HandlerAnalysis>,
    handler_ranges: BTreeSet<(usize, usize)>,
    spawned_tasks: Vec<Span>,
    max_view_depth: usize,
    max_view_control_depth: usize,
    component_calls: BTreeSet<String>,
    function_calls: BTreeSet<String>,
    fragments: &'analysis mut Vec<FragmentAnalysis>,
}

impl FunctionCollector<'_> {
    fn collect_handler(&mut self, expression: &Expr) {
        let closure = match expression {
            Expr::Closure(closure) => Some(closure),
            Expr::Path(path) => path
                .path
                .segments
                .last()
                .and_then(|segment| self.closures.get(&segment.ident.to_string())),
            _ => None,
        };
        let Some(closure) = closure else { return };
        let range = self.offsets.range(closure.span());
        if !self.handler_ranges.insert((range.start, range.end)) {
            return;
        }
        let (statements, body) = match closure.body.as_ref() {
            Expr::Block(block) => (block.block.stmts.len(), closure.body.as_ref()),
            body => (1, body),
        };
        let mut complexity = ControlDepth::default();
        complexity.visit_expr(body);
        self.handlers.push(HandlerAnalysis {
            span: self.document.span(range),
            statements,
            control_depth: complexity.maximum,
        });
    }

    fn collect_view_node(&mut self, node: &Node, depth: usize) -> Option<Fingerprint> {
        match node {
            Node::Element(element) => Some(self.collect_element(element, depth + 1)),
            Node::Fragment(fragment) => {
                let children = fragment
                    .children
                    .iter()
                    .filter_map(|child| self.collect_view_node(child, depth))
                    .collect::<Vec<_>>();
                Some(Fingerprint::join("fragment", &children))
            }
            Node::Block(block) => {
                let block = block.try_block()?;
                let mut control = ControlDepth::default();
                control.visit_block(block);
                self.max_view_control_depth = self.max_view_control_depth.max(control.maximum);
                self.visit_block(block);
                Some(Fingerprint {
                    text: format!("expr:{}", expression_family_block(block)),
                    nodes: 1,
                })
            }
            Node::Text(_) | Node::RawText(_) => Some(Fingerprint {
                text: "text".to_owned(),
                nodes: 1,
            }),
            Node::Comment(_) | Node::Doctype(_) | Node::Custom(_) => None,
        }
    }

    fn collect_element(
        &mut self,
        element: &NodeElement<rstml::Infallible>,
        depth: usize,
    ) -> Fingerprint {
        self.max_view_depth = self.max_view_depth.max(depth);
        let name = element.name().to_string().replace(' ', "");
        let terminal = name.rsplit("::").next().unwrap_or(&name).to_owned();
        if terminal.chars().next().is_some_and(char::is_uppercase) {
            self.component_calls.insert(terminal);
        }
        let mut attributes = Vec::new();
        for attribute in element.attributes() {
            match attribute {
                NodeAttribute::Attribute(attribute) => {
                    let key = attribute.key.to_string().replace(' ', "");
                    let value = attribute.value().map_or("flag", expression_family);
                    if key.starts_with("on:")
                        && let Some(value) = attribute.value()
                    {
                        self.collect_handler(value);
                    }
                    if let Some(value) = attribute.value() {
                        self.visit_expr(value);
                    }
                    attributes.push(format!("{key}={value}"));
                }
                NodeAttribute::Block(_) => attributes.push("{attrs}".to_owned()),
            }
        }
        let children = element
            .children
            .iter()
            .filter_map(|child| self.collect_view_node(child, depth))
            .collect::<Vec<_>>();
        let children_nodes = children.iter().map(|child| child.nodes).sum::<usize>();
        let text = format!(
            "<{name} {}>[{}]",
            attributes.join(","),
            children
                .iter()
                .map(|child| child.text.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
        let fingerprint = Fingerprint {
            text,
            nodes: 1 + children_nodes,
        };
        self.fragments.push(FragmentAnalysis {
            span: self.document.span(self.offsets.range(element.span())),
            fingerprint: fingerprint.text.clone(),
            nodes: fingerprint.nodes,
            owner: self.owner.to_owned(),
            range: self.offsets.range(element.span()),
        });
        fingerprint
    }

    fn collect_view(&mut self, mac: &Macro) {
        self.has_view = true;
        let parser = rstml::Parser::new(rstml::ParserConfig::default().recover_block(true));
        let (nodes, errors) = parser.parse_recoverable(mac.tokens.clone()).split_vec();
        if !errors.is_empty() {
            return;
        }
        for node in &nodes {
            let _ = self.collect_view_node(node, 0);
        }
    }
}

impl<'ast> Visit<'ast> for FunctionCollector<'_> {
    fn visit_expr_call(&mut self, call: &'ast ExprCall) {
        let terminal = call_terminal(&call.func);
        if let Some(terminal) = terminal.as_deref() {
            self.function_calls.insert(terminal.to_owned());
            if is_reactive_primitive(call, terminal) {
                self.reactive_primitives
                    .push(self.document.span(self.offsets.range(call.span())));
            }
            if terminal == "spawn_local" || self.aliases.contains(terminal) {
                self.spawned_tasks
                    .push(self.document.span(self.offsets.range(call.span())));
            }
            if terminal == "new"
                && path_text(&call.func).ends_with("Callback::new")
                && let Some(handler) = call.args.first()
            {
                self.collect_handler(handler);
            }
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        if mac
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "view")
        {
            self.collect_view(mac);
        }
        visit::visit_macro(self, mac);
    }
}

#[derive(Clone)]
struct Fingerprint {
    text: String,
    nodes: usize,
}

impl Fingerprint {
    fn join(name: &str, children: &[Self]) -> Self {
        Self {
            text: format!(
                "{name}[{}]",
                children
                    .iter()
                    .map(|child| child.text.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            nodes: children.iter().map(|child| child.nodes).sum(),
        }
    }
}

#[derive(Default)]
struct ControlDepth {
    current: usize,
    maximum: usize,
}

impl ControlDepth {
    fn nested(&mut self, visit: impl FnOnce(&mut Self)) {
        self.current += 1;
        self.maximum = self.maximum.max(self.current);
        visit(self);
        self.current -= 1;
    }
}

impl<'ast> Visit<'ast> for ControlDepth {
    fn visit_expr_if(&mut self, expression: &'ast syn::ExprIf) {
        self.nested(|visitor| visit::visit_expr_if(visitor, expression));
    }
    fn visit_expr_match(&mut self, expression: &'ast syn::ExprMatch) {
        self.nested(|visitor| visit::visit_expr_match(visitor, expression));
    }
    fn visit_expr_for_loop(&mut self, expression: &'ast syn::ExprForLoop) {
        self.nested(|visitor| visit::visit_expr_for_loop(visitor, expression));
    }
    fn visit_expr_while(&mut self, expression: &'ast syn::ExprWhile) {
        self.nested(|visitor| visit::visit_expr_while(visitor, expression));
    }
    fn visit_expr_loop(&mut self, expression: &'ast syn::ExprLoop) {
        self.nested(|visitor| visit::visit_expr_loop(visitor, expression));
    }
    fn visit_expr_closure(&mut self, expression: &'ast ExprClosure) {
        self.nested(|visitor| visit::visit_expr_closure(visitor, expression));
    }
    fn visit_expr_call(&mut self, expression: &'ast ExprCall) {
        if call_terminal(&expression.func).as_deref() == Some("spawn_local") {
            return;
        }
        visit::visit_expr_call(self, expression);
    }

    fn visit_expr_macro(&mut self, expression: &'ast syn::ExprMacro) {
        if expression
            .mac
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "view")
        {
            self.nested(|_| {});
        }
        visit::visit_expr_macro(self, expression);
    }
}

fn is_reactive_primitive(call: &ExprCall, terminal: &str) -> bool {
    if matches!(
        terminal,
        "signal"
            | "create_signal"
            | "create_rw_signal"
            | "create_memo"
            | "create_effect"
            | "create_resource"
            | "create_local_resource"
            | "create_action"
            | "create_node_ref"
            | "create_trigger"
            | "store_value"
    ) {
        return true;
    }
    if !matches!(terminal, "new" | "new_local" | "derive") {
        return false;
    }
    let path = path_text(&call.func).replace(' ', "");
    [
        "RwSignal::new",
        "ArcRwSignal::new",
        "Store::new",
        "Resource::new",
        "LocalResource::new",
        "Action::new",
        "Action::new_local",
        "MultiAction::new",
        "MultiAction::new_local",
        "Memo::new",
        "Signal::derive",
        "Effect::new",
        "NodeRef::new",
        "Trigger::new",
        "StoredValue::new",
    ]
    .iter()
    .any(|suffix| path.ends_with(suffix))
}

fn call_terminal(expression: &Expr) -> Option<String> {
    let Expr::Path(path) = expression else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn path_text(expression: &Expr) -> String {
    expression.to_token_stream().to_string()
}

const fn expression_family(expression: &Expr) -> &'static str {
    match expression {
        Expr::Array(_) => "array",
        Expr::Async(_) => "async",
        Expr::Binary(_) => "binary",
        Expr::Block(_) => "block",
        Expr::Call(_) => "call",
        Expr::Closure(_) => "closure",
        Expr::Field(_) => "field",
        Expr::If(_) => "if",
        Expr::Lit(_) => "literal",
        Expr::Macro(_) => "macro",
        Expr::Match(_) => "match",
        Expr::MethodCall(_) => "method",
        Expr::Path(_) => "binding",
        Expr::Tuple(_) => "tuple",
        _ => "expression",
    }
}

fn expression_family_block(block: &syn::Block) -> &'static str {
    block
        .stmts
        .last()
        .map_or("block", |statement| match statement {
            Stmt::Expr(expression, _) => expression_family(expression),
            _ => "block",
        })
}

fn spawn_aliases(items: &[Item]) -> BTreeSet<String> {
    let mut aliases = BTreeSet::from(["spawn_local".to_owned()]);
    collect_spawn_aliases_from_items(items, &mut aliases);
    aliases
}

fn collect_spawn_aliases_from_items(items: &[Item], aliases: &mut BTreeSet<String>) {
    for item in items {
        match item {
            Item::Use(item) => collect_spawn_alias(&item.tree, false, aliases),
            Item::Mod(ItemMod {
                content: Some((_, nested)),
                ..
            }) => {
                collect_spawn_aliases_from_items(nested, aliases);
            }
            _ => {}
        }
    }
}

fn collect_spawn_alias(tree: &syn::UseTree, beneath_spawn: bool, aliases: &mut BTreeSet<String>) {
    match tree {
        syn::UseTree::Path(path) => {
            collect_spawn_alias(
                &path.tree,
                beneath_spawn || path.ident == "spawn_local",
                aliases,
            );
        }
        syn::UseTree::Name(name) if beneath_spawn || name.ident == "spawn_local" => {
            aliases.insert(name.ident.to_string());
        }
        syn::UseTree::Rename(rename) if beneath_spawn || rename.ident == "spawn_local" => {
            aliases.insert(rename.rename.to_string());
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_spawn_alias(item, beneath_spawn, aliases);
            }
        }
        _ => {}
    }
}

// -----------------------------------------------------------------------------
// SourceOffsets: Proc-macro span conversion
// -----------------------------------------------------------------------------

struct SourceOffsets {
    line_starts: Vec<usize>,
    source_len: usize,
}

impl From<&str> for SourceOffsets {
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
    fn offset(&self, position: LineColumn) -> usize {
        self.line_starts
            .get(position.line.saturating_sub(1))
            .copied()
            .unwrap_or(self.source_len)
            .saturating_add(position.column)
            .min(self.source_len)
    }

    fn range(&self, span: TokenSpan) -> Range<usize> {
        self.offset(span.start())..self.offset(span.end())
    }
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use syn::visit::Visit;

    use super::{ControlDepth, expression_family};

    #[test]
    fn classifies_bindings_without_retaining_generated_names() {
        assert_eq!(expression_family(&syn::parse_quote!(first_name)), "binding");
        assert_eq!(
            expression_family(&syn::parse_quote!(second_name)),
            "binding"
        );
    }

    #[test]
    fn measures_nested_handler_control_flow() {
        let expression = syn::parse_quote!(if ready {
            match value {
                Some(value) => value,
                None => 0,
            }
        } else {
            0
        });
        let mut depth = ControlDepth::default();
        depth.visit_expr(&expression);
        assert_eq!(depth.maximum, 2);
    }
}
