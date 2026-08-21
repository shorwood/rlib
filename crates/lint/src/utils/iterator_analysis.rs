extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{Body, Expr, ExprKind, HirId, Item, ItemKind, Mutability, PatKind, Stmt, StmtKind};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::symbol::sym;
use rustc_span::{Span, Symbol};

use super::identifier_case;

// -----------------------------------------------------------------------------
// IteratorCandidate: Complete diagnostic context
// -----------------------------------------------------------------------------
/// One unique stateful traversal that should use Rust's iterator protocol.
#[derive(Clone)]
pub struct IteratorCandidateSource {
    /// HIR node used for lint-level configuration.
    pub hir_id: HirId,
    /// Authored method name.
    pub name: Symbol,
    /// Method identifier source range.
    pub name_span: Span,
    /// Mutable receiver establishing persistent traversal state.
    pub receiver_span: Span,
    /// State advance or delegated `next` operation.
    pub evidence_span: Span,
}

/// Inferred iterator contract shown in diagnostics.
#[derive(Clone)]
pub struct IteratorCandidateProtocol {
    /// Receiver type that should own `Iterator`.
    pub type_name: String,
    /// Concrete yielded item spelling.
    pub item_name: String,
}

/// One unique stateful traversal that should use Rust's iterator protocol.
#[derive(Clone)]
pub struct IteratorCandidate {
    /// Authored method identity and structural evidence.
    pub source: IteratorCandidateSource,
    /// Inferred standard iterator contract.
    pub protocol: IteratorCandidateProtocol,
    /// Receiver definition used for family selection.
    type_def_id: LocalDefId,
}

// -----------------------------------------------------------------------------
// IteratorAnalysis: Stateful traversal family selection
// -----------------------------------------------------------------------------
/// Finds unique cursor-like methods that reproduce `Iterator::next`.
#[derive(Default)]
pub struct IteratorAnalysis {
    /// Proven cursor-like methods in traversal order.
    candidates: Vec<IteratorCandidate>,
    /// Local types already implementing `Iterator`.
    occupied: HashSet<LocalDefId>,
    /// Types whose reusable traversal should not consume receiver state.
    reusable_collections: HashSet<LocalDefId>,
}

impl IteratorAnalysis {
    /// Records existing iterator contracts and reusable collection traversal APIs.
    pub fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Resolve the local receiver before classifying its traversal ownership.
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };
        let self_ty = cx.tcx.type_of(item.owner_id.def_id).instantiate_identity();

        // Implementations for foreign or nonnominal types cannot own a local iterator contract.
        let Some(type_def_id) = iterator_classification_local_adt(self_ty) else {
            return;
        };

        // Trait implementations directly establish occupied traversal protocols.
        if let Some(trait_ref) = cx.tcx.impl_opt_trait_ref(item.owner_id.def_id) {
            let path = cx.tcx.def_path_str(trait_ref.instantiate_identity().def_id);
            if path.ends_with("::Iterator") {
                self.occupied.insert(type_def_id);
            }
            if path.ends_with("::IntoIterator") {
                self.reusable_collections.insert(type_def_id);
            }
            return;
        }

        // Inherent iteration methods identify reusable collections, not cursor owners.
        let has_reusable_method = implementation.items.iter().any(|reference| {
            ["iter", "iter_mut"].contains(
                &cx.tcx
                    .associated_item(reference.owner_id.to_def_id())
                    .name()
                    .as_str(),
            )
        });

        // Types without reusable traversal methods provide no collection evidence.
        if !has_reusable_method {
            return;
        }
        self.reusable_collections.insert(type_def_id);
    }

    /// Records one authored inherent method with cursor and exhaustion evidence.
    pub fn record_function<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        body: &'tcx Body<'tcx>,
        def_id: LocalDefId,
    ) {
        // Reject trait methods and unsupported method contracts before type analysis.
        if is_iterator_classification_trait_method(cx, def_id) {
            return;
        }

        // Restrict candidates to safe, runtime inherent methods with no extra parameters.
        let FnKind::Method(ident, signature) = kind else {
            return;
        };

        // Resolve header exclusions independently from the method-shape check.
        let is_const = signature.header.constness == rustc_hir::Constness::Const;
        let is_unsafe = matches!(
            signature.header.safety,
            rustc_hir::HeaderSafety::Normal(rustc_hir::Safety::Unsafe)
        );

        // Unsafe, const, or multi-parameter methods cannot be ordinary iterator-next candidates.
        if is_const || is_unsafe || body.params.len() != 1 {
            return;
        }

        // Resolve the mutable local receiver before inspecting its yielded item.
        let function = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();

        // Methods with other semantic arities cannot advance only their receiver.
        let [receiver] = function.inputs() else {
            return;
        };

        // Iterator advancement requires a mutable receiver reference.
        let ty::Ref(_, receiver, Mutability::Mut) = receiver.kind() else {
            return;
        };

        // Foreign or nonnominal receivers cannot own a local iterator implementation.
        let Some(type_def_id) = iterator_classification_local_adt(*receiver) else {
            return;
        };

        // Require an owned item and a name without contextual traversal policy.
        let Some(item) = iterator_classification_option_item(cx, function.output()) else {
            return;
        };

        // Borrowed yields or policy-bearing names do not claim ordinary owned iteration.
        if matches!(item.kind(), ty::Ref(..) | ty::RawPtr(..))
            || !has_iterator_classification_neutral_name(cx, ident.name, type_def_id, item)
        {
            return;
        }

        // Prove that the body advances persistent receiver state.
        let PatKind::Binding(_, receiver_binding, _, None) = body.params[0].pat.kind else {
            return;
        };

        // Methods without persistent state-advance evidence do not reproduce Iterator::next.
        let Some(evidence_span) = IteratorEvidence::analyze(cx, body, receiver_binding) else {
            return;
        };

        // Preserve authored method identity and exact state-advance evidence.
        let source = IteratorCandidateSource {
            hir_id: cx.tcx.local_def_id_to_hir_id(def_id),
            name: ident.name,
            name_span: ident.span,
            receiver_span: body.params[0].span,
            evidence_span,
        };

        // Retain the resolved receiver and yielded item as one protocol contract.
        let protocol = IteratorCandidateProtocol {
            type_name: cx.tcx.def_path_str(type_def_id.to_def_id()),
            item_name: item.to_string(),
        };

        // Store the compact source and protocol contexts for family selection.
        self.candidates.push(IteratorCandidate {
            source,
            protocol,
            type_def_id,
        });
    }

    /// Returns unique stateful traversals without an existing iterator owner.
    pub fn findings(&self) -> Vec<&IteratorCandidate> {
        // Group all traversal candidates by the receiver that would own `Iterator`.
        let mut families = HashMap::<LocalDefId, Vec<&IteratorCandidate>>::new();
        for candidate in &self.candidates {
            families
                .entry(candidate.type_def_id)
                .or_default()
                .push(candidate);
        }

        // Retain only unique traversals without existing or reusable iteration ownership.
        let mut findings = Vec::new();
        for (type_def_id, family) in families {
            if family.len() != 1
                || self.occupied.contains(&type_def_id)
                || self.reusable_collections.contains(&type_def_id)
            {
                continue;
            }
            findings.push(family[0]);
        }
        findings.sort_unstable_by_key(|candidate| candidate.source.name_span.lo());
        findings
    }
}

// -----------------------------------------------------------------------------
// IteratorVocabulary: Traversal policy words
// -----------------------------------------------------------------------------

/// Vocabulary that adds no traversal policy.
const ITERATOR_VOCABULARY_NEUTRAL_NAMES: &[&str] = &["Advance", "Item", "Next", "Take"];

/// Vocabulary whose absence represents context rather than exhaustion.
const ITERATOR_VOCABULARY_CONTEXTUAL_NAMES: &[&str] = &[
    "Available",
    "Highest",
    "Matching",
    "Optional",
    "Parse",
    "Priority",
    "Ready",
    "Receive",
    "Timeout",
    "Try",
    "Until",
    "With",
];

/// Queue and channel operations that represent temporary availability.
const ITERATOR_VOCABULARY_TRANSIENT_METHODS: &[&str] = &[
    "pop",
    "pop_back",
    "pop_front",
    "recv",
    "remove",
    "swap_remove",
    "try_recv",
];

// -----------------------------------------------------------------------------
// IteratorClassification: Traversal discovery helpers
// -----------------------------------------------------------------------------

/// Resolves a possibly referenced type to one local nominal definition.
fn iterator_classification_local_adt(ty: Ty<'_>) -> Option<LocalDefId> {
    // Only nominal types can resolve to a local iterator owner.
    let ty::Adt(definition, _) = ty.peel_refs().kind() else {
        return None;
    };
    definition.did().as_local()
}

/// Returns whether an associated function belongs to a trait implementation.
fn is_iterator_classification_trait_method(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
    cx.tcx
        .opt_local_parent(def_id)
        .is_some_and(|parent| matches!(cx.tcx.def_kind(parent), DefKind::Impl { of_trait: true }))
}

/// Extracts the item from an exact standard `Option<Item>` return.
fn iterator_classification_option_item<'tcx>(
    cx: &LateContext<'tcx>,
    ty: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    // Non-ADT returns cannot be the standard Option item contract.
    let ty::Adt(definition, arguments) = ty.kind() else {
        return None;
    };
    cx.tcx
        .is_diagnostic_item(sym::Option, definition.did())
        .then(|| arguments.type_at(0))
}

/// Returns whether a method name claims unqualified sequence advancement.
fn has_iterator_classification_neutral_name(
    cx: &LateContext<'_>,
    name: Symbol,
    type_def_id: LocalDefId,
    item: Ty<'_>,
) -> bool {
    // Reject names carrying availability, parsing, or selection policy.
    let words = identifier_case::words(name.as_str());

    // Contextual vocabulary distinguishes temporary absence from iterator exhaustion.
    if words
        .iter()
        .any(|word| ITERATOR_VOCABULARY_CONTEXTUAL_NAMES.contains(&word.as_str()))
    {
        return false;
    }

    // Accept vocabulary already supplied by the receiver or yielded item type.
    let mut semantic = identifier_case::words(cx.tcx.item_name(type_def_id.to_def_id()).as_str());
    let item_name = item.to_string();
    let item_words = item_name
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .flat_map(identifier_case::words);
    semantic.extend(item_words);

    // Every remaining word must be protocol vocabulary or type-derived vocabulary.
    words.into_iter().all(|word| {
        ITERATOR_VOCABULARY_NEUTRAL_NAMES.contains(&word.as_str()) || semantic.contains(&word)
    })
}

// -----------------------------------------------------------------------------
// IteratorMutation: Persistent receiver change
// -----------------------------------------------------------------------------
/// One explicit receiver mutation and its top-level field, when identifiable.
struct IteratorMutation {
    /// Assignment expression used as diagnostic evidence.
    span: Span,
    /// Receiver field changed by the assignment.
    field: Option<Symbol>,
}

// -----------------------------------------------------------------------------
// IteratorEvidenceState: Accumulated traversal facts
// -----------------------------------------------------------------------------

/// Mutable facts accumulated while traversing one iterator-like body.
#[derive(Default)]
struct IteratorEvidenceState {
    /// Explicit assignments paired with their top-level receiver field.
    mutations: Vec<IteratorMutation>,
    /// Direct delegation to an inner iterator's `next`.
    delegated_next: Option<Span>,
    /// Whether the returned item reads receiver-owned state.
    has_returned_receiver_use: bool,
    /// Top-level receiver fields contributing to returned items.
    returned_receiver_fields: HashSet<Symbol>,
    /// Whether the body performs queue-like or temporary removal.
    has_transient_operation: bool,
}

/// Returns whether an expression is a projection rooted in the mutable receiver.
fn is_iterator_evidence_receiver_rooted(
    cx: &LateContext<'_>,
    receiver: HirId,
    expression: &Expr<'_>,
) -> bool {
    // A direct receiver path completes the receiver-rooted proof immediately.
    if let ExprKind::Path(path) = expression.kind {
        return matches!(cx.qpath_res(&path, expression.hir_id), Res::Local(binding) if binding == receiver);
    }
    let base = match expression.kind {
        ExprKind::Field(base, _) | ExprKind::Unary(_, base) | ExprKind::AddrOf(_, _, base) => {
            Some(base)
        }
        ExprKind::Index(base, _, _) => Some(base),
        _ => None,
    };
    base.is_some_and(|base| is_iterator_evidence_receiver_rooted(cx, receiver, base))
}

/// Returns whether an expression is the receiver path itself.
fn is_iterator_evidence_direct_receiver(
    cx: &LateContext<'_>,
    receiver: HirId,
    expression: &Expr<'_>,
) -> bool {
    matches!(
        expression.kind,
        ExprKind::Path(path)
            if matches!(cx.qpath_res(&path, expression.hir_id), Res::Local(binding) if binding == receiver)
    )
}

// -----------------------------------------------------------------------------
// Iterator: Persistent state advancement and returned-value evidence
// -----------------------------------------------------------------------------

/// Finds whether an expression reads the receiver or a receiver-derived local.
struct IteratorReceiverUse<'set, 'analysis, 'tcx> {
    /// Compiler context used to resolve local paths.
    cx: &'analysis LateContext<'tcx>,
    /// Mutable receiver binding carrying persistent state.
    receiver: HirId,
    /// Local bindings whose values originate in receiver-owned state.
    receiver_derived: &'set HashSet<HirId>,
    /// Whether the traversal found a receiver-owned value.
    is_found: bool,
}

impl<'tcx> Visitor<'tcx> for IteratorReceiverUse<'_, '_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // The first receiver-owned value completes this existence query.
        if is_iterator_evidence_receiver_rooted(self.cx, self.receiver, expression)
            || matches!(
                expression.kind,
                ExprKind::Path(path)
                    if matches!(
                        self.cx.qpath_res(&path, expression.hir_id),
                        Res::Local(binding) if self.receiver_derived.contains(&binding)
                    )
            )
        {
            self.is_found = true;
            return;
        }

        // Nested closures own independent traversal state.
        if matches!(expression.kind, ExprKind::Closure(_)) {
            return;
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

/// Collects top-level receiver fields read directly or through local aliases.
struct IteratorFieldUse<'set, 'analysis, 'tcx> {
    /// Compiler context used to resolve local paths.
    cx: &'analysis LateContext<'tcx>,
    /// Mutable receiver binding carrying persistent state.
    receiver: HirId,
    /// Receiver fields carried by each receiver-derived local.
    aliases: &'set HashMap<HirId, HashSet<Symbol>>,
    /// Receiver fields observed during traversal.
    fields: HashSet<Symbol>,
}

impl<'tcx> Visitor<'tcx> for IteratorFieldUse<'_, '_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Field(base, field) = expression.kind
            && is_iterator_evidence_direct_receiver(self.cx, self.receiver, base)
        {
            self.fields.insert(field.name);
        }
        if let ExprKind::Path(path) = expression.kind
            && let Res::Local(binding) = self.cx.qpath_res(&path, expression.hir_id)
            && let Some(fields) = self.aliases.get(&binding)
        {
            self.fields.extend(fields);
        }

        // Nested closures own independent traversal state.
        if matches!(expression.kind, ExprKind::Closure(_)) {
            return;
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

/// Persistent receiver-state evidence for one iterator-like method.
struct IteratorEvidence<'analysis, 'tcx> {
    /// Compiler context used to resolve local receiver paths.
    cx: &'analysis LateContext<'tcx>,
    /// Mutable receiver binding carrying persistent state.
    receiver: HirId,
    /// Expressions that contribute to a tail value or explicit return.
    returned: HashSet<HirId>,
    /// Locals whose values originate in receiver-owned state.
    receiver_derived: HashSet<HirId>,
    /// Receiver fields carried by each receiver-derived local.
    receiver_derived_fields: HashMap<HirId, HashSet<Symbol>>,
    /// Mutable evidence accumulated during traversal.
    state: IteratorEvidenceState,
}

impl<'analysis, 'tcx> IteratorEvidence<'analysis, 'tcx> {
    /// Analyzes one body for persistent cursor advancement.
    fn analyze(
        cx: &'analysis LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
        receiver: HirId,
    ) -> Option<Span> {
        // Traverse once while retaining explicit mutation and delegation evidence.
        let state = IteratorEvidenceState::default();
        let mut evidence = IteratorEvidence {
            cx,
            receiver,
            returned: IteratorResultCollector::collect(body.value),
            receiver_derived: HashSet::new(),
            receiver_derived_fields: HashMap::new(),
            state,
        };
        evidence.visit_expr(body.value);

        // Prefer direct delegation, then explicit state mutation rooted in the receiver.
        if evidence.state.has_transient_operation {
            return None;
        }

        // Direct delegation is the strongest available state-advance evidence.
        if let Some(delegated) = evidence.state.delegated_next {
            return Some(delegated);
        }
        evidence.state.has_returned_receiver_use.then(|| {
            evidence
                .state
                .mutations
                .iter()
                .find(|mutation| {
                    let field = mutation.field;
                    field.is_none_or(|field| {
                        evidence.state.returned_receiver_fields.contains(&field)
                    })
                })
                .map(|mutation| mutation.span)
        })?
    }

    /// Returns whether an expression is rooted in the mutable receiver.
    fn is_receiver_rooted(&self, expression: &'tcx Expr<'tcx>) -> bool {
        // Resolve direct receiver references before following projections.
        if let ExprKind::Path(path) = expression.kind {
            return matches!(
                self.cx.qpath_res(&path, expression.hir_id),
                Res::Local(binding) if binding == self.receiver
            );
        }

        // Preserve receiver provenance through non-owning projections.
        let base = match expression.kind {
            ExprKind::Field(base, _) | ExprKind::Unary(_, base) | ExprKind::AddrOf(_, _, base) => {
                Some(base)
            }
            ExprKind::Index(base, _, _) => Some(base),
            _ => None,
        };
        base.is_some_and(|base| self.is_receiver_rooted(base))
    }

    /// Returns whether an expression reads receiver-owned state directly or through a local.
    fn is_using_receiver_value(&self, expression: &'tcx Expr<'tcx>) -> bool {
        let mut use_finder = IteratorReceiverUse {
            cx: self.cx,
            receiver: self.receiver,
            receiver_derived: &self.receiver_derived,
            is_found: false,
        };
        use_finder.visit_expr(expression);
        use_finder.is_found
    }

    /// Collects top-level receiver fields read by an expression and its aliases.
    fn receiver_fields(&self, expression: &'tcx Expr<'tcx>) -> HashSet<Symbol> {
        let mut field_use = IteratorFieldUse {
            cx: self.cx,
            receiver: self.receiver,
            aliases: &self.receiver_derived_fields,
            fields: HashSet::new(),
        };
        field_use.visit_expr(expression);
        field_use.fields
    }

    /// Returns the top-level receiver field targeted by an assignment.
    fn mutated_receiver_field(&self, expression: &'tcx Expr<'tcx>) -> Option<Symbol> {
        match expression.kind {
            ExprKind::Field(base, field)
                if is_iterator_evidence_direct_receiver(self.cx, self.receiver, base) =>
            {
                Some(field.name)
            }
            ExprKind::Field(base, _)
            | ExprKind::Unary(_, base)
            | ExprKind::AddrOf(_, _, base)
            | ExprKind::Index(base, _, _) => self.mutated_receiver_field(base),
            _ => None,
        }
    }

    /// Classifies one receiver-rooted method as delegation or transient removal.
    fn record_receiver_method(&mut self, name: &str, span: Span) {
        if name == "next" {
            self.state.delegated_next.get_or_insert(span);
        }

        // Queue and channel removal represent availability, not iterator exhaustion.
        self.state.has_transient_operation |= ITERATOR_VOCABULARY_TRANSIENT_METHODS.contains(&name);
    }
}

impl<'tcx> Visitor<'tcx> for IteratorEvidence<'_, 'tcx> {
    fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
        if let StmtKind::Let(local) = statement.kind
            && let Some(initializer) = local.init
            && self.is_using_receiver_value(initializer)
            && let PatKind::Binding(_, binding, _, None) = local.pat.kind
        {
            self.receiver_derived.insert(binding);
            self.receiver_derived_fields
                .insert(binding, self.receiver_fields(initializer));
        }
        intravisit::walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Only receiver values contributing to the method result establish yielded-item evidence.
        if self.returned.contains(&expression.hir_id) && self.is_using_receiver_value(expression) {
            self.state.has_returned_receiver_use = true;
            self.state
                .returned_receiver_fields
                .extend(self.receiver_fields(expression));
        }

        // Retain only explicit receiver mutation and direct inner-iterator delegation.
        match expression.kind {
            ExprKind::Assign(left, _, _) | ExprKind::AssignOp(_, left, _)
                if self.is_receiver_rooted(left) =>
            {
                self.state.mutations.push(IteratorMutation {
                    span: expression.span,
                    field: self.mutated_receiver_field(left),
                });
            }
            ExprKind::MethodCall(segment, receiver, _, _)
                if self.returned.contains(&expression.hir_id)
                    && self.is_receiver_rooted(receiver) =>
            {
                self.record_receiver_method(segment.ident.name.as_str(), expression.span);
            }
            // Closure bodies carry traversal state independent from the surrounding method.
            ExprKind::Closure(_) => return,
            _ => {}
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

/// Collects every non-closure expression contributing to one result value.
struct IteratorValueCollector<'set> {
    /// Destination set of contributing expression identifiers.
    expressions: &'set mut HashSet<HirId>,
}

impl<'tcx> Visitor<'tcx> for IteratorValueCollector<'_> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Nested closures return from a different callable.
        if matches!(expression.kind, ExprKind::Closure(_)) {
            return;
        }
        self.expressions.insert(expression.hir_id);
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

// -----------------------------------------------------------------------------
// IteratorResultCollector: Returned traversal ownership
// -----------------------------------------------------------------------------

/// Finds expressions that contribute to tail values and explicit returns.
#[derive(Default)]
struct IteratorResultCollector {
    /// Expressions contributing to the callable's returned traversal.
    expressions: HashSet<HirId>,
}

impl IteratorResultCollector {
    /// Adds every non-closure expression contributing to one result value.
    fn visit_value_expr(&mut self, expression: &Expr<'_>) {
        IteratorValueCollector {
            expressions: &mut self.expressions,
        }
        .visit_expr(expression);
    }

    /// Follows expressions that can supply the callable result.
    fn visit_result_expr<'tcx>(&mut self, expression: &'tcx Expr<'tcx>) {
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
            ExprKind::Ret(Some(value)) | ExprKind::DropTemps(value) => {
                self.visit_result_expr(value);
            }
            ExprKind::Closure(_) => {}
            _ => self.visit_value_expr(expression),
        }
    }

    /// Collects expressions contributing to one callable result.
    fn collect(expression: &Expr<'_>) -> HashSet<HirId> {
        let mut collector = Self::default();
        collector.visit_result_expr(expression);
        collector.expressions
    }
}

impl<'tcx> Visitor<'tcx> for IteratorResultCollector {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        match expression.kind {
            ExprKind::Ret(Some(value)) => self.visit_result_expr(value),
            ExprKind::Closure(_) => {}
            _ => intravisit::walk_expr(self, expression),
        }
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}
