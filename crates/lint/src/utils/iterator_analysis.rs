extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{Body, Expr, ExprKind, HirId, Item, ItemKind, Mutability, PatKind};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::symbol::sym;
use rustc_span::{Span, Symbol};

use super::identifier_case;

// -----------------------------------------------------------------------------
// IteratorCandidate: Complete diagnostic context
// -----------------------------------------------------------------------------
#[derive(Clone)]
/// One unique stateful traversal that should use Rust's iterator protocol.
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
#[derive(Clone)]
/// Inferred iterator contract shown in diagnostics.
pub struct IteratorCandidateProtocol {
    /// Receiver type that should own `Iterator`.
    pub type_name: String,
    /// Concrete yielded item spelling.
    pub item_name: String,
}
#[derive(Clone)]
/// One unique stateful traversal that should use Rust's iterator protocol.
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
#[derive(Default)]
/// Finds unique cursor-like methods that reproduce `Iterator::next`.
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
        if iterator_classification_is_trait_method(cx, def_id) {
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
        if is_const || is_unsafe || body.params.len() != 1 {
            return;
        }

        // Resolve the mutable local receiver before inspecting its yielded item.
        let function = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();
        let [receiver] = function.inputs() else {
            return;
        };
        let ty::Ref(_, receiver, Mutability::Mut) = receiver.kind() else {
            return;
        };
        let Some(type_def_id) = iterator_classification_local_adt(*receiver) else {
            return;
        };

        // Require an owned item and a name without contextual traversal policy.
        let Some(item) = iterator_classification_option_item(cx, function.output()) else {
            return;
        };
        if matches!(item.kind(), ty::Ref(..) | ty::RawPtr(..))
            || !iterator_classification_has_neutral_name(cx, ident.name, type_def_id, item)
        {
            return;
        }

        // Prove that the body advances persistent receiver state.
        let PatKind::Binding(_, receiver_binding, _, None) = body.params[0].pat.kind else {
            return;
        };
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
        for analyze_candidate in &self.candidates {
            families
                .entry(analyze_candidate.type_def_id)
                .or_default()
                .push(analyze_candidate);
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
        findings.sort_unstable_by_key(|analyze_candidate| analyze_candidate.source.name_span.lo());
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
    let ty::Adt(definition, _) = ty.peel_refs().kind() else {
        return None;
    };
    definition.did().as_local()
}

/// Returns whether an associated function belongs to a trait implementation.
fn iterator_classification_is_trait_method(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
    cx.tcx
        .opt_local_parent(def_id)
        .is_some_and(|parent| matches!(cx.tcx.def_kind(parent), DefKind::Impl { of_trait: true }))
}

/// Extracts the item from an exact standard `Option<Item>` return.
fn iterator_classification_option_item<'tcx>(
    cx: &LateContext<'tcx>,
    ty: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    let ty::Adt(definition, arguments) = ty.kind() else {
        return None;
    };
    cx.tcx
        .is_diagnostic_item(sym::Option, definition.did())
        .then(|| arguments.type_at(0))
}

/// Returns whether a method name claims unqualified sequence advancement.
fn iterator_classification_has_neutral_name(
    cx: &LateContext<'_>,
    name: Symbol,
    type_def_id: LocalDefId,
    item: Ty<'_>,
) -> bool {
    // Reject names carrying availability, parsing, or selection policy.
    let words = identifier_case::words(name.as_str());
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
// IteratorEvidence: Persistent state advance proof
// -----------------------------------------------------------------------------
#[derive(Default)]
/// Mutable facts accumulated while traversing one iterator-like body.
struct IteratorEvidenceState {
    /// First explicit assignment to receiver-owned state.
    mutation: Option<Span>,
    /// Direct delegation to an inner iterator's `next`.
    delegated_next: Option<Span>,
    /// Whether the body reads any receiver-owned value.
    has_receiver_use: bool,
    /// Whether the body performs queue-like or temporary removal.
    has_transient_operation: bool,
}

/// Persistent receiver-state evidence for one iterator-like method.
struct IteratorEvidence<'analysis, 'tcx> {
    /// Compiler context used to resolve local receiver paths.
    cx: &'analysis LateContext<'tcx>,
    /// Mutable receiver binding carrying persistent state.
    receiver: HirId,
    /// Mutable evidence accumulated during traversal.
    state: IteratorEvidenceState,
}

impl IteratorEvidence<'_, '_> {
    /// Analyzes one body for persistent cursor advancement.
    fn analyze<'tcx>(
        cx: &LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
        receiver: HirId,
    ) -> Option<Span> {
        // Traverse once while retaining explicit mutation and delegation evidence.
        let state = IteratorEvidenceState::default();
        let mut evidence = IteratorEvidence {
            cx,
            receiver,
            state,
        };
        evidence.visit_expr(body.value);

        // Prefer direct delegation, then explicit state mutation rooted in the receiver.
        if evidence.state.has_transient_operation {
            return None;
        }
        if let Some(delegated) = evidence.state.delegated_next {
            return Some(delegated);
        }
        evidence
            .state
            .has_receiver_use
            .then_some(evidence.state.mutation)
            .flatten()
    }

    /// Returns whether an expression is rooted in the mutable receiver.
    fn is_receiver_rooted(&self, expression: &Expr<'_>) -> bool {
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
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Mark any direct or projected receiver read before classifying mutations.
        if self.is_receiver_rooted(expression) {
            self.state.has_receiver_use = true;
        }

        // Retain only explicit receiver mutation and direct inner-iterator delegation.
        match expression.kind {
            ExprKind::Assign(left, _, _) | ExprKind::AssignOp(_, left, _)
                if self.is_receiver_rooted(left) =>
            {
                self.state.mutation.get_or_insert(expression.span);
            }
            ExprKind::MethodCall(segment, receiver, _, _) if self.is_receiver_rooted(receiver) => {
                self.record_receiver_method(segment.ident.name.as_str(), expression.span);
            }
            _ => {}
        }
        intravisit::walk_expr(self, expression);
    }
}
