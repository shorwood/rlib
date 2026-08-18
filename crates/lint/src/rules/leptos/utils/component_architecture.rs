extern crate rustc_span;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::ops::Range;

use proc_macro2::{LineColumn, Span as TokenSpan};
use quote::ToTokens;
use rstml::node::{Node, NodeAttribute, NodeElement};
use rustc_span::Span;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};
use syn::{Expr, ExprCall, ExprClosure, FnArg, Item, ItemFn, ItemMod, Macro, Pat, Stmt};

use super::authored_files::SourceDocument;

// -----------------------------------------------------------------------------
// HandlerAnalysis: Event handler complexity evidence
// -----------------------------------------------------------------------------

/// One event handler found in a view attribute or `Callback::new` expression.
pub struct HandlerAnalysis {
    /// Authored span of the handler closure.
    pub(crate) span: Span,
    /// Number of statements in the handler body.
    pub(crate) statements: usize,
    /// Deepest nested control-flow construct in the handler body.
    pub(crate) control_depth: usize,
}

// -----------------------------------------------------------------------------
// FragmentAnalysis: Internal normalized view fragment evidence
// -----------------------------------------------------------------------------

/// One normalized view subtree that is large enough to become a clone candidate.
struct FragmentAnalysis {
    /// Authored span of the candidate subtree.
    span: Span,
    /// Binding- and literal-independent subtree representation.
    fingerprint: String,
    /// Number of normalized nodes in the subtree.
    nodes: usize,
    /// Fully qualified function name containing the subtree.
    owner: String,
    /// Source byte range used to reject overlapping candidates.
    range: Range<usize>,
}

// -----------------------------------------------------------------------------
// FunctionAnalysis: Authored function architecture evidence
// -----------------------------------------------------------------------------

/// Source facts about one authored function.
pub struct FunctionAnalysis {
    /// Function identifier used to resolve local calls.
    pub(crate) name: String,
    /// Authored span of the function identifier.
    pub(crate) span: Span,
    /// Whether the function is annotated as a Leptos component or island.
    pub(crate) is_component: bool,
    /// Whether the function follows the `use_` composable naming convention.
    is_composable: bool,
    /// Number of statements before the function's tail expression.
    pub(crate) setup_statements: usize,
    /// Spans of reactive primitive constructions in the function.
    pub(crate) reactive_primitives: Vec<Span>,
    /// Event handlers declared or referenced by the function's view.
    pub(crate) handlers: Vec<HandlerAnalysis>,
    /// Spans of local task-spawning calls in the function.
    pub(crate) spawned_tasks: Vec<Span>,
    /// Deepest nested RSX element in the function's view.
    pub(crate) max_view_depth: usize,
    /// Deepest control-flow expression in the function's view.
    pub(crate) max_view_control_depth: usize,
    /// Component names invoked from the function's view.
    component_calls: BTreeSet<String>,
    /// Function names invoked from the function body.
    function_calls: BTreeSet<String>,
    /// Number of non-children component parameters.
    pub(crate) props: usize,
}

impl FunctionAnalysis {
    /// Collects architecture facts from one authored function.
    fn analyze(
        document: &SourceDocument,
        offsets: &SourceOffsets,
        aliases: &BTreeSet<String>,
        function: &ItemFn,
        module: &str,
        fragments: &mut Vec<FragmentAnalysis>,
    ) -> Self {
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
        Self {
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

    /// Returns whether the function owns a reactive scope by convention.
    pub(crate) const fn is_reactive_owner(&self) -> bool {
        self.is_component || self.is_composable
    }
}

// -----------------------------------------------------------------------------
// ModuleAnalysis: Authored module component-density evidence
// -----------------------------------------------------------------------------

/// A module containing more authored component entry points than policy permits.
pub struct ModuleAnalysis {
    /// Fully qualified authored module name.
    pub(crate) name: String,
    /// Span of the module's first component.
    pub(crate) span: Span,
    /// Number of direct component functions in the module.
    pub(crate) components: usize,
}

// -----------------------------------------------------------------------------
// UnnamedComposableAnalysis: Reactive helper naming evidence
// -----------------------------------------------------------------------------

/// One helper that performs reactive work without composable naming.
pub struct UnnamedComposableAnalysis {
    /// Authored helper name.
    pub(crate) name: String,
    /// Authored span of the helper identifier.
    pub(crate) span: Span,
}

// -----------------------------------------------------------------------------
// CompositionAnalysis: Component call-chain depth evidence
// -----------------------------------------------------------------------------

/// One root component whose local composition chain exceeds policy.
pub struct CompositionAnalysis {
    /// Root component name.
    pub(crate) name: String,
    /// Authored span of the root component identifier.
    pub(crate) span: Span,
    /// Deepest component call chain reachable from the root.
    pub(crate) depth: usize,
}

// -----------------------------------------------------------------------------
// RepeatedFragment: Repeated normalized subtree evidence and policy
// -----------------------------------------------------------------------------

/// One repeated maximal view fragment after literal and binding normalization.
pub struct RepeatedFragmentAnalysis {
    /// Authored span of the later repeated fragment.
    pub(crate) span: Span,
    /// Authored span of the first matching fragment.
    pub(crate) first_span: Span,
    /// Number of normalized nodes in the fragment.
    pub(crate) nodes: usize,
    /// Number of nonoverlapping occurrences in the clone group.
    pub(crate) occurrences: usize,
}

/// Named thresholds for repeated view-fragment detection.
#[derive(Clone, Copy)]
pub struct RepeatedFragmentPolicy {
    /// Minimum normalized node count for a candidate fragment.
    pub(crate) minimum_nodes: usize,
    /// Minimum nonoverlapping occurrences required for a finding.
    pub(crate) minimum_occurrences: usize,
}

// -----------------------------------------------------------------------------
// ArchitectureAnalysis: Crate-wide authored-source query model
// -----------------------------------------------------------------------------

/// Complete crate-wide evidence reused by every architecture policy.
pub struct ArchitectureAnalysis {
    /// Facts collected from every authored function.
    pub(crate) functions: Vec<FunctionAnalysis>,
    /// Component density facts collected from every authored module.
    pub(crate) modules: Vec<ModuleAnalysis>,
    /// Internal normalized view fragments used for clone detection.
    fragments: Vec<FragmentAnalysis>,
}

impl ArchitectureAnalysis {
    /// Parses all authored files into a deterministic crate-wide architecture model.
    pub(crate) fn analyze(documents: Vec<SourceDocument>) -> Self {
        let mut functions = Vec::new();
        let mut modules = Vec::new();
        let mut fragments = Vec::new();
        for document in documents {
            // A syntax error belongs to rustc; architecture findings require a complete source tree.
            let Ok(file) = syn::parse_file(&document.source) else {
                continue;
            };

            // Resolve file-local names and proc-macro coordinates before collecting evidence.
            let offsets = SourceOffsets::from(document.source.as_str());
            let aliases = spawn_aliases(&file.items);
            let root = document
                .path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("crate")
                .to_owned();

            // Merge this authored file into the crate-wide query model.
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
        let components =
            FunctionGraph::unique_indices(&self.functions, |function| function.is_component);
        let mut incoming = vec![0usize; self.functions.len()];
        for function in &self.functions {
            for call in &function.component_calls {
                let Some(index) = components.get(call) else {
                    continue;
                };
                incoming[*index] += 1;
            }
        }
        let mut memo = HashMap::new();
        let mut findings = Vec::new();
        for (index, function) in self.functions.iter().enumerate() {
            if !function.is_component {
                continue;
            }
            let depth = FunctionGraph::composition_depth(
                index,
                &self.functions,
                &components,
                &mut memo,
                &mut HashSet::new(),
            );
            if depth <= maximum || incoming[index] != 0 {
                continue;
            }
            findings.push(CompositionAnalysis {
                name: function.name.clone(),
                span: function.span,
                depth,
            });
        }

        // A closed cycle has no root; retain one deterministic representative when it exceeds policy.
        let mut cyclic_candidate = None::<CompositionCandidate<'_>>;
        if findings.is_empty() {
            for (index, function) in self.functions.iter().enumerate() {
                if !function.is_component {
                    continue;
                }
                let depth = FunctionGraph::composition_depth(
                    index,
                    &self.functions,
                    &components,
                    &mut memo,
                    &mut HashSet::new(),
                );
                if depth <= maximum {
                    continue;
                }
                let replaces_candidate = cyclic_candidate
                    .as_ref()
                    .is_none_or(|candidate| function.name < candidate.function.name);
                if !replaces_candidate {
                    continue;
                }
                cyclic_candidate = Some(CompositionCandidate { function, depth });
            }
        }
        if let Some(CompositionCandidate { function, depth }) = cyclic_candidate {
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
        let indices = FunctionGraph::unique_indices(&self.functions, |_| true);
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
        policy: RepeatedFragmentPolicy,
    ) -> Vec<RepeatedFragmentAnalysis> {
        let mut groups: BTreeMap<&str, Vec<&FragmentAnalysis>> = BTreeMap::new();
        for fragment in &self.fragments {
            if fragment.nodes < policy.minimum_nodes {
                continue;
            }
            groups
                .entry(&fragment.fingerprint)
                .or_default()
                .push(fragment);
        }
        let mut repeated = groups
            .into_values()
            .filter(|group| group.len() >= policy.minimum_occurrences)
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
            if available.len() < policy.minimum_occurrences {
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

// -----------------------------------------------------------------------------
// CompositionCandidate: Deterministic cyclic composition representative
// -----------------------------------------------------------------------------

/// One overlong composition candidate used to choose a cyclic representative.
struct CompositionCandidate<'analysis> {
    /// Component whose call chain exceeds policy.
    function: &'analysis FunctionAnalysis,
    /// Deepest component call chain reachable from the candidate.
    depth: usize,
}

// -----------------------------------------------------------------------------
// FunctionGraph: Unambiguous local function and component call resolution
// -----------------------------------------------------------------------------

/// Helpers for resolving uniquely named functions and component-call depth.
struct FunctionGraph;

impl FunctionGraph {
    /// Maps included, uniquely named functions to their crate-wide analysis index.
    fn unique_indices(
        functions: &[FunctionAnalysis],
        include: impl Fn(&FunctionAnalysis) -> bool,
    ) -> BTreeMap<String, usize> {
        let mut grouped: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, function) in functions.iter().enumerate() {
            if !include(function) {
                continue;
            }
            grouped
                .entry(function.name.clone())
                .or_default()
                .push(index);
        }
        grouped
            .into_iter()
            .filter_map(|(name, indices)| {
                // Ambiguous duplicate names cannot resolve to one local function.
                let [index] = indices.as_slice() else {
                    return None;
                };
                Some((name, *index))
            })
            .collect()
    }

    /// Computes the maximum component depth reachable from one function index.
    fn composition_depth(
        index: usize,
        functions: &[FunctionAnalysis],
        components: &BTreeMap<String, usize>,
        memo: &mut HashMap<usize, usize>,
        visiting: &mut HashSet<usize>,
    ) -> usize {
        // Reuse completed subgraphs when several roots share a descendant.
        if let Some(depth) = memo.get(&index) {
            return *depth;
        }

        // Count a back edge as one node so closed component cycles remain finite.
        if !visiting.insert(index) {
            return 1;
        }
        let child = functions[index]
            .component_calls
            .iter()
            .filter_map(|name| components.get(name))
            .map(|child| Self::composition_depth(*child, functions, components, memo, visiting))
            .max()
            .unwrap_or(0);
        visiting.remove(&index);
        let depth = child + 1;
        memo.insert(index, depth);
        depth
    }
}

// -----------------------------------------------------------------------------
// HandlerSourceRange: Referenced event-handler deduplication coordinates
// -----------------------------------------------------------------------------

/// Comparable source coordinates used to deduplicate referenced handler closures.
#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct HandlerSourceRange {
    /// Inclusive byte offset of the closure start.
    start: usize,
    /// Exclusive byte offset of the closure end.
    end: usize,
}

// -----------------------------------------------------------------------------
// FunctionCollector: Authored function and view evidence collection
// -----------------------------------------------------------------------------

/// Visitor that accumulates architecture evidence for one authored function.
struct FunctionCollector<'analysis> {
    /// File-backed source document used to construct rustc spans.
    document: &'analysis SourceDocument,
    /// Proc-macro line and column converter for the source document.
    offsets: &'analysis SourceOffsets,
    /// Names that resolve to the Leptos `spawn_local` function.
    aliases: &'analysis BTreeSet<String>,
    /// Named local closures that may be referenced as event handlers.
    closures: BTreeMap<String, ExprClosure>,
    /// Fully qualified name of the function being visited.
    owner: &'analysis str,
    /// Spans of reactive primitive constructions.
    reactive_primitives: Vec<Span>,
    /// Event handler complexity evidence.
    handlers: Vec<HandlerAnalysis>,
    /// Source ranges used to avoid collecting one handler more than once.
    handler_ranges: BTreeSet<HandlerSourceRange>,
    /// Spans of calls that launch local asynchronous work.
    spawned_tasks: Vec<Span>,
    /// Deepest nested RSX element in the function.
    max_view_depth: usize,
    /// Deepest control-flow expression in the function's views.
    max_view_control_depth: usize,
    /// Component names invoked from the function's views.
    component_calls: BTreeSet<String>,
    /// Function names invoked from the function body.
    function_calls: BTreeSet<String>,
    /// Crate-wide destination for normalized view-fragment candidates.
    fragments: &'analysis mut Vec<FragmentAnalysis>,
}

impl FunctionCollector<'_> {
    /// Collects one closure-valued event handler without duplicate references.
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

        // Expressions that do not resolve to a closure are not event handlers.
        let Some(closure) = closure else {
            return;
        };
        let range = self.offsets.range(closure.span());

        // A named closure may be referenced by several attributes but is one handler.
        if !self.handler_ranges.insert(HandlerSourceRange {
            start: range.start,
            end: range.end,
        }) {
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

    /// Normalizes one RSX node and recursively collects its nested evidence.
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

    /// Normalizes one RSX element and records it as a clone candidate.
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

    /// Parses and collects one Leptos `view!` macro invocation.
    fn collect_view(&mut self, mac: &Macro) {
        let parser = rstml::Parser::new(rstml::ParserConfig::default().recover_block(true));
        let (nodes, errors) = parser.parse_recoverable(mac.tokens.clone()).split_vec();

        // Rstml recovery may return partial nodes that are unsafe to compare as complete views.
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
            if reactive_primitive_call(call, terminal) {
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

/// Recursively collects functions and component counts from an authored module tree.
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
                let analysis = FunctionAnalysis::analyze(
                    document, offsets, aliases, function, module, fragments,
                );
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

    // Modules without direct components do not contribute density evidence.
    let Some(span) = module_components.first() else {
        return;
    };
    modules.push(ModuleAnalysis {
        name: module.to_owned(),
        span: *span,
        components: module_components.len(),
    });
}

/// Returns named local closures that can be referenced from view attributes.
fn named_closures(statements: &[Stmt]) -> BTreeMap<String, ExprClosure> {
    statements
        .iter()
        .filter_map(|statement| {
            // Only local bindings can provide handler aliases.
            let Stmt::Local(local) = statement else {
                return None;
            };
            // Destructuring patterns cannot be referenced as one handler name.
            let Pat::Ident(binding) = &local.pat else {
                return None;
            };
            // Non-closure values are irrelevant to handler complexity.
            let Expr::Closure(closure) = local.init.as_ref()?.expr.as_ref() else {
                return None;
            };
            Some((binding.ident.to_string(), closure.clone()))
        })
        .collect()
}

// -----------------------------------------------------------------------------
// Fingerprint: Binding-independent RSX subtree representation
// -----------------------------------------------------------------------------

/// Normalized RSX subtree text and its meaningful node count.
#[derive(Clone)]
struct Fingerprint {
    /// Binding- and literal-independent subtree text.
    text: String,
    /// Number of meaningful nodes represented by the text.
    nodes: usize,
}

impl Fingerprint {
    /// Joins child fingerprints under one normalized container name.
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

// -----------------------------------------------------------------------------
// ControlDepth: Nested handler and view control-flow measurement
// -----------------------------------------------------------------------------

/// Visitor state for the maximum nested control-flow depth in one expression.
#[derive(Default)]
struct ControlDepth {
    /// Current control-flow depth during traversal.
    current: usize,
    /// Maximum control-flow depth observed during traversal.
    maximum: usize,
}

impl ControlDepth {
    /// Visits one nested control-flow construct while maintaining depth state.
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
        // Spawned work has an independent control-flow lifecycle and is linted separately.
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

// -----------------------------------------------------------------------------
// CallTerminal: Direct function-path terminal extraction
// -----------------------------------------------------------------------------

/// Returns the terminal identifier when an expression is a direct function path.
fn call_terminal(expression: &Expr) -> Option<String> {
    // Method calls and computed callees have no stable local function identifier.
    let Expr::Path(path) = expression else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

// -----------------------------------------------------------------------------
// PathText: Rust expression path rendering
// -----------------------------------------------------------------------------

/// Renders an expression path without depending on rustc lowering.
fn path_text(expression: &Expr) -> String {
    expression.to_token_stream().to_string()
}

// -----------------------------------------------------------------------------
// ReactivePrimitive: Reactive construction classification
// -----------------------------------------------------------------------------

/// Free functions that directly construct one Leptos reactive primitive.
const REACTIVE_PRIMITIVE_FUNCTIONS: &[&str] = &[
    "signal",
    "create_signal",
    "create_rw_signal",
    "create_memo",
    "create_effect",
    "create_resource",
    "create_local_resource",
    "create_action",
    "create_node_ref",
    "create_trigger",
    "store_value",
];

/// Associated call suffixes that directly construct one Leptos reactive primitive.
const REACTIVE_PRIMITIVE_PATH_SUFFIXES: &[&str] = &[
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
];

/// Returns whether a call directly constructs one Leptos reactive primitive.
fn reactive_primitive_call(call: &ExprCall, terminal: &str) -> bool {
    // Free constructor functions unambiguously create reactive state.
    if REACTIVE_PRIMITIVE_FUNCTIONS.contains(&terminal) {
        return true;
    }

    // Only conventional associated constructors need path-level classification.
    if !matches!(terminal, "new" | "new_local" | "derive") {
        return false;
    }
    let path = path_text(&call.func).replace(' ', "");
    REACTIVE_PRIMITIVE_PATH_SUFFIXES
        .iter()
        .any(|suffix| path.ends_with(suffix))
}

// -----------------------------------------------------------------------------
// ExpressionFamily: Binding-independent expression classification
// -----------------------------------------------------------------------------

/// Classifies an expression while deliberately discarding literals and binding names.
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

/// Classifies the tail expression of an RSX block.
fn expression_family_block(block: &syn::Block) -> &'static str {
    block
        .stmts
        .last()
        .map_or("block", |statement| match statement {
            Stmt::Expr(expression, _) => expression_family(expression),
            _ => "block",
        })
}

// -----------------------------------------------------------------------------
// SpawnAliasScope: Imported local-task launcher discovery
// -----------------------------------------------------------------------------

/// Import-tree position used while resolving aliases of `spawn_local`.
#[derive(Clone, Copy)]
enum SpawnAliasScope {
    /// Traversal is outside a `spawn_local` import path.
    Ordinary,
    /// Traversal is beneath a `spawn_local` import path.
    BeneathSpawn,
}

/// Resolves aliases while retaining whether traversal is beneath `spawn_local`.
fn spawn_alias_collect_tree(
    tree: &syn::UseTree,
    scope: SpawnAliasScope,
    aliases: &mut BTreeSet<String>,
) {
    match tree {
        syn::UseTree::Path(path) => {
            let nested_scope = if path.ident == "spawn_local" {
                SpawnAliasScope::BeneathSpawn
            } else {
                scope
            };
            spawn_alias_collect_tree(&path.tree, nested_scope, aliases);
        }
        syn::UseTree::Name(name)
            if matches!(scope, SpawnAliasScope::BeneathSpawn) || name.ident == "spawn_local" =>
        {
            aliases.insert(name.ident.to_string());
        }
        syn::UseTree::Rename(rename)
            if matches!(scope, SpawnAliasScope::BeneathSpawn) || rename.ident == "spawn_local" =>
        {
            aliases.insert(rename.rename.to_string());
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                spawn_alias_collect_tree(item, scope, aliases);
            }
        }
        _ => {}
    }
}

/// Recursively discovers `spawn_local` aliases in one module tree.
fn spawn_alias_collect_items(items: &[Item], aliases: &mut BTreeSet<String>) {
    for item in items {
        match item {
            Item::Use(item) => {
                spawn_alias_collect_tree(&item.tree, SpawnAliasScope::Ordinary, aliases);
            }
            Item::Mod(ItemMod {
                content: Some((_, nested)),
                ..
            }) => {
                spawn_alias_collect_items(nested, aliases);
            }
            _ => {}
        }
    }
}

/// Returns every authored name that resolves to Leptos `spawn_local`.
fn spawn_aliases(items: &[Item]) -> BTreeSet<String> {
    let mut aliases = BTreeSet::from(["spawn_local".to_owned()]);
    spawn_alias_collect_items(items, &mut aliases);
    aliases
}

// -----------------------------------------------------------------------------
// SourceOffsets: Proc-macro span conversion
// -----------------------------------------------------------------------------

/// Converts proc-macro line and column coordinates into source byte offsets.
struct SourceOffsets {
    /// Byte offset at which each authored source line begins.
    line_starts: Vec<usize>,
    /// Total source length used to clamp invalid recovered spans.
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
    /// Converts one proc-macro coordinate into a clamped source byte offset.
    fn offset(&self, position: LineColumn) -> usize {
        self.line_starts
            .get(position.line.saturating_sub(1))
            .copied()
            .unwrap_or(self.source_len)
            .saturating_add(position.column)
            .min(self.source_len)
    }

    /// Converts one proc-macro span into a clamped source byte range.
    fn range(&self, span: TokenSpan) -> Range<usize> {
        self.offset(span.start())..self.offset(span.end())
    }
}

// -----------------------------------------------------------------------------
// Tests: Component architecture source-analysis unit coverage
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
