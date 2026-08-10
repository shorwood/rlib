extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def::Res;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{Body, Expr, ExprKind, HirId, Item, ItemKind, LoopSource, Mutability, PatKind};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::{Span, Symbol};

use super::identifier_case;

// -----------------------------------------------------------------------------
// Collection: Standard sequence ingestion model
// -----------------------------------------------------------------------------

/// Standard collection trait represented by an authored API.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CollectionContract {
    /// Complete construction from an item sequence.
    FromIterator,
    /// In-place extension from an item sequence.
    Extend,
}

impl CollectionContract {
    /// Resolves one standard collection contract from its trait path.
    fn from_trait_path(path: &str) -> Option<Self> {
        if path.ends_with("::FromIterator") {
            Some(Self::FromIterator)
        } else if path.ends_with("::Extend") {
            Some(Self::Extend)
        } else {
            None
        }
    }

    /// Returns the standard trait name shown in diagnostics.
    pub const fn trait_name(self) -> &'static str {
        match self {
            Self::FromIterator => "FromIterator",
            Self::Extend => "Extend",
        }
    }
}

/// Why an otherwise valid collection protocol remains reportable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionProblem {
    /// One implementation exists without the standard trait.
    MissingTrait,
    /// Several neutral implementations compete for one item contract.
    AmbiguousFamily {
        /// Number of neutral APIs competing for this item family.
        count: usize,
    },
    /// The standard trait and a separate implementation both own the behavior.
    CompetingTrait,
}

impl CollectionProblem {
    /// Classifies ownership for one exact collection family.
    const fn classify(candidate_count: usize, occupancy: CollectionFamilyOccupancy) -> Self {
        if candidate_count > 1 {
            return Self::AmbiguousFamily {
                count: candidate_count,
            };
        }
        if matches!(occupancy, CollectionFamilyOccupancy::Occupied) {
            Self::CompetingTrait
        } else {
            Self::MissingTrait
        }
    }
}

// -----------------------------------------------------------------------------
// CollectionFamilyCandidate: Complete diagnostic context
// -----------------------------------------------------------------------------

/// One proven sequence-to-storage protocol.
#[derive(Clone)]
pub struct CollectionFamilyCandidateSource {
    /// Function definition used for lint levels and cross-policy precedence.
    pub hir_id: HirId,
    /// Authored function name.
    pub name: Symbol,
    /// Function identifier source range.
    pub name_span: Span,
    /// Iterable input range.
    pub source_span: Span,
    /// Storage operation proving complete item ingestion.
    pub evidence_span: Span,
}

/// Inferred standard collection contract shown in diagnostics.
#[derive(Clone)]
pub struct CollectionFamilyCandidateProtocol {
    /// Local collection wrapper.
    pub target_name: String,
    /// Stored item family inferred from target storage.
    pub item_name: String,
    /// Standard collection protocol selected by the signature.
    pub contract: CollectionContract,
}

// -----------------------------------------------------------------------------
// CollectionFamily: Crate wide collection family
// -----------------------------------------------------------------------------

/// One proven sequence-to-storage protocol.
#[derive(Clone)]
pub struct CollectionFamilyCandidate {
    /// Function definition used for cross-policy precedence.
    def_id: LocalDefId,
    /// Authored function identity and storage-flow evidence.
    pub source: CollectionFamilyCandidateSource,
    /// Inferred standard collection protocol.
    pub protocol: CollectionFamilyCandidateProtocol,
    /// Local wrapper definition used for family selection.
    target_def_id: LocalDefId,
    /// Whether the body delegates to a standard collection operation.
    has_standard_trait_delegation: bool,
}

/// Reportable collection protocol with family classification.
pub struct CollectionFamilyFinding<'candidate> {
    /// Candidate carrying precise source and remediation context.
    pub candidate: &'candidate CollectionFamilyCandidate,
    /// Missing, ambiguous, or competing protocol ownership.
    pub problem: CollectionProblem,
}

/// Local target and standard trait used for occupancy checks.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct CollectionFamilyOccupancyKey {
    /// Local collection wrapper.
    target_def_id: LocalDefId,
    /// Construction or extension contract.
    contract: CollectionContract,
}

/// Exact target, trait, and item family used for ambiguity analysis.
#[derive(Eq, Hash, PartialEq)]
struct CollectionFamilyKey {
    /// Local collection wrapper.
    target_def_id: LocalDefId,
    /// Construction or extension contract.
    contract: CollectionContract,
    /// Stored item family.
    item_name: String,
}

/// Whether a standard trait already owns one collection family.
#[derive(Clone, Copy)]
enum CollectionFamilyOccupancy {
    /// No standard trait implementation owns this family.
    Available,
    /// A standard trait implementation already owns this family.
    Occupied,
}

impl CollectionFamilyOccupancy {
    /// Converts local trait-table presence into explicit family occupancy.
    fn from_entry<T>(entry: Option<T>) -> Self {
        entry.map_or(Self::Available, |_| Self::Occupied)
    }

    /// Returns whether one candidate still deserves a diagnostic.
    const fn is_reportable(self, candidate: &CollectionFamilyCandidate) -> bool {
        matches!(self, Self::Available) || !candidate.has_standard_trait_delegation
    }
}

// -----------------------------------------------------------------------------
// CollectionConstructionAnalysis: Crate wide family selection
// -----------------------------------------------------------------------------

/// Finds constructors and mutators that reproduce standard collection traits.
#[derive(Default)]
pub struct CollectionConstructionAnalysis {
    /// Structurally proven sequence-ingestion APIs.
    candidates: Vec<CollectionFamilyCandidate>,
    /// Target and trait pairs already owned by standard implementations.
    occupied: HashSet<CollectionFamilyOccupancyKey>,
}

impl CollectionConstructionAnalysis {
    /// Records existing `FromIterator` and `Extend` implementations.
    pub fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Resolve only standard collection trait implementations on local types.
        let ItemKind::Impl(_) = item.kind else {
            return;
        };
        let Some(trait_ref) = cx.tcx.impl_opt_trait_ref(item.owner_id.def_id) else {
            return;
        };

        // Resolve the local implementation target and standard trait identity.
        let trait_ref = trait_ref.instantiate_identity();
        let self_ty = cx.tcx.type_of(item.owner_id.def_id).instantiate_identity();
        let Some(target_def_id) = collection_classification_local_adt(self_ty) else {
            return;
        };
        let path = cx.tcx.def_path_str(trait_ref.def_id);
        let Some(contract) = CollectionContract::from_trait_path(&path) else {
            return;
        };

        // Occupancy is independent of the trait's generic item argument.
        self.occupied.insert(CollectionFamilyOccupancyKey {
            target_def_id,
            contract,
        });
    }

    /// Records free and inherent constructors or extension operations.
    pub fn record_function<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        body: &'tcx Body<'tcx>,
        def_id: LocalDefId,
    ) {
        // Reject trait methods and unsupported compile-time or unsafe contracts.
        if collection_classification_is_trait_method(cx, def_id) {
            return;
        }

        // Resolve the authored identifier and reject unsupported function kinds.
        let (ident, header) = match kind {
            FnKind::ItemFn(ident, _, header) => (ident, header),
            FnKind::Method(ident, signature) => (ident, signature.header),
            FnKind::Closure => return,
        };

        // Exclude compile-time and unsafe APIs from automatic protocol inference.
        if header.constness == rustc_hir::Constness::Const
            || matches!(
                header.safety,
                rustc_hir::HeaderSafety::Normal(rustc_hir::Safety::Unsafe)
            )
        {
            return;
        }

        // Resolve a supported constructor or extension signature and its source.
        let signature = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();
        let Some(shape) = CollectionFunctionShape::classify(signature.inputs(), signature.output())
        else {
            return;
        };
        if body.params.len() != shape.parameter_count {
            return;
        }

        // Contextual names preserve domain policy outside the standard trait contract.
        let has_neutral_name = collection_classification_has_neutral_name(
            cx,
            ident.name,
            shape.target_def_id,
            shape.contract,
        );
        if !has_neutral_name {
            return;
        }

        // Bind the iterable parameter so body evidence can track its complete consumption.
        let source_parameter = &body.params[shape.source_index];
        let PatKind::Binding(_, source_binding, _, None) = source_parameter.pat.kind else {
            return;
        };

        // Require complete source iteration into storage physically owned by the target.
        let storage = collection_storage_discover(cx, shape.target_def_id);
        let Some(evidence) = CollectionEvidence::analyze(cx, body, source_binding, &storage) else {
            return;
        };

        // Preserve authored API identity and exact source-to-storage evidence.
        let source = CollectionFamilyCandidateSource {
            hir_id: cx.tcx.local_def_id_to_hir_id(def_id),
            name: ident.name,
            name_span: ident.span,
            source_span: source_parameter.span,
            evidence_span: evidence.span,
        };

        // Retain target, item, and standard trait as one inferred protocol.
        let protocol = CollectionFamilyCandidateProtocol {
            target_name: cx.tcx.def_path_str(shape.target_def_id.to_def_id()),
            item_name: evidence.item_name,
            contract: shape.contract,
        };

        // Store the compact source and protocol contexts with private family state.
        self.candidates.push(CollectionFamilyCandidate {
            def_id,
            source,
            protocol,
            target_def_id: shape.target_def_id,
            has_standard_trait_delegation: evidence.has_standard_trait_delegation,
        });
    }

    /// Returns stable findings for every proven target/item family.
    pub fn findings(&self) -> Vec<CollectionFamilyFinding<'_>> {
        let mut families = HashMap::<CollectionFamilyKey, Vec<&CollectionFamilyCandidate>>::new();
        for candidate in &self.candidates {
            let key = CollectionFamilyKey {
                target_def_id: candidate.target_def_id,
                contract: candidate.protocol.contract,
                item_name: candidate.protocol.item_name.clone(),
            };
            families.entry(key).or_default().push(candidate);
        }
        let mut findings = Vec::new();

        // Classify each exact item family before creating candidate diagnostics.
        for (key, family) in families {
            // Resolve standard trait occupancy for this target and contract.
            let occupancy_key = CollectionFamilyOccupancyKey {
                target_def_id: key.target_def_id,
                contract: key.contract,
            };
            let occupancy =
                CollectionFamilyOccupancy::from_entry(self.occupied.get(&occupancy_key));
            let problem = CollectionProblem::classify(family.len(), occupancy);

            // Suppress only direct delegation into an already occupied standard trait.
            let reportable = family
                .into_iter()
                .filter(|candidate| occupancy.is_reportable(candidate));
            for candidate in reportable {
                findings.push(CollectionFamilyFinding { candidate, problem });
            }
        }
        findings.sort_unstable_by_key(|finding| finding.candidate.source.name_span.lo());
        findings
    }

    /// Returns definitions owned by this stronger standard-protocol policy.
    pub fn reportable_definitions(&self) -> HashSet<LocalDefId> {
        let findings = self.findings();
        let mut definitions = HashSet::new();
        for finding in findings {
            definitions.insert(finding.candidate.def_id);
        }
        definitions
    }
}

// -----------------------------------------------------------------------------
// CollectionClassification: Storage and signature helpers
// -----------------------------------------------------------------------------

/// Resolves a possibly referenced type to one local nominal definition.
fn collection_classification_local_adt(ty: Ty<'_>) -> Option<LocalDefId> {
    let ty::Adt(definition, _) = ty.peel_refs().kind() else {
        return None;
    };
    definition.did().as_local()
}

/// Returns whether a function returns a mutable reference to the selected target.
fn collection_classification_returns_target(output: Ty<'_>, target: LocalDefId) -> bool {
    let ty::Ref(_, returned, Mutability::Mut) = output.kind() else {
        return false;
    };
    collection_classification_local_adt(*returned) == Some(target)
}

/// Returns whether an associated function belongs to a trait implementation.
fn collection_classification_is_trait_method(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
    cx.tcx.opt_local_parent(def_id).is_some_and(|parent| {
        matches!(
            cx.tcx.def_kind(parent),
            rustc_hir::def::DefKind::Impl { of_trait: true }
        )
    })
}

/// Returns whether a name claims an unqualified collection protocol.
fn collection_classification_has_neutral_name(
    cx: &LateContext<'_>,
    name: Symbol,
    target: LocalDefId,
    contract: CollectionContract,
) -> bool {
    /// Vocabulary exposing policy that standard collection traits cannot carry.
    const CONTEXTUAL: &[&str] = &[
        "Deduplicate",
        "Filtered",
        "First",
        "Last",
        "Limited",
        "Lossy",
        "Policy",
        "Sorted",
        "Strict",
        "Truncated",
        "Validated",
        "With",
    ];
    /// Vocabulary that adds no policy beyond sequence ingestion.
    const NEUTRAL: &[&str] = &[
        "Add", "All", "Append", "Build", "Collect", "Create", "Entries", "Extend", "From", "Items",
        "New",
    ];
    let words = identifier_case::words(name.as_str());
    if words.iter().any(|word| CONTEXTUAL.contains(&word.as_str())) {
        return false;
    }
    let target_words = identifier_case::words(cx.tcx.item_name(target.to_def_id()).as_str());
    words.into_iter().all(|word| {
        NEUTRAL.contains(&word.as_str())
            || target_words.contains(&word)
            || (contract == CollectionContract::Extend && word == "Insert")
    })
}

// -----------------------------------------------------------------------------
// CollectionFunctionShape: Constructor or extension signature
// -----------------------------------------------------------------------------

/// Supported constructor or extension signature and iterable position.
struct CollectionFunctionShape {
    /// Local wrapper constructed or mutated by the function.
    target_def_id: LocalDefId,
    /// Construction or extension trait selected by the signature.
    contract: CollectionContract,
    /// Exact authored parameter count required by this shape.
    parameter_count: usize,
    /// Parameter position containing the iterable source.
    source_index: usize,
}

impl CollectionFunctionShape {
    /// Classifies one function as complete construction or in-place extension.
    fn classify(inputs: &[Ty<'_>], output: Ty<'_>) -> Option<Self> {
        // Prefer the single-source constructor shape before testing mutation.
        if inputs.len() == 1
            && let Some(target_def_id) = collection_classification_local_adt(output)
        {
            return Some(Self {
                target_def_id,
                contract: CollectionContract::FromIterator,
                parameter_count: 1,
                source_index: 0,
            });
        }

        // Extension accepts a mutable target followed by exactly one iterable source.
        let [receiver, _source] = inputs else {
            return None;
        };
        let ty::Ref(_, target, Mutability::Mut) = receiver.kind() else {
            return None;
        };
        let target_def_id = collection_classification_local_adt(*target)?;

        // Fluent mutation may return the same mutable receiver instead of unit.
        let returns_unit = output.is_unit();
        let returns_receiver = collection_classification_returns_target(output, target_def_id);

        // Materialize the extension contract only after validating its return shape.
        let shape = Self {
            target_def_id,
            contract: CollectionContract::Extend,
            parameter_count: 2,
            source_index: 1,
        };
        (returns_unit || returns_receiver).then_some(shape)
    }
}

// -----------------------------------------------------------------------------
// CollectionStorage: Target owned standard storage
// -----------------------------------------------------------------------------

/// One standard collection field and the item family it stores.
struct CollectionStorageField<'tcx> {
    /// Resolved field type used to recognize storage receiver expressions.
    ty: Ty<'tcx>,
    /// Item or key-value tuple stored by the collection.
    item_name: String,
}

/// Discovers standard collection fields directly owned by one local wrapper.
fn collection_storage_discover<'tcx>(
    cx: &LateContext<'tcx>,
    target: LocalDefId,
) -> Vec<CollectionStorageField<'tcx>> {
    let fields = cx.tcx.adt_def(target.to_def_id()).all_fields();
    fields
        .filter_map(|field| {
            let ty = cx.tcx.type_of(field.did).instantiate_identity();
            let ty::Adt(definition, arguments) = ty.kind() else {
                return None;
            };
            let path = cx.tcx.def_path_str(definition.did());
            let item = if path.ends_with("::HashMap") || path.ends_with("::BTreeMap") {
                format!("({}, {})", arguments.type_at(0), arguments.type_at(1))
            } else if [
                "::Vec",
                "::VecDeque",
                "::HashSet",
                "::BTreeSet",
                "::BinaryHeap",
                "::LinkedList",
            ]
            .iter()
            .any(|suffix| path.ends_with(suffix))
            {
                arguments.type_at(0).to_string()
            } else {
                return None;
            };
            Some(CollectionStorageField {
                ty,
                item_name: item,
            })
        })
        .collect()
}

// -----------------------------------------------------------------------------
// CollectionEvidence: Complete source to storage flow proof
// -----------------------------------------------------------------------------

/// Proven storage operation and its inferred item family.
struct CollectionEvidenceResult {
    /// Source range of the operation storing sequence items.
    span: Span,
    /// Item family inferred from the target field.
    item_name: String,
    /// Whether storage delegates through a standard `extend` operation.
    has_standard_trait_delegation: bool,
}

/// Mutable facts accumulated while traversing one collection-like body.
#[derive(Default)]
struct CollectionEvidenceState {
    /// Whether traversal reached the iterable source.
    has_seen_source: bool,
    /// Whether a for-loop or direct extend consumes the whole source.
    has_complete_iteration: bool,
    /// First operation storing source items.
    result: Option<CollectionEvidenceResult>,
    /// Whether filtering or truncation introduces hidden policy.
    has_policy: bool,
}

/// Proves that one source is completely consumed into target-owned storage.
struct CollectionEvidence<'analysis, 'tcx, 'storage> {
    /// Compiler context used for path and expression type resolution.
    cx: &'analysis LateContext<'tcx>,
    /// Iterable source parameter binding.
    source: HirId,
    /// Standard collection fields owned by the target.
    storage: &'storage [CollectionStorageField<'tcx>],
    /// Mutable evidence accumulated during traversal.
    state: CollectionEvidenceState,
}

impl CollectionEvidence<'_, '_, '_> {
    /// Analyzes one candidate body for complete source-to-storage flow.
    fn analyze<'tcx>(
        cx: &LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
        source: HirId,
        storage: &[CollectionStorageField<'tcx>],
    ) -> Option<CollectionEvidenceResult> {
        // A target without standard storage cannot own a collection protocol.
        if storage.is_empty() {
            return None;
        }

        // Traverse only after the target proves it owns standard collection storage.
        let state = CollectionEvidenceState::default();
        let mut evidence = CollectionEvidence {
            cx,
            source,
            storage,
            state,
        };

        // Traverse once after initializing independent flow and policy state.
        evidence.visit_expr(body.value);

        // Report only complete, policy-free ingestion with a proven storage operation.
        (!evidence.state.has_policy
            && evidence.state.has_seen_source
            && evidence.state.has_complete_iteration)
            .then_some(evidence.state.result)
            .flatten()
    }
}

impl<'tcx> CollectionEvidence<'_, 'tcx, '_> {
    /// Returns whether an expression references the iterable source parameter.
    fn uses_source(&self, expression: &'tcx Expr<'tcx>) -> bool {
        /// Finds a reference to the exact iterable source binding.
        struct SourceFinder<'analysis, 'tcx> {
            /// Compiler context used to resolve local paths.
            cx: &'analysis LateContext<'tcx>,
            /// Source binding being searched.
            source: HirId,
            /// Whether traversal reached the source binding.
            has_found: bool,
        }
        impl<'tcx> Visitor<'tcx> for SourceFinder<'_, 'tcx> {
            fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
                if let ExprKind::Path(path) = expression.kind
                    && matches!(self.cx.qpath_res(&path, expression.hir_id), Res::Local(binding) if binding == self.source)
                {
                    self.has_found = true;
                    return;
                }
                intravisit::walk_expr(self, expression);
            }
        }
        let mut finder = SourceFinder {
            cx: self.cx,
            source: self.source,
            has_found: false,
        };
        finder.visit_expr(expression);
        finder.has_found
    }

    /// Resolves a receiver expression to one target-owned collection field.
    fn storage_item(&self, receiver: &'tcx Expr<'tcx>) -> Option<String> {
        let receiver_ty = self.cx.typeck_results().expr_ty(receiver).peel_refs();
        self.storage
            .iter()
            .find(|storage| storage.ty.peel_refs() == receiver_ty)
            .map(|storage| storage.item_name.clone())
    }

    /// Records filtering or truncation that changes which source items are stored.
    fn record_policy_adapter(
        &mut self,
        name: &str,
        receiver: &'tcx Expr<'tcx>,
        arguments: &'tcx [Expr<'tcx>],
    ) {
        let is_policy = ["filter", "filter_map", "skip", "take", "take_while"].contains(&name);
        let uses_source = self.uses_source(receiver)
            || arguments.iter().any(|argument| self.uses_source(argument));
        self.state.has_policy |= is_policy && uses_source;
    }

    /// Records a write into collection storage owned by the target type.
    fn record_storage_operation(
        &mut self,
        expression: &'tcx Expr<'tcx>,
        name: &str,
        receiver: &'tcx Expr<'tcx>,
        arguments: &'tcx [Expr<'tcx>],
    ) {
        // Ignore methods that cannot write into standard target-owned storage.
        if !["extend", "insert", "push", "push_back", "push_front"].contains(&name) {
            return;
        }
        let Some(item_name) = self.storage_item(receiver) else {
            return;
        };

        // Direct `extend(source)` proves complete iteration and standard delegation.
        if name == "extend" && arguments.iter().any(|argument| self.uses_source(argument)) {
            self.state.has_complete_iteration = true;
        }
        self.state.result.get_or_insert(CollectionEvidenceResult {
            span: expression.span,
            item_name,
            has_standard_trait_delegation: name == "extend",
        });
    }
}

impl<'tcx> Visitor<'tcx> for CollectionEvidence<'_, 'tcx, '_> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        self.state.has_seen_source |= self.uses_source(expression);
        match expression.kind {
            ExprKind::Loop(_, _, LoopSource::ForLoop, _) => {
                self.state.has_complete_iteration = true;
            }
            ExprKind::MethodCall(segment, receiver, arguments, _) => {
                let name = segment.ident.name.as_str();
                self.record_policy_adapter(name, receiver, arguments);
                self.record_storage_operation(expression, name, receiver, arguments);
            }
            _ => {}
        }
        intravisit::walk_expr(self, expression);
    }
}
