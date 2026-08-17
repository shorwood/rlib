extern crate rustc_abi;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_abi::ExternAbi;
use rustc_hir::def::{CtorOf, DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{
    Body, Expr, ExprKind, HirId, Item, ItemKind, MatchSource, Node, PatKind, Stmt, StmtKind,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::symbol::sym;
use rustc_span::{Span, Symbol};

use super::identifier_case;

// -----------------------------------------------------------------------------
// Construction: Candidate semantic record
// -----------------------------------------------------------------------------
/// Container path from a function return type to the local value it constructs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConstructionReturn {
    /// The function returns the constructed type directly.
    Direct,
    /// The function returns the constructed type through at least one standard container.
    Contained,
    /// The function returns exactly `Result<T, E>` for the constructed `T`.
    FallibleDirect,
}
/// Whether a constructor-like function is free or already associated with a type.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConstructionOrigin {
    /// A module-level free function.
    Free,
    /// A receiver-free function in an inherent implementation.
    Inherent,
}
/// Structural facts needed to recognize a canonical owned string parser.
#[derive(Clone, Copy)]
pub struct ConstructionParserFacts {
    /// Whether the function takes exactly one immutable string slice.
    has_single_str_input: bool,
    /// Whether that string binding is referenced by the body.
    has_used_string_input: bool,
    /// Whether the target declares a lifetime that `FromStr` cannot return.
    has_target_lifetime: bool,
}
/// Authored source properties used by conservative constructor relocation.
#[derive(Clone, Copy)]
pub struct ConstructionMigrationFacts {
    /// Whether source visibility is private.
    pub(crate) is_private: bool,
    /// Whether attributes decorate the function declaration.
    pub(crate) has_attributes: bool,
}

// -----------------------------------------------------------------------------
// ConstructionCandidate: Function and target identity
// -----------------------------------------------------------------------------
/// Authored function identity retained by one construction `candidate`.
#[derive(Clone)]
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
/// Constructed local type and the return contract that reaches it.
#[derive(Clone)]
pub struct ConstructionCandidateTarget {
    /// Local nominal type constructed by the body.
    pub(crate) def_id: LocalDefId,
    /// Display name of the constructed type.
    pub(crate) name: Symbol,
    /// Standard-container shape around the constructed value.
    pub(super) return_shape: ConstructionReturn,
}
/// Function ownership facts used to select the responsible lint policy.
#[derive(Clone, Copy)]
pub struct ConstructionCandidateOwnership {
    /// Whether the function is module-level or already associated.
    pub(crate) origin: ConstructionOrigin,
    /// Whether the first parameter already supplies the constructed type.
    pub(crate) is_first_input_target: bool,
    /// Whether the function and constructed type are defined in the same module.
    pub(crate) is_target_same_module: bool,
}
/// One authored function proven to construct a local nominal type.
#[derive(Clone)]
pub struct ConstructionCandidate {
    /// Function identity and source ownership.
    pub(crate) function: ConstructionCandidateFunction,
    /// Local type and return-container contract.
    pub(crate) target: ConstructionCandidateTarget,
    /// Ownership and receiver-overlap facts.
    pub(crate) ownership: ConstructionCandidateOwnership,
    /// Text-parser-specific structural evidence.
    parser: ConstructionParserFacts,
    /// Whether the body can propagate or explicitly return a standard `Result` failure.
    #[cfg_attr(
        not(any(feature = "bon", feature = "derive_more", feature = "serde")),
        allow(dead_code)
    )]
    has_failure_path: bool,
    /// Source facts governing automatic relocation.
    pub(crate) migration: ConstructionMigrationFacts,
}

impl ConstructionCandidate {
    /// Returns whether this constructor exposes an exact `Result<T, E>` contract.
    #[cfg(any(feature = "bon", feature = "derive_more", feature = "serde"))]
    pub(crate) fn is_fallible_direct(&self) -> bool {
        self.target.return_shape == ConstructionReturn::FallibleDirect && self.has_failure_path
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
/// Outermost container used to distinguish exact `Result<T, E>` returns.
#[derive(Clone, Copy)]
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
/// One direct module item retained for guarded adjacency analysis.
#[derive(Clone, Copy)]
pub struct ConstructionAnalysisModuleItem {
    /// Definition identity of the item.
    pub(crate) def_id: LocalDefId,
    /// Authored source range of the item.
    pub(crate) span: Span,
}
/// Crate-wide construction discovery shared by ownership and parser lints.
#[derive(Default)]
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

        // Only implementation items can own inherent associated functions.
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

        // A module parent identifies a free function constructor.
        if cx.tcx.def_kind(parent) == DefKind::Mod {
            return Some(ConstructionInputFunction {
                origin: ConstructionOrigin::Free,
                module: parent,
            });
        }

        // A non-trait implementation parent identifies an inherent constructor.
        if cx.tcx.def_kind(parent) == (DefKind::Impl { of_trait: false }) {
            return Self::inherent_function(cx, parent);
        }
        None
    }

    /// Recursively follows `Option` and `Result` success values to one local nominal type.
    fn constructed_target(cx: &LateContext<'_>, output: Ty<'_>) -> Option<ConstructionInputTarget> {
        Self::constructed_target_inner(cx, output, 0, ConstructionInputContainerRoot::Direct)
    }

    /// Preserves container depth and outer shape while unwrapping nested success values.
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

        // Other wrapper types do not preserve a recognized construction success path.
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

        // Only nominal input types can be compared with the construction target.
        let ty::Adt(definition, _) = input.kind() else {
            return None;
        };
        definition.did().as_local()
    }

    /// Resolves the sole immutable string-slice parameter to its body binding.
    fn single_string_binding(body: &Body<'_>, inputs: &[Ty<'_>]) -> Option<HirId> {
        // Canonical parsers accept exactly one source parameter.
        if inputs.len() != 1 || body.params.len() != 1 {
            return None;
        }

        // The sole parameter must be an immutable string slice.
        let ty::Ref(_, inner, rustc_hir::Mutability::Not) = inputs[0].kind() else {
            return None;
        };

        // Other referenced types cannot supply the parser's textual source.
        if !inner.is_str() {
            return None;
        }

        // The parameter must remain a direct binding for use tracking.
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

        // Only function definitions are tracked as relocatable references.
        let Res::Def(DefKind::Fn, definition) = cx.qpath_res(&path, expression.hir_id) else {
            return;
        };

        // External definitions cannot be rewritten within this crate.
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

            // Closures do not establish a named construction API.
            FnKind::Closure => return,
        };

        // Foreign ABI functions and external macro output are not authored candidates.
        if header.abi != ExternAbi::Rust || span.in_external_macro(cx.sess().source_map()) {
            return;
        }

        // Resolve one local target through supported return containers.
        let Some(function) = Self::function_context(cx, def_id) else {
            return;
        };
        let signature = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();

        // The return type must resolve to a supported local construction target.
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
        let returned_targets = ConstructionResultCollector::collect(cx, target.def_id, body.value);
        let mut evidence =
            ConstructionEvidence::new(cx, target.def_id, string_binding, returned_targets);
        evidence.visit_expr(body.value);

        // The function body must actually construct the target promised by its return type.
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
        let has_failure_path = ConstructionFailureCollector::collect(cx, body.value);

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
            has_failure_path,
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

        // Items without a local parent cannot participate in module adjacency analysis.
        let Some(module) = cx.tcx.opt_local_parent(def_id) else {
            return;
        };

        // Nested items are not direct members of the module's authored order.
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
        // Only use declarations can create imported constructor references.
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

        // Only implementation items can establish a `FromStr` contract.
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

        // Same-named user traits do not preclude a standard `FromStr` implementation.
        if !is_core_from_str {
            return;
        }

        // Retain the local nominal self type implementing the standard contract.
        let self_type = cx.tcx.type_of(def_id).instantiate_identity();

        // Only nominal self types can be construction-parser targets.
        let ty::Adt(definition, _) = self_type.kind() else {
            return;
        };

        // External target types are not part of this crate's parser families.
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
// ConstructionFailureCollector: Actual fallible-boundary evidence
// -----------------------------------------------------------------------------

/// Finds an explicit standard `Err` result or authored `?` propagation in a constructor body.
struct ConstructionFailureCollector<'analysis, 'tcx> {
    /// Compiler context used to resolve standard result variants and operations.
    cx: &'analysis LateContext<'tcx>,
    /// Whether the constructor contains an explicit failure path.
    has_failure_path: bool,
}

impl<'analysis, 'tcx> ConstructionFailureCollector<'analysis, 'tcx> {
    /// Returns whether one expression directly constructs the standard `Result::Err` variant.
    fn is_result_variant(&self, expression: &Expr<'_>, expected: &str) -> bool {
        // Result variant recognition starts with a constructor call.
        let ExprKind::Call(callee, _) = expression.kind else {
            return false;
        };

        // The constructor must be named by a direct path.
        let ExprKind::Path(path) = callee.kind else {
            return false;
        };

        // Only resolved variant constructors can be standard result variants.
        let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
            self.cx.qpath_res(&path, callee.hir_id)
        else {
            return false;
        };
        let variant = self.cx.tcx.parent(constructor);
        let owner = self.cx.tcx.parent(variant);
        self.cx.tcx.item_name(variant).as_str() == expected
            && self.cx.tcx.is_diagnostic_item(sym::Result, owner)
    }

    /// Returns whether a returned call has a standard `Result` contract beyond direct `Ok`.
    fn is_result_operation(&self, expression: &Expr<'_>) -> bool {
        // Only non-`Ok` calls can supply a fallible result operation.
        if !matches!(
            expression.kind,
            ExprKind::Call(..) | ExprKind::MethodCall(..)
        ) || self.is_result_variant(expression, "Ok")
        {
            return false;
        }
        matches!(self.cx.typeck_results().expr_ty(expression).kind(), ty::Adt(definition, _) if self.cx.tcx.is_diagnostic_item(sym::Result, definition.did()))
    }

    /// Visits only expressions whose value can become the function's returned result.
    fn visit_result_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // One explicit failure path is sufficient evidence for a fallible constructor.
        if self.is_result_variant(expression, "Err") || self.is_result_operation(expression) {
            self.has_failure_path = true;
            return;
        }
        match expression.kind {
            ExprKind::Block(block, _) => {
                if let Some(tail) = block.expr {
                    self.visit_result_expr(tail);
                }
            }
            ExprKind::If(_, then_expression, else_expression) => {
                self.visit_result_expr(then_expression);
                if let Some(else_expression) = else_expression {
                    self.visit_result_expr(else_expression);
                }
            }
            ExprKind::Match(_, arms, _) => {
                for arm in arms {
                    self.visit_result_expr(arm.body);
                }
            }
            ExprKind::Ret(Some(value)) | ExprKind::DropTemps(value) => {
                self.visit_result_expr(value);
            }
            _ => {}
        }
    }

    /// Returns whether a constructor body contains concrete failure evidence.
    fn collect(cx: &'analysis LateContext<'tcx>, expression: &'tcx Expr<'tcx>) -> bool {
        let mut collector = Self {
            cx,
            has_failure_path: false,
        };
        collector.visit_result_expr(expression);
        collector.visit_expr(expression);
        collector.has_failure_path
    }
}

impl<'tcx> Visitor<'tcx> for ConstructionFailureCollector<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if matches!(
            expression.kind,
            ExprKind::Match(_, _, MatchSource::TryDesugar(_))
        ) {
            self.has_failure_path = true;
        }
        if let ExprKind::Ret(Some(value)) = expression.kind {
            self.visit_result_expr(value);
        }

        // Closure-local control flow cannot make the enclosing constructor fallible.
        if matches!(expression.kind, ExprKind::Closure(_)) {
            return;
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
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
    /// Target constructions contributing to the function result.
    returned_targets: HashSet<HirId>,
    /// Whether a target-producing expression occurs in the body.
    has_constructed_target: bool,
    /// Whether the parser source occurs in the body.
    has_used_source: bool,
}

impl<'analysis, 'tcx> ConstructionEvidence<'analysis, 'tcx> {
    /// Starts evidence collection for one function body.
    fn new(
        cx: &'analysis LateContext<'tcx>,
        target: LocalDefId,
        source: Option<HirId>,
        returned_targets: HashSet<HirId>,
    ) -> Self {
        let has_constructed_target = !returned_targets.is_empty();
        Self {
            cx,
            target,
            source,
            returned_targets,
            has_constructed_target,
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

    /// Returns whether an expression is exactly the parser source binding.
    fn is_source_path(&self, expression: &Expr<'_>) -> bool {
        matches!(
            expression.kind,
            ExprKind::Path(path)
                if matches!(
                    self.cx.qpath_res(&path, expression.hir_id),
                    Res::Local(binding) if self.source == Some(binding)
                )
        )
    }
}

impl<'tcx> Visitor<'tcx> for ConstructionEvidence<'_, 'tcx> {
    fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
        // Discarded parser-source bindings do not demonstrate meaningful source use.
        if let StmtKind::Let(local) = statement.kind
            && matches!(local.pat.kind, PatKind::Wild)
            && local
                .init
                .is_some_and(|initializer| self.is_source_path(initializer))
        {
            return;
        }
        intravisit::walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Path(path) = expression.kind
            && let Res::Local(binding) = self.cx.qpath_res(&path, expression.hir_id)
            && self.source == Some(binding)
        {
            self.has_used_source = true;
        }
        self.has_constructed_target |=
            self.returned_targets.contains(&expression.hir_id) && self.produces_target(expression);

        // Nested closures do not contribute construction evidence to the outer function.
        if matches!(expression.kind, ExprKind::Closure(_)) {
            return;
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

// -----------------------------------------------------------------------------
// ConstructionResultCollector: Returned target ownership
// -----------------------------------------------------------------------------

/// Finds target constructions contributing to tail values and explicit returns.
struct ConstructionResultCollector<'analysis, 'tcx> {
    /// Compiler context used to resolve construction expressions.
    cx: &'analysis LateContext<'tcx>,
    /// Type whose returned constructions are collected.
    target: LocalDefId,
    /// Expressions known to contribute the target type to the function result.
    returned_targets: HashSet<HirId>,
    /// Local bindings mapped to the target-producing expressions they retain.
    bindings: HashMap<HirId, HashSet<HirId>>,
}

impl<'analysis, 'tcx> ConstructionResultCollector<'analysis, 'tcx> {
    /// Returns whether an expression directly constructs the target type.
    fn produces_target(&self, expression: &Expr<'_>) -> bool {
        let operation = matches!(
            expression.kind,
            ExprKind::Call(..) | ExprKind::MethodCall(..) | ExprKind::Struct(..)
        ) || matches!(
            expression.kind,
            ExprKind::Path(path)
                if matches!(self.cx.qpath_res(&path, expression.hir_id), Res::Def(DefKind::Ctor(..), _))
        );
        operation
            && ConstructionAnalysis::direct_adt(self.cx.typeck_results().expr_ty(expression))
                == Some(self.target)
    }

    /// Collects target-producing expressions nested beneath one expression.
    fn targets_in(&self, expression: &'tcx Expr<'tcx>) -> HashSet<HirId> {
        struct TargetFinder<'collector, 'analysis, 'tcx> {
            collector: &'collector ConstructionResultCollector<'analysis, 'tcx>,
            targets: HashSet<HirId>,
        }

        impl<'tcx> Visitor<'tcx> for TargetFinder<'_, '_, 'tcx> {
            fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
                // A direct construction is a complete target-producing leaf.
                if self.collector.produces_target(expression) {
                    self.targets.insert(expression.hir_id);
                    return;
                }

                // A bound target value contributes all of its recorded construction sites.
                if let ExprKind::Path(path) = expression.kind
                    && let Res::Local(binding) =
                        self.collector.cx.qpath_res(&path, expression.hir_id)
                    && let Some(targets) = self.collector.bindings.get(&binding)
                {
                    self.targets.extend(targets);
                    return;
                }

                // Nested closures cannot contribute targets to the outer return value.
                if matches!(expression.kind, ExprKind::Closure(_)) {
                    return;
                }
                intravisit::walk_expr(self, expression);
            }

            fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
        }

        let mut finder = TargetFinder {
            collector: self,
            targets: HashSet::new(),
        };
        finder.visit_expr(expression);
        finder.targets
    }

    /// Follows expressions that can contribute to the callable's returned value.
    fn visit_result_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // A direct construction is a complete target-producing result.
        if self.produces_target(expression) {
            self.returned_targets.insert(expression.hir_id);
            return;
        }

        // A bound target value contributes all of its recorded construction sites.
        if let ExprKind::Path(path) = expression.kind
            && let Res::Local(binding) = self.cx.qpath_res(&path, expression.hir_id)
            && let Some(targets) = self.bindings.get(&binding)
        {
            self.returned_targets.extend(targets);
            return;
        }
        match expression.kind {
            ExprKind::Block(block, _) => {
                for statement in block.stmts {
                    self.visit_stmt(statement);
                }
                if let Some(tail) = block.expr {
                    self.visit_result_expr(tail);
                }
            }
            ExprKind::If(condition, then_expression, else_expression) => {
                self.visit_expr(condition);
                self.visit_result_expr(then_expression);
                if let Some(else_expression) = else_expression {
                    self.visit_result_expr(else_expression);
                }
            }
            ExprKind::Match(scrutinee, arms, _) => {
                self.visit_expr(scrutinee);
                for arm in arms {
                    if let Some(guard) = arm.guard {
                        self.visit_expr(guard);
                    }
                    self.visit_result_expr(arm.body);
                }
            }
            ExprKind::Call(callee, arguments) => {
                self.visit_expr(callee);
                for argument in arguments {
                    self.visit_result_expr(argument);
                }
            }
            ExprKind::MethodCall(_, receiver, arguments, _) => {
                self.visit_result_expr(receiver);
                for argument in arguments {
                    self.visit_result_expr(argument);
                }
            }
            ExprKind::Closure(closure) => {
                let body = self.cx.tcx.hir_body(closure.body);
                self.visit_result_expr(body.value);
            }
            ExprKind::Ret(Some(value)) | ExprKind::DropTemps(value) => {
                self.visit_result_expr(value);
            }
            _ => self.visit_expr(expression),
        }
    }

    /// Collects every target construction contributing to a callable result.
    fn collect(
        cx: &'analysis LateContext<'tcx>,
        target: LocalDefId,
        expression: &'tcx Expr<'tcx>,
    ) -> HashSet<HirId> {
        let mut collector = Self {
            cx,
            target,
            returned_targets: HashSet::new(),
            bindings: HashMap::new(),
        };
        collector.visit_result_expr(expression);
        collector.returned_targets
    }
}

impl<'tcx> Visitor<'tcx> for ConstructionResultCollector<'_, 'tcx> {
    fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
        if let StmtKind::Let(local) = statement.kind
            && let Some(initializer) = local.init
            && let PatKind::Binding(_, binding, _, None) = local.pat.kind
        {
            let targets = self.targets_in(initializer);
            if !targets.is_empty() {
                self.bindings.insert(binding, targets);
            }
        }
        intravisit::walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Assignment handling records the binding update before ending this traversal branch.
        if let ExprKind::Assign(left, right, _) = expression.kind {
            let targets = self.targets_in(right);
            self.visit_expr(right);
            if let ExprKind::Path(path) = left.kind
                && let Res::Local(binding) = self.cx.qpath_res(&path, left.hir_id)
            {
                if targets.is_empty() {
                    self.bindings.remove(&binding);
                } else {
                    self.bindings.insert(binding, targets);
                }
            }
            self.visit_expr(left);
            return;
        }
        match expression.kind {
            ExprKind::Ret(Some(value)) => self.visit_result_expr(value),
            ExprKind::Closure(_) => {}
            _ => intravisit::walk_expr(self, expression),
        }
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}
