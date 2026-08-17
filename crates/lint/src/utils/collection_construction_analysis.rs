extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{Body, Expr, ExprKind, HirId, Item, ItemKind, MatchSource, Mutability, PatKind};
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
        // Multiple candidates leave standard collection-trait ownership ambiguous.
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
    fn for_standard_trait_presence(entry: Option<&CollectionFamilyOccupancyKey>) -> Self {
        entry.map_or(Self::Available, |_| Self::Occupied)
    }

    /// Returns whether one `candidate` still deserves a diagnostic.
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

        // Trait occupancy requires a resolved trait implementation reference.
        let Some(trait_ref) = cx.tcx.impl_opt_trait_ref(item.owner_id.def_id) else {
            return;
        };

        // Resolve the local implementation target and standard trait identity.
        let trait_ref = trait_ref.instantiate_identity();
        let self_ty = cx.tcx.type_of(item.owner_id.def_id).instantiate_identity();

        // Only local nominal types can own a collection trait family.
        let Some(target_def_id) = collection_classification_local_adt(self_ty) else {
            return;
        };
        let path = cx.tcx.def_path_str(trait_ref.def_id);

        // Only standard collection traits occupy an ingestion protocol family.
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

            // Closures do not define a named collection API.
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

        // The signature must match one supported construction or extension shape.
        let Some(shape) = CollectionFunctionShape::classify(signature.inputs(), signature.output())
        else {
            return;
        };

        // The body parameter count must agree with the inferred collection shape.
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

        // Contextual names indicate policy beyond the standard collection contract.
        if !has_neutral_name {
            return;
        }

        // Bind the iterable parameter so body evidence can track its complete consumption.
        let source_parameter = &body.params[shape.source_index];

        // Source-flow analysis requires a direct iterable parameter binding.
        let PatKind::Binding(_, source_binding, _, None) = source_parameter.pat.kind else {
            return;
        };

        // Require complete source iteration into storage physically owned by the target.
        let storage = collection_storage_discover(cx, shape.target_def_id);

        // A candidate needs one complete source-to-target-storage ingestion proof.
        let Some(evidence) =
            CollectionEvidence::analyze(cx, body, source_binding, &storage, shape.target_def_id)
        else {
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

            // Classify standard-trait occupancy before selecting the family problem.
            let occupancy = CollectionFamilyOccupancy::for_standard_trait_presence(
                self.occupied.get(&occupancy_key),
            );
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
// CollectionClassification: Storage and signature classification helpers
// -----------------------------------------------------------------------------

/// Context words that make a collection-oriented name specific enough.
const COLLECTION_CLASSIFICATION_CONTEXTUAL_NAMES: &[&str] = &[
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

/// Generic collection words that cannot establish protocol ownership alone.
const COLLECTION_CLASSIFICATION_NEUTRAL_NAMES: &[&str] = &[
    "Add", "All", "Append", "Build", "Collect", "Create", "Entries", "Extend", "From", "Items",
    "New",
];

/// Resolves a possibly referenced type to one local nominal definition.
fn collection_classification_local_adt(ty: Ty<'_>) -> Option<LocalDefId> {
    // Only nominal types can identify a local collection wrapper.
    let ty::Adt(definition, _) = ty.peel_refs().kind() else {
        return None;
    };
    definition.did().as_local()
}

/// Returns whether a function returns a mutable reference to the selected target.
fn collection_classification_returns_target(output: Ty<'_>, target: LocalDefId) -> bool {
    // Fluent extension must return a mutable reference to the target wrapper.
    let ty::Ref(_, returned, Mutability::Mut) = output.kind() else {
        return false;
    };
    collection_classification_local_adt(*returned) == Some(target)
}

/// Returns whether an associated function belongs to a trait implementation.
fn collection_classification_is_trait_method(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
    cx.tcx
        .opt_local_parent(def_id)
        .is_some_and(|parent| matches!(cx.tcx.def_kind(parent), DefKind::Impl { of_trait: true }))
}

/// Returns whether authored vocabulary claims only a generic collection contract.
fn collection_classification_has_neutral_name(
    cx: &LateContext<'_>,
    name: Symbol,
    target: LocalDefId,
    contract: CollectionContract,
) -> bool {
    let words = identifier_case::words(name.as_str());

    // Contextual vocabulary encodes policy that a standard collection trait cannot express.
    if words
        .iter()
        .any(|word| COLLECTION_CLASSIFICATION_CONTEXTUAL_NAMES.contains(&word.as_str()))
    {
        return false;
    }
    let target_words = identifier_case::words(cx.tcx.item_name(target.to_def_id()).as_str());
    words.into_iter().all(|word| {
        COLLECTION_CLASSIFICATION_NEUTRAL_NAMES.contains(&word.as_str())
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

        // The extension receiver must be a mutable reference to its target.
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
// CollectionStorageField: Target owned standard storage
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

            // Storage discovery considers only nominal standard collection field types.
            let ty::Adt(definition, arguments) = ty.kind() else {
                return None;
            };
            let path = cx.tcx.def_path_str(definition.did());
            let item = match () {
                () if path.ends_with("::HashMap") || path.ends_with("::BTreeMap") => {
                    format!("({}, {})", arguments.type_at(0), arguments.type_at(1))
                }
                () if [
                    "::Vec",
                    "::VecDeque",
                    "::HashSet",
                    "::BTreeSet",
                    "::BinaryHeap",
                    "::LinkedList",
                ]
                .iter()
                .any(|suffix| path.ends_with(suffix)) =>
                {
                    arguments.type_at(0).to_string()
                }

                // Other field types cannot prove standard sequence storage.
                () => return None,
            };
            Some(CollectionStorageField {
                ty,
                item_name: item,
            })
        })
        .collect()
}

// -----------------------------------------------------------------------------
// CollectionEvidence: Complete source-to-storage flow proof
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
    /// First operation storing source items.
    result: Option<CollectionEvidenceResult>,
    /// Whether filtering or truncation introduces hidden policy.
    has_policy: bool,
    /// Number of enclosing desugared `for` matches whose iterator references the source.
    source_loop_depth: usize,
}

/// Finds a reference to an iterable source binding.
struct CollectionEvidenceSourceFinder<'analysis, 'tcx> {
    /// Compiler context used to resolve local paths.
    cx: &'analysis LateContext<'tcx>,
    /// Iterable source parameter binding.
    source: HirId,
    /// Whether traversal reached the source binding.
    has_found: bool,
}

impl<'tcx> Visitor<'tcx> for CollectionEvidenceSourceFinder<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // The first source reference completes this existence query.
        if let ExprKind::Path(path) = expression.kind
            && matches!(self.cx.qpath_res(&path, expression.hir_id), Res::Local(binding) if binding == self.source)
        {
            self.has_found = true;
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

/// Proves that one source is completely consumed into target-owned storage.
struct CollectionEvidence<'analysis, 'tcx, 'storage> {
    /// Compiler context used for path and expression type resolution.
    cx: &'analysis LateContext<'tcx>,
    /// Iterable source parameter binding.
    source: HirId,
    /// Standard collection fields owned by the target.
    storage: &'storage [CollectionStorageField<'tcx>],
    /// Local wrapper whose directly owned fields qualify as storage.
    target: LocalDefId,
    /// Mutable evidence accumulated during traversal.
    state: CollectionEvidenceState,
}

impl CollectionEvidence<'_, '_, '_> {
    /// Analyzes one `candidate` body for complete source-to-storage flow.
    fn analyze<'tcx>(
        cx: &LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
        source: HirId,
        storage: &[CollectionStorageField<'tcx>],
        target: LocalDefId,
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
            target,
            state,
        };

        // Traverse once after initializing independent flow and policy state.
        evidence.visit_expr(body.value);

        // Report only complete, policy-free ingestion with a proven storage operation.
        (!evidence.state.has_policy)
            .then_some(evidence.state.result)
            .flatten()
    }
}

impl<'tcx> CollectionEvidence<'_, 'tcx, '_> {
    /// Returns whether an expression references the iterable source parameter.
    fn uses_source(&self, expression: &'tcx Expr<'tcx>) -> bool {
        let mut finder = CollectionEvidenceSourceFinder {
            cx: self.cx,
            source: self.source,
            has_found: false,
        };
        finder.visit_expr(expression);
        finder.has_found
    }

    /// Returns whether an expression is a projection or adapter rooted at the source binding.
    fn is_source_rooted(&self, expression: &'tcx Expr<'tcx>) -> bool {
        // A direct source path is the base case for source-rooted adapter analysis.
        if let ExprKind::Path(path) = expression.kind {
            return matches!(
                self.cx.qpath_res(&path, expression.hir_id),
                Res::Local(binding) if binding == self.source
            );
        }
        match expression.kind {
            ExprKind::MethodCall(_, receiver, _, _)
            | ExprKind::Field(receiver, _)
            | ExprKind::AddrOf(_, _, receiver)
            | ExprKind::Unary(_, receiver)
            | ExprKind::DropTemps(receiver) => self.is_source_rooted(receiver),
            _ => false,
        }
    }

    /// Resolves a receiver expression to one target-owned collection field.
    fn storage_item(&self, receiver: &'tcx Expr<'tcx>) -> Option<String> {
        // Storage writes must target a field projection.
        let ExprKind::Field(base, _) = receiver.kind else {
            return None;
        };
        let base_ty = self.cx.typeck_results().expr_ty_adjusted(base).peel_refs();

        // The storage field must be owned directly by the candidate wrapper.
        if collection_classification_local_adt(base_ty) != Some(self.target) {
            return None;
        }
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
        let is_policy = [
            "filter",
            "filter_map",
            "map",
            "map_while",
            "scan",
            "skip",
            "take",
            "take_while",
        ]
        .contains(&name);
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

        // The write receiver must resolve to a recognized target-owned storage field.
        let Some(item_name) = self.storage_item(receiver) else {
            return;
        };

        // Direct extension must be rooted at the iterable parameter; generated ranges and other
        // expressions that merely mention it do not make the parameter an item source.
        let consumes_source = name == "extend"
            && arguments
                .iter()
                .any(|argument| self.is_source_rooted(argument));
        let consumes_loop_item = name != "extend" && self.state.source_loop_depth > 0;

        // Storage writes must consume the source directly or an item from its active loop.
        if !consumes_source && !consumes_loop_item {
            return;
        }
        self.state.result.get_or_insert(CollectionEvidenceResult {
            span: expression.span,
            item_name,
            has_standard_trait_delegation: name == "extend",
        });
    }

    /// Records `Target { field: source.into_iter().collect() }` construction.
    fn record_collected_field(&mut self, expression: &'tcx Expr<'tcx>) {
        // Collected-field recognition starts from a struct construction expression.
        let ExprKind::Struct(_, fields, _) = expression.kind else {
            return;
        };
        let target_ty = self
            .cx
            .typeck_results()
            .expr_ty_adjusted(expression)
            .peel_refs();

        // The struct construction must produce the candidate wrapper itself.
        if collection_classification_local_adt(target_ty) != Some(self.target) {
            return;
        }
        for field in fields {
            let ExprKind::MethodCall(segment, receiver, arguments, _) = field.expr.kind else {
                continue;
            };
            if segment.ident.name.as_str() != "collect"
                || !arguments.is_empty()
                || !self.is_source_rooted(receiver)
            {
                continue;
            }
            let field_ty = self.cx.typeck_results().expr_ty(field.expr).peel_refs();
            let Some(storage) = self
                .storage
                .iter()
                .find(|storage| storage.ty.peel_refs() == field_ty)
            else {
                continue;
            };
            self.state
                .result
                .get_or_insert_with(|| CollectionEvidenceResult {
                    span: field.expr.span,
                    item_name: storage.item_name.clone(),
                    has_standard_trait_delegation: false,
                });
        }
    }
}

impl<'tcx> Visitor<'tcx> for CollectionEvidence<'_, 'tcx, '_> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        match expression.kind {
            // For-loop desugaring needs its own source-loop-depth accounting.
            ExprKind::Match(scrutinee, _, MatchSource::ForLoopDesugar) => {
                let iterates_source = self.uses_source(scrutinee);
                self.state.source_loop_depth += usize::from(iterates_source);
                intravisit::walk_expr(self, expression);
                self.state.source_loop_depth -= usize::from(iterates_source);

                // The loop traversal fully handles its nested source-depth accounting.
                return;
            }
            ExprKind::MethodCall(segment, receiver, arguments, _) => {
                let name = segment.ident.name.as_str();
                self.record_policy_adapter(name, receiver, arguments);
                self.record_storage_operation(expression, name, receiver, arguments);
            }
            ExprKind::Struct(..) => self.record_collected_field(expression),
            _ => {}
        }
        intravisit::walk_expr(self, expression);
    }
}
