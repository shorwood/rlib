extern crate rustc_abi;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_abi::ExternAbi;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{Body, Expr, ExprKind, HirId, Item, ItemKind, Node, PatKind};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::symbol::sym;
use rustc_span::{Span, Symbol};

use super::identifier_case;

// -----------------------------------------------------------------------------
// Construction: Candidate semantic record
// -----------------------------------------------------------------------------
#[derive(Clone, Copy, PartialEq, Eq)]
/// Container path from a function return type to the local value it constructs.
pub enum ConstructionReturn {
    /// The function returns the constructed type directly.
    Direct,
    /// The function returns the constructed type through at least one standard container.
    Contained,
    /// The function returns exactly `Result<T, E>` for the constructed `T`.
    FallibleDirect,
}
#[derive(Clone, Copy, PartialEq, Eq)]
/// Whether a constructor-like function is free or already associated with a type.
pub enum ConstructionOrigin {
    /// A module-level free function.
    Free,
    /// A receiver-free function in an inherent implementation.
    Inherent,
}
#[derive(Clone, Copy)]
/// Structural facts needed to recognize a canonical owned string parser.
pub struct ConstructionParserFacts {
    /// Whether the function takes exactly one immutable string slice.
    has_single_str_input: bool,
    /// Whether that string binding is referenced by the body.
    has_used_string_input: bool,
    /// Whether the target declares a lifetime that `FromStr` cannot return.
    has_target_lifetime: bool,
}
#[derive(Clone, Copy)]
/// Authored source properties used by conservative constructor relocation.
pub struct ConstructionMigrationFacts {
    /// Whether source visibility is private.
    pub(crate) is_private: bool,
    /// Whether attributes decorate the function declaration.
    pub(crate) has_attributes: bool,
}

// -----------------------------------------------------------------------------
// ConstructionCandidate: Function and target identity
// -----------------------------------------------------------------------------
#[derive(Clone)]
/// Authored function identity retained by one construction `candidate`.
pub struct ConstructionCandidateFunction {
    /// Function definition used to index resolved references.
    pub(crate) def_id: LocalDefId,
    /// Authored function identifier.
    pub(crate) name: Symbol,
    /// Function identifier source range.
    pub(crate) name_span: Span,
    /// Complete source range used by guarded migration.
    pub(crate) item_span: Span,
    /// Module containing the function and its construction target.
    pub(crate) module: LocalDefId,
}
#[derive(Clone)]
/// Constructed local type and the return contract that reaches it.
pub struct ConstructionCandidateTarget {
    /// Local nominal type constructed by the body.
    pub(crate) def_id: LocalDefId,
    /// Display name of the constructed type.
    pub(crate) name: Symbol,
    /// Standard-container shape around the constructed value.
    pub(super) return_shape: ConstructionReturn,
}
#[derive(Clone, Copy)]
/// Function ownership facts used to select the responsible lint policy.
pub struct ConstructionCandidateOwnership {
    /// Whether the function is module-level or already associated.
    pub(crate) origin: ConstructionOrigin,
    /// Whether the first parameter already supplies the constructed type.
    pub(crate) is_first_input_target: bool,
    /// Whether the function and constructed type are defined in the same module.
    pub(crate) is_target_same_module: bool,
}
#[derive(Clone)]
/// One authored function proven to construct a local nominal type.
pub struct ConstructionCandidate {
    /// Function identity and source ownership.
    pub(crate) function: ConstructionCandidateFunction,
    /// Local type and return-container contract.
    pub(crate) target: ConstructionCandidateTarget,
    /// Ownership and receiver-overlap facts.
    pub(crate) ownership: ConstructionCandidateOwnership,
    /// Text-parser-specific structural evidence.
    parser: ConstructionParserFacts,
    /// Source facts governing automatic relocation.
    pub(crate) migration: ConstructionMigrationFacts,
}

impl ConstructionCandidate {
    /// Returns whether this constructor exposes an exact `Result<T, E>` contract.
    #[cfg(any(feature = "bon", feature = "derive_more", feature = "serde"))]
    pub(crate) fn is_fallible_direct(&self) -> bool {
        self.target.return_shape == ConstructionReturn::FallibleDirect
    }

    /// Returns whether this `candidate` is a structurally canonical textual parser.
    pub(super) fn is_text_parser(&self) -> bool {
        self.ownership.is_target_same_module
            && self.target.return_shape == ConstructionReturn::FallibleDirect
            && self.parser.has_single_str_input
            && self.parser.has_used_string_input
            && !self.parser.has_target_lifetime
    }

    /// Returns whether the authored name describes one canonical, unqualified parser.
    pub(crate) fn has_unqualified_parser_name(&self) -> bool {
        // Establish the vocabulary that adds no format or policy qualification.
        let target_words = identifier_case::words(self.target.name.as_str());
        let neutral = [
            "Build", "Create", "Decode", "From", "Make", "New", "Parse", "Str", "String", "Text",
            "Try",
        ];

        // Treat every remaining authored word as an intentional qualifier.
        identifier_case::words(self.function.name.as_str())
            .into_iter()
            .all(|word| neutral.iter().any(|known| word == *known) || target_words.contains(&word))
    }

    /// Builds a crate-root-qualified associated-function path for reference rewrites.
    pub(crate) fn qualified_associated_path(&self, cx: &LateContext<'_>) -> String {
        let path = cx.tcx.def_path_str(self.target.def_id.to_def_id());
        let target_path = path.split_once("::").map_or_else(
            || format!("crate::{path}"),
            |(_, rest)| format!("crate::{rest}"),
        );
        format!("{target_path}::{}", self.function.name)
    }
}

// -----------------------------------------------------------------------------
// ConstructionInput: Resolved discovery inputs
// -----------------------------------------------------------------------------

/// Function ownership resolved before construction analysis.
struct ConstructionInputFunction {
    /// Whether the function is free or inherent.
    origin: ConstructionOrigin,
    /// Module that must also contain the constructed type.
    module: LocalDefId,
}

/// Local type and container shape resolved from a function return.
struct ConstructionInputTarget {
    /// Local nominal type reached through success containers.
    def_id: LocalDefId,
    /// Standard-container path leading to the type.
    return_shape: ConstructionReturn,
}

/// Source-level migration properties of one function declaration.
struct ConstructionInputSource {
    /// Complete authored item range.
    item_span: Span,
    /// Whether source visibility is private.
    is_private: bool,
    /// Whether attributes decorate the declaration.
    has_attributes: bool,
}
#[derive(Clone, Copy)]
/// Outermost container used to distinguish exact `Result<T, E>` returns.
enum ConstructionInputContainerRoot {
    /// No standard container has been traversed.
    Direct,
    /// The outermost container is `Result`.
    Result,
    /// The outermost container is another supported shape.
    Other,
}

// -----------------------------------------------------------------------------
// ConstructionAnalysis: Crate wide discovery
// -----------------------------------------------------------------------------
#[derive(Clone, Copy)]
/// One direct module item retained for guarded adjacency analysis.
pub struct ConstructionAnalysisModuleItem {
    /// Definition identity of the item.
    pub(crate) def_id: LocalDefId,
    /// Authored source range of the item.
    pub(crate) span: Span,
}
#[derive(Default)]
/// Crate-wide construction discovery shared by ownership and parser lints.
pub struct ConstructionAnalysis {
    /// Proven constructor-like functions in traversal order.
    pub(crate) candidates: Vec<ConstructionCandidate>,
    /// Direct authored items grouped by their module.
    pub(crate) module_items: HashMap<LocalDefId, Vec<ConstructionAnalysisModuleItem>>,
    /// Resolved path references grouped by function definition.
    pub(crate) function_uses: HashMap<LocalDefId, Vec<Span>>,
    /// Functions referenced by use declarations.
    pub(crate) imported_functions: HashSet<LocalDefId>,
    /// Local nominal types that already implement `FromStr`.
    pub(crate) from_str_targets: HashSet<LocalDefId>,
}

impl ConstructionAnalysis {
    /// Resolves an inherent associated function's owning module.
    fn inherent_function(
        cx: &LateContext<'_>,
        implementation: LocalDefId,
    ) -> Option<ConstructionInputFunction> {
        // Validate the implementation node before resolving its surrounding module.
        let Node::Item(item) = cx.tcx.hir_node_by_def_id(implementation) else {
            return None;
        };
        if !matches!(item.kind, ItemKind::Impl(_)) {
            return None;
        }

        // Preserve the inherent origin and the module shared with its target.
        let module = cx.tcx.opt_local_parent(implementation)?;
        Some(ConstructionInputFunction {
            origin: ConstructionOrigin::Inherent,
            module,
        })
    }

    /// Resolves whether a function is free or a receiver-free inherent associated function.
    fn function_context(
        cx: &LateContext<'_>,
        def_id: LocalDefId,
    ) -> Option<ConstructionInputFunction> {
        // Classify the direct HIR owner without following nested declarations.
        let parent = cx.tcx.opt_local_parent(def_id)?;
        if cx.tcx.def_kind(parent) == DefKind::Mod {
            return Some(ConstructionInputFunction {
                origin: ConstructionOrigin::Free,
                module: parent,
            });
        }
        if cx.tcx.def_kind(parent) == (DefKind::Impl { of_trait: false }) {
            return Self::inherent_function(cx, parent);
        }
        None
    }

    /// Recursively follows `Option` and `Result` success values to one local nominal type.
    fn constructed_target(cx: &LateContext<'_>, output: Ty<'_>) -> Option<ConstructionInputTarget> {
        Self::constructed_target_inner(cx, output, 0, ConstructionInputContainerRoot::Direct)
    }

    /// Carries container depth and outer shape through recursive success-value unwrapping.
    fn constructed_target_inner(
        cx: &LateContext<'_>,
        output: Ty<'_>,
        depth: usize,
        root: ConstructionInputContainerRoot,
    ) -> Option<ConstructionInputTarget> {
        // Only nominal values and the supported standard containers participate.
        let ty::Adt(definition, arguments) = output.kind() else {
            return None;
        };

        // Resolve a local terminal type and describe the traversed container path.
        if let Some(def_id) = definition.did().as_local() {
            // Classify the exact outer shape independently from the local target.
            let return_shape = match (depth, root) {
                (0, _) => ConstructionReturn::Direct,
                (1, ConstructionInputContainerRoot::Result) => ConstructionReturn::FallibleDirect,
                _ => ConstructionReturn::Contained,
            };

            // Return the terminal nominal type with its complete container shape.
            return Some(ConstructionInputTarget {
                def_id,
                return_shape,
            });
        }

        // Follow only the success slot of the two standard construction containers.
        let is_option = cx.tcx.is_diagnostic_item(sym::Option, definition.did());
        let is_result = cx.tcx.is_diagnostic_item(sym::Result, definition.did());
        if !is_option && !is_result {
            return None;
        }

        // Preserve the first container while descending through later combinations.
        let next_root = match (depth, is_result) {
            (0, true) => ConstructionInputContainerRoot::Result,
            (0, false) => ConstructionInputContainerRoot::Other,
            _ => root,
        };
        Self::constructed_target_inner(cx, arguments.type_at(0), depth + 1, next_root)
    }

    /// Returns a local nominal type after peeling ordinary references.
    fn direct_adt(input: Ty<'_>) -> Option<LocalDefId> {
        let input = input.peel_refs();
        let ty::Adt(definition, _) = input.kind() else {
            return None;
        };
        definition.did().as_local()
    }

    /// Resolves the sole immutable string-slice parameter to its body binding.
    fn single_string_binding(body: &Body<'_>, inputs: &[Ty<'_>]) -> Option<HirId> {
        if inputs.len() != 1 || body.params.len() != 1 {
            return None;
        }
        let ty::Ref(_, inner, rustc_hir::Mutability::Not) = inputs[0].kind() else {
            return None;
        };
        if !inner.is_str() {
            return None;
        }
        let PatKind::Binding(_, binding, _, None) = body.params[0].pat.kind else {
            return None;
        };
        Some(binding)
    }

    /// Extracts whole-item syntax safety from a free or associated function node.
    fn item_source(cx: &LateContext<'_>, item: &Item<'_>) -> ConstructionInputSource {
        ConstructionInputSource {
            item_span: item.span,
            is_private: item.vis_span.is_empty(),
            has_attributes: !cx.tcx.hir_attrs(item.hir_id()).is_empty(),
        }
    }

    /// Extracts source safety from an associated function node.
    fn impl_item_source(
        cx: &LateContext<'_>,
        item: &rustc_hir::ImplItem<'_>,
    ) -> ConstructionInputSource {
        ConstructionInputSource {
            item_span: item.span,
            is_private: item.vis_span().is_none_or(Span::is_empty),
            has_attributes: !cx.tcx.hir_attrs(item.hir_id()).is_empty(),
        }
    }

    /// Extracts whole-item syntax safety from a free or associated function node.
    const fn fallback_source(fallback: Span) -> ConstructionInputSource {
        ConstructionInputSource {
            item_span: fallback,
            is_private: false,
            has_attributes: true,
        }
    }

    /// Extracts whole-item syntax safety from a free or associated function node.
    fn source_facts(
        cx: &LateContext<'_>,
        node: Node<'_>,
        fallback: Span,
    ) -> ConstructionInputSource {
        match node {
            Node::Item(item) => Self::item_source(cx, item),
            Node::ImplItem(item) => Self::impl_item_source(cx, item),
            _ => Self::fallback_source(fallback),
        }
    }

    /// Records a resolved authored reference to a local free function.
    pub(crate) fn record_expression(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Resolve one direct path expression to a local function definition.
        let ExprKind::Path(path) = expression.kind else {
            return;
        };
        let Res::Def(DefKind::Fn, definition) = cx.qpath_res(&path, expression.hir_id) else {
            return;
        };
        let Some(definition) = definition.as_local() else {
            return;
        };

        // Retain the authored reference for possible same-file rewriting.
        self.function_uses
            .entry(definition)
            .or_default()
            .push(expression.span);
    }

    /// Discovers one function whose body constructs the local type in its return contract.
    pub(crate) fn record_function<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        def_id: LocalDefId,
    ) {
        // Restrict discovery to ordinary authored Rust functions and methods.
        let (ident, header) = match kind {
            FnKind::ItemFn(ident, _, header) => (ident, header),
            FnKind::Method(ident, signature) => (ident, signature.header),
            FnKind::Closure => return,
        };
        if header.abi != ExternAbi::Rust || span.in_external_macro(cx.sess().source_map()) {
            return;
        }

        // Resolve one local target through supported return containers.
        let Some(function) = Self::function_context(cx, def_id) else {
            return;
        };
        let signature = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();
        let Some(target) = Self::constructed_target(cx, signature.output()) else {
            return;
        };
        let is_target_same_module = cx.tcx.opt_local_parent(target.def_id) == Some(function.module);

        // Require a target-producing expression and retain parser source evidence.
        let inputs = signature.inputs();
        let is_first_input_target = inputs
            .first()
            .is_some_and(|input| Self::direct_adt(*input) == Some(target.def_id));

        // Prove that the body both constructs its target and consumes parser input.
        let string_binding = Self::single_string_binding(body, inputs);
        let mut evidence = ConstructionEvidence::new(cx, target.def_id, string_binding);
        evidence.visit_expr(body.value);
        if !evidence.has_constructed_target {
            return;
        }

        // Resolve source migration facts and `FromStr` lifetime compatibility.
        let node = cx.tcx.hir_node_by_def_id(def_id);
        let source = Self::source_facts(cx, node, span);

        // Reject borrowed target families that cannot satisfy `FromStr`.
        let has_target_lifetime = cx
            .tcx
            .generics_of(target.def_id)
            .own_params
            .iter()
            .any(|parameter| matches!(parameter.kind, ty::GenericParamDefKind::Lifetime));

        // Preserve the complete semantic candidate for crate-wide policy selection.
        let function_facts = ConstructionCandidateFunction {
            def_id,
            name: ident.name,
            name_span: ident.span,
            item_span: source.item_span,
            module: function.module,
        };

        // Retain the constructed type and container shape as one target contract.
        let target_facts = ConstructionCandidateTarget {
            def_id: target.def_id,
            name: cx.tcx.item_name(target.def_id.to_def_id()),
            return_shape: target.return_shape,
        };

        // Preserve policy selection facts independently from parser evidence.
        let ownership = ConstructionCandidateOwnership {
            origin: function.origin,
            is_first_input_target,
            is_target_same_module,
        };

        // Preserve string-input evidence used only by the parser rule.
        let parser = ConstructionParserFacts {
            has_single_str_input: string_binding.is_some(),
            has_used_string_input: evidence.has_used_source,
            has_target_lifetime,
        };

        // Preserve conservative source facts used only by automatic migration.
        let migration = ConstructionMigrationFacts {
            is_private: source.is_private,
            has_attributes: source.has_attributes,
        };

        // Group policy-specific evidence around the resolved function and target.
        self.candidates.push(ConstructionCandidate {
            function: function_facts,
            target: target_facts,
            ownership,
            parser,
            migration,
        });
    }

    /// Returns the construction `candidate` most recently recorded for one function.
    pub(crate) fn candidate(&self, def_id: LocalDefId) -> Option<&ConstructionCandidate> {
        self.candidates
            .iter()
            .rev()
            .find(|candidate| candidate.function.def_id == def_id)
    }

    /// Groups every structurally valid textual parser by its constructed target.
    pub(crate) fn parser_families(&self) -> HashMap<LocalDefId, Vec<&ConstructionCandidate>> {
        let mut families = HashMap::<LocalDefId, Vec<&ConstructionCandidate>>::new();
        let parsers = self
            .candidates
            .iter()
            .filter(|candidate| candidate.is_text_parser());
        for candidate in parsers {
            families
                .entry(candidate.target.def_id)
                .or_default()
                .push(candidate);
        }
        families
    }

    /// Records one item in direct module traversal order.
    fn record_module_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Resolve only items directly owned by a module.
        let def_id = item.owner_id.def_id;
        let Some(module) = cx.tcx.opt_local_parent(def_id) else {
            return;
        };
        if cx.tcx.def_kind(module) != DefKind::Mod {
            return;
        }

        // Retain physical traversal order for later adjacency checks.
        self.module_items
            .entry(module)
            .or_default()
            .push(ConstructionAnalysisModuleItem {
                def_id,
                span: item.span,
            });
    }

    /// Records function definitions reached through one use declaration.
    fn record_import(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        let ItemKind::Use(path, _) = item.kind else {
            return;
        };
        let resolutions = [path.res.type_ns, path.res.value_ns, path.res.macro_ns];
        for resolution in resolutions.into_iter().flatten() {
            let Some(def_id) = resolution.opt_def_id().and_then(DefId::as_local) else {
                continue;
            };
            if cx.tcx.def_kind(def_id) != DefKind::Fn {
                continue;
            }
            self.imported_functions.insert(def_id);
        }
    }

    /// Records the local nominal target of an authored core `FromStr` implementation.
    fn record_from_str_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Resolve only authored trait implementation items.
        let def_id = item.owner_id.def_id;
        if !matches!(item.kind, ItemKind::Impl(_)) {
            return;
        }

        // Resolve the implemented trait before checking its standard identity.
        let Some(trait_ref) = cx
            .tcx
            .impl_opt_trait_ref(def_id)
            .map(rustc_middle::ty::EarlyBinder::instantiate_identity)
        else {
            return;
        };

        // Identify the standard trait without mistaking a same-named user trait for it.
        let is_core_from_str = cx.tcx.crate_name(trait_ref.def_id.krate).as_str() == "core"
            && cx.tcx.item_name(trait_ref.def_id).as_str() == "FromStr";
        if !is_core_from_str {
            return;
        }

        // Retain the local nominal self type implementing the standard contract.
        let self_type = cx.tcx.type_of(def_id).instantiate_identity();
        let ty::Adt(definition, _) = self_type.kind() else {
            return;
        };
        let Some(target) = definition.did().as_local() else {
            return;
        };
        self.from_str_targets.insert(target);
    }

    /// Records module order, imports, and existing `FromStr` implementations.
    pub(crate) fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.record_module_item(cx, item);
        self.record_import(cx, item);
        self.record_from_str_impl(cx, item);
    }
}

// -----------------------------------------------------------------------------
// ConstructionEvidence: Body level semantic proof
// -----------------------------------------------------------------------------

/// Finds target-producing expressions and use of an optional source binding.
struct ConstructionEvidence<'analysis, 'tcx> {
    /// Compiler context whose current typeck results cover the visited body.
    cx: &'analysis LateContext<'tcx>,
    /// Nominal type promised by the function return contract.
    target: LocalDefId,
    /// Optional parser source binding.
    source: Option<HirId>,
    /// Whether a target-producing expression occurs in the body.
    has_constructed_target: bool,
    /// Whether the parser source occurs in the body.
    has_used_source: bool,
}

impl<'analysis, 'tcx> ConstructionEvidence<'analysis, 'tcx> {
    /// Starts evidence collection for one function body.
    const fn new(
        cx: &'analysis LateContext<'tcx>,
        target: LocalDefId,
        source: Option<HirId>,
    ) -> Self {
        Self {
            cx,
            target,
            source,
            has_constructed_target: false,
            has_used_source: false,
        }
    }

    /// Returns whether an expression is an actual target-producing operation.
    fn produces_target(&self, expression: &Expr<'_>) -> bool {
        // Recognize calls, struct expressions, and fieldless variant constructors.
        let is_operation = matches!(
            expression.kind,
            ExprKind::Call(..) | ExprKind::MethodCall(..) | ExprKind::Struct(..)
        );

        // Resolve the source shape used by fieldless enum and tuple constructors.
        let is_unit_constructor = matches!(
            expression.kind,
            ExprKind::Path(path)
                if matches!(self.cx.qpath_res(&path, expression.hir_id), Res::Def(DefKind::Ctor(..), _))
        );

        // Require the operation's resolved result to be the promised local target.
        let expression_type = self.cx.typeck_results().expr_ty(expression);
        let produced_target = ConstructionAnalysis::direct_adt(expression_type);
        (is_operation || is_unit_constructor) && produced_target == Some(self.target)
    }
}

impl<'tcx> Visitor<'tcx> for ConstructionEvidence<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Path(path) = expression.kind
            && let Res::Local(binding) = self.cx.qpath_res(&path, expression.hir_id)
            && self.source == Some(binding)
        {
            self.has_used_source = true;
        }
        self.has_constructed_target |= self.produces_target(expression);
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, body: rustc_hir::BodyId) {
        self.visit_body(self.cx.tcx.hir_body(body));
    }
}
