extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{
    BinOpKind, Body, Expr, ExprKind, HirId, Item, ItemKind, Pat, PatKind, Stmt, StmtKind,
};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::symbol::sym;
use rustc_span::{Span, Symbol};

use super::identifier_case;

/// Exact operand count for one binary comparison relation.
const COMPARISON_PARAMETER_COUNT: usize = 2;

// -----------------------------------------------------------------------------
// Comparison: Standard relation model
// -----------------------------------------------------------------------------
/// Standard comparison contract structurally represented by a `candidate`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ComparisonContract {
    /// Boolean equality suitable for `PartialEq`.
    Equality,
    /// Optional ordering suitable for `PartialOrd`.
    PartialOrdering,
    /// Total ordering suitable for `Ord` and its equality stack.
    TotalOrdering,
}

impl ComparisonContract {
    /// Resolves one standard comparison contract from its trait path.
    fn from_trait_path(path: &str) -> Option<Self> {
        if path.ends_with("::PartialEq") {
            Some(Self::Equality)
        } else if path.ends_with("::PartialOrd") {
            Some(Self::PartialOrdering)
        } else if path.ends_with("::Ord") {
            Some(Self::TotalOrdering)
        } else {
            None
        }
    }

    /// Classifies a function return type as equality, partial ordering, or total ordering.
    fn from_output(cx: &LateContext<'_>, output: Ty<'_>) -> Option<Self> {
        // Boolean outputs directly establish an equality relation.
        if output.is_bool() {
            return Some(Self::Equality);
        }

        // A direct `Ordering` output establishes a total ordering relation.
        if is_comparison_type_ordering(cx, output) {
            return Some(Self::TotalOrdering);
        }

        // Partial ordering requires an `Option<Ordering>` return type.
        let ty::Adt(definition, arguments) = output.kind() else {
            return None;
        };
        (cx.tcx.is_diagnostic_item(sym::Option, definition.did())
            && is_comparison_type_ordering(cx, arguments.type_at(0)))
        .then_some(Self::PartialOrdering)
    }

    /// Returns the standard trait that owns this return contract.
    pub const fn trait_name(self) -> &'static str {
        match self {
            Self::Equality => "PartialEq",
            Self::PartialOrdering => "PartialOrd",
            Self::TotalOrdering => "Ord",
        }
    }

    /// Returns whether this contract belongs to the equality lint.
    const fn is_equality(self) -> bool {
        matches!(self, Self::Equality)
    }
}

/// Why a canonical-looking comparison remains reportable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonProblem {
    /// One relation exists and the corresponding trait is absent.
    MissingTrait,
    /// Several neutral APIs compete for the same canonical relation.
    AmbiguousFamily {
        /// Number of neutral APIs competing for this relation.
        count: usize,
    },
    /// A separate standard trait implementation already owns the relation.
    CompetingTrait,
}

impl ComparisonProblem {
    /// Classifies ownership for one exact comparison family.
    const fn classify(candidate_count: usize, occupancy: ComparisonFamilyOccupancy) -> Self {
        // Multiple candidates leave canonical trait ownership ambiguous.
        if candidate_count > 1 {
            return Self::AmbiguousFamily {
                count: candidate_count,
            };
        }
        if matches!(occupancy, ComparisonFamilyOccupancy::Occupied) {
            Self::CompetingTrait
        } else {
            Self::MissingTrait
        }
    }
}

// -----------------------------------------------------------------------------
// ComparisonFamilyCandidate: Complete relation context
// -----------------------------------------------------------------------------
/// One structurally proven canonical-looking comparison operation.
#[derive(Clone)]
pub struct ComparisonFamilyCandidateSource {
    /// HIR node used for lint-level configuration.
    pub hir_id: HirId,
    /// Authored function identifier.
    pub name: Symbol,
    /// Function identifier source range.
    pub name_span: Span,
}

/// Authored operand and relation evidence ranges.
#[derive(Clone)]
pub struct ComparisonFamilyCandidateEvidence {
    /// First compared operand.
    pub left: Span,
    /// Second compared operand.
    pub right: Span,
    /// Expression proving that both operands participate in the relation.
    pub relation: Span,
}

/// Inferred standard comparison contract shown in diagnostics.
#[derive(Clone)]
pub struct ComparisonFamilyCandidateProtocol {
    /// Human-readable compared type.
    pub type_name: String,
    /// Standard comparison contract selected by the return type.
    pub contract: ComparisonContract,
}

/// Private family-selection context for one comparison `candidate`.
#[derive(Clone, Copy)]
struct ComparisonFamilyCandidateSelection {
    /// Local nominal type being compared.
    type_def_id: LocalDefId,
    /// Whether the body directly delegates to the already implemented trait.
    has_standard_trait_delegation: bool,
}

// -----------------------------------------------------------------------------
// ComparisonFamily: Crate wide relation family
// -----------------------------------------------------------------------------
/// One structurally proven canonical-looking comparison operation.
#[derive(Clone)]
pub struct ComparisonFamilyCandidate {
    /// Function definition used for cross-lint precedence.
    def_id: LocalDefId,
    /// Authored function identity.
    pub source: ComparisonFamilyCandidateSource,
    /// Authored operand and relation evidence.
    pub evidence: ComparisonFamilyCandidateEvidence,
    /// Inferred standard comparison protocol.
    pub protocol: ComparisonFamilyCandidateProtocol,
    /// Private family-selection state.
    selection: ComparisonFamilyCandidateSelection,
}

/// Reportable comparison plus family and trait-occupancy classification.
pub struct ComparisonFamilyFinding<'candidate> {
    /// Candidate carrying exact source and remediation context.
    pub candidate: &'candidate ComparisonFamilyCandidate,
    /// Missing, ambiguous, or competing canonical ownership.
    pub problem: ComparisonProblem,
}

/// Exact type and return contract used to group comparison families.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct ComparisonFamilyKey {
    /// Local type whose values are compared.
    type_def_id: LocalDefId,
    /// Equality, partial ordering, or total ordering contract.
    contract: ComparisonContract,
}

/// Public lint policy selecting equality or ordering findings.
#[derive(Clone, Copy)]
pub enum ComparisonFamilySelection {
    /// Boolean equality relations.
    Equality,
    /// Partial and total ordering relations.
    Ordering,
}

/// Whether a standard trait already owns one comparison family.
#[derive(Clone, Copy)]
enum ComparisonFamilyOccupancy {
    /// No standard trait implementation owns this family.
    Available,
    /// A standard trait implementation already owns this family.
    Occupied,
}

impl ComparisonFamilyOccupancy {
    /// Converts local trait-table presence into explicit family occupancy.
    fn for_standard_trait_presence(entry: Option<&ComparisonFamilyKey>) -> Self {
        entry.map_or(Self::Available, |_| Self::Occupied)
    }

    /// Returns whether one `candidate` still deserves a diagnostic.
    const fn is_reportable(self, candidate: &ComparisonFamilyCandidate) -> bool {
        matches!(self, Self::Available) || !candidate.selection.has_standard_trait_delegation
    }
}

// -----------------------------------------------------------------------------
// ComparisonAnalysis: Crate wide relation selection
// -----------------------------------------------------------------------------
/// Finds canonical-looking local equality and ordering APIs.
#[derive(Default)]
pub struct ComparisonAnalysis {
    /// Structurally proven authored relations in traversal order.
    candidates: Vec<ComparisonFamilyCandidate>,
    /// Type and contract pairs already owned by standard traits.
    occupied: HashSet<ComparisonFamilyKey>,
}

impl ComparisonAnalysis {
    /// Records authored standard comparison implementations.
    pub fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Resolve only authored trait implementations on local nominal types.
        let ItemKind::Impl(_) = item.kind else {
            return;
        };

        // Trait occupancy is meaningful only for a resolved standard trait reference.
        let Some(trait_ref) = cx.tcx.impl_opt_trait_ref(item.owner_id.def_id) else {
            return;
        };

        // Resolve the local implementation target and standard trait identity.
        let trait_ref = trait_ref.instantiate_identity();
        let self_ty = cx.tcx.type_of(item.owner_id.def_id).instantiate_identity();

        // Only local nominal types can own a comparison trait family.
        let Some(type_def_id) = comparison_type_local_adt(self_ty) else {
            return;
        };
        let path = cx.tcx.def_path_str(trait_ref.def_id);

        // Only standard comparison traits occupy a relation family.
        let Some(contract) = ComparisonContract::from_trait_path(&path) else {
            return;
        };

        // Occupancy is tracked per exact comparison contract.
        self.occupied.insert(ComparisonFamilyKey {
            type_def_id,
            contract,
        });
    }

    /// Records one free function or inherent method when its body proves a relation.
    pub fn record_function<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        body: &'tcx Body<'tcx>,
        def_id: LocalDefId,
    ) {
        // Reject trait methods and unsupported function contracts before type analysis.
        if is_comparison_source_trait_method(cx, def_id) {
            return;
        }

        // Resolve the authored identifier and reject unsupported function kinds.
        let (name, header) = match kind {
            FnKind::ItemFn(ident, _, header) => (ident.name, header),
            FnKind::Method(ident, signature) => (ident.name, signature.header),

            // Closures do not define a named comparison API.
            FnKind::Closure => return,
        };

        // Exclude compile-time, unsafe, and non-binary APIs from relation inference.
        if header.constness == rustc_hir::Constness::Const
            || matches!(
                header.safety,
                rustc_hir::HeaderSafety::Normal(rustc_hir::Safety::Unsafe)
            )
            || body.params.len() != COMPARISON_PARAMETER_COUNT
        {
            return;
        }

        // Resolve two shared operands of the same local nominal type.
        let signature = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();

        // A comparison API requires exactly two signature inputs.
        let [left, right] = signature.inputs() else {
            return;
        };

        // Both inputs must immutably borrow the same local nominal type.
        let Some(type_def_id) = comparison_type_same_shared_local_adt(*left, *right) else {
            return;
        };

        // Resolve the exact standard return contract and reject contextual names.
        let Some(contract) = ComparisonContract::from_output(cx, signature.output()) else {
            return;
        };

        // Qualified names describe policy beyond a neutral comparison operation.
        if !has_comparison_source_neutral_name(cx, name, type_def_id, contract) {
            return;
        }

        // Floating-point fields require explicit totalization for an `Ord`-like contract.
        if contract == ComparisonContract::TotalOrdering
            && has_comparison_type_float_field(cx, type_def_id)
            && !is_comparison_source_using_total_cmp(cx, body)
        {
            return;
        }

        // Prove both operands participate in a returned comparison operation.
        let Some(left_binding) = comparison_source_binding(body.params[0].pat) else {
            return;
        };

        // The right operand likewise needs a plain binding for provenance tracking.
        let Some(right_binding) = comparison_source_binding(body.params[1].pat) else {
            return;
        };

        // Require one operation combining provenance from both operands.
        let Some(evidence) =
            RelationEvidence::analyze(cx, body, left_binding, right_binding, type_def_id)
        else {
            return;
        };

        // Preserve authored identity separately from operand-flow evidence.
        let source = ComparisonFamilyCandidateSource {
            hir_id: cx.tcx.local_def_id_to_hir_id(def_id),
            name,
            name_span: cx.tcx.def_span(def_id),
        };

        // Keep operand ranges grouped as the proof attached to that authored function.
        let evidence_spans = ComparisonFamilyCandidateEvidence {
            left: body.params[0].span,
            right: body.params[1].span,
            relation: evidence.span,
        };

        // Retain the inferred trait contract and private family selection state.
        let protocol = ComparisonFamilyCandidateProtocol {
            type_name: cx.tcx.def_path_str(type_def_id.to_def_id()),
            contract,
        };

        // Keep private family identity and delegation state out of diagnostic context.
        let selection = ComparisonFamilyCandidateSelection {
            type_def_id,
            has_standard_trait_delegation: evidence.has_standard_trait_delegation,
        };

        // Store the compact contexts as one crate-wide comparison candidate.
        self.candidates.push(ComparisonFamilyCandidate {
            def_id,
            source,
            evidence: evidence_spans,
            protocol,
            selection,
        });
    }

    /// Returns stable findings for one of the two public lint policies.
    pub fn findings(
        &self,
        selection: ComparisonFamilySelection,
    ) -> Vec<ComparisonFamilyFinding<'_>> {
        let wants_equality = matches!(selection, ComparisonFamilySelection::Equality);
        let mut families = HashMap::<ComparisonFamilyKey, Vec<&ComparisonFamilyCandidate>>::new();
        for candidate in self
            .candidates
            .iter()
            .filter(|candidate| candidate.protocol.contract.is_equality() == wants_equality)
        {
            families
                .entry(ComparisonFamilyKey {
                    type_def_id: candidate.selection.type_def_id,
                    contract: candidate.protocol.contract,
                })
                .or_default()
                .push(candidate);
        }

        // Classify each family once before emitting its individual candidate findings.
        let mut findings = Vec::new();
        for (key, family) in families {
            let occupancy =
                ComparisonFamilyOccupancy::for_standard_trait_presence(self.occupied.get(&key));
            let problem = ComparisonProblem::classify(family.len(), occupancy);

            // Suppress only direct delegation into an already occupied standard trait.
            let reportable = family
                .into_iter()
                .filter(|candidate| occupancy.is_reportable(candidate));
            for candidate in reportable {
                findings.push(ComparisonFamilyFinding { candidate, problem });
            }
        }
        findings.sort_unstable_by_key(|finding| finding.candidate.source.name_span.lo());
        findings
    }

    /// Returns definitions claimed by a specialized comparison diagnostic.
    pub fn reportable_definitions(&self) -> HashSet<LocalDefId> {
        let equality = self.findings(ComparisonFamilySelection::Equality);
        let ordering = self.findings(ComparisonFamilySelection::Ordering);
        let mut definitions = HashSet::new();
        for finding in equality.into_iter().chain(ordering) {
            definitions.insert(finding.candidate.def_id);
        }
        definitions
    }
}

// -----------------------------------------------------------------------------
// ComparisonType: Resolved relation types
// -----------------------------------------------------------------------------

/// Resolves a possibly referenced type to one local nominal definition.
fn comparison_type_local_adt(ty: Ty<'_>) -> Option<LocalDefId> {
    // Only nominal types can provide a local comparison family identity.
    let ty::Adt(definition, _) = ty.peel_refs().kind() else {
        return None;
    };
    definition.did().as_local()
}

/// Resolves two shared references to the same local nominal type.
fn comparison_type_same_shared_local_adt<'tcx>(
    left: Ty<'tcx>,
    right: Ty<'tcx>,
) -> Option<LocalDefId> {
    // Require two immutable references before comparing their pointee identity.
    let (
        ty::Ref(_, left, rustc_hir::Mutability::Not),
        ty::Ref(_, right, rustc_hir::Mutability::Not),
    ) = (left.kind(), right.kind())
    else {
        return None;
    };

    // Peel the common reference only after mutability and type identity agree.
    (left == right)
        .then(|| comparison_type_local_adt(*left))
        .flatten()
}

/// Returns whether a type is exactly `std::cmp::Ordering`.
fn is_comparison_type_ordering(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    // Only nominal types can resolve to the standard `Ordering` definition.
    let ty::Adt(definition, _) = ty.kind() else {
        return false;
    };
    cx.tcx.def_path_str(definition.did()) == "std::cmp::Ordering"
}

/// Returns whether a local type directly stores a floating-point value.
fn has_comparison_type_float_field(cx: &LateContext<'_>, type_def_id: LocalDefId) -> bool {
    cx.tcx
        .adt_def(type_def_id.to_def_id())
        .all_fields()
        .any(|field| {
            cx.tcx
                .type_of(field.did)
                .instantiate_identity()
                .is_floating_point()
        })
}

// -----------------------------------------------------------------------------
// ComparisonVocabulary: Standard relation syntax
// -----------------------------------------------------------------------------

/// Vocabulary that adds no policy beyond the standard comparison relation.
const COMPARISON_VOCABULARY_NEUTRAL_NAMES: &[&str] = &[
    "Are",
    "Compare",
    "Comparison",
    "Cmp",
    "Different",
    "Equal",
    "Equals",
    "Eq",
    "Is",
    "Not",
    "Partial",
    "Same",
    "Unequal",
];

/// Built-in operators capable of proving a binary relation.
const COMPARISON_VOCABULARY_BINARY_OPERATORS: &[BinOpKind] = &[
    BinOpKind::Eq,
    BinOpKind::Ne,
    BinOpKind::Lt,
    BinOpKind::Le,
    BinOpKind::Gt,
    BinOpKind::Ge,
];

/// Standard-looking methods capable of proving a binary relation.
const COMPARISON_VOCABULARY_RELATION_METHODS: &[&str] =
    &["cmp", "partial_cmp", "eq", "ne", "total_cmp"];

// -----------------------------------------------------------------------------
// TotalCmpFinder: Floating-point totalization discovery
// -----------------------------------------------------------------------------

/// Finds a resolved call to the standard floating-point total comparator.
struct TotalCmpFinder<'analysis, 'tcx> {
    /// Compiler context used to resolve method definitions.
    cx: &'analysis LateContext<'tcx>,
    /// Whether traversal reached `total_cmp`.
    has_found: bool,
    /// Comparison operations contributing to the function result.
    returned_relations: HashSet<HirId>,
}

impl TotalCmpFinder<'_, '_> {
    /// Returns whether a method call resolves to the standard float comparator.
    fn is_total_cmp(&self, expression: &Expr<'_>) -> bool {
        // Calls without a resolved method definition cannot prove standard delegation.
        let Some(definition) = self
            .cx
            .typeck_results()
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };
        self.cx.tcx.item_name(definition).as_str() == "total_cmp"
    }
}

impl<'tcx> Visitor<'tcx> for TotalCmpFinder<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if self.returned_relations.contains(&expression.hir_id)
            && matches!(expression.kind, ExprKind::MethodCall(..))
            && self.is_total_cmp(expression)
        {
            self.has_found = true;
        }

        // Nested closures return from a different callable.
        if matches!(expression.kind, ExprKind::Closure(_)) {
            return;
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

// -----------------------------------------------------------------------------
// ComparisonSource: Authored relation evidence
// -----------------------------------------------------------------------------

/// Returns whether an associated function belongs to a trait implementation.
fn is_comparison_source_trait_method(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
    cx.tcx
        .opt_local_parent(def_id)
        .is_some_and(|parent| matches!(cx.tcx.def_kind(parent), DefKind::Impl { of_trait: true }))
}

/// Returns whether authored vocabulary claims one unqualified comparison relation.
fn has_comparison_source_neutral_name(
    cx: &LateContext<'_>,
    name: Symbol,
    type_def_id: LocalDefId,
    contract: ComparisonContract,
) -> bool {
    // Seed vocabulary with the compared type's own semantic words.
    let mut neutral = identifier_case::words(cx.tcx.item_name(type_def_id.to_def_id()).as_str());

    // Add only vocabulary naming the standard relation rather than domain policy.
    neutral.extend(
        COMPARISON_VOCABULARY_NEUTRAL_NAMES
            .iter()
            .map(ToString::to_string),
    );

    // Plurals and the ordering noun remain aliases of the same contract.
    identifier_case::words(name.as_str())
        .into_iter()
        .all(|word| {
            neutral.contains(&word)
                || neutral.iter().any(|known| word == format!("{known}s"))
                || (contract != ComparisonContract::Equality && word == "Order")
        })
}

/// Returns whether a body explicitly totalizes floating-point ordering.
fn is_comparison_source_using_total_cmp<'tcx>(cx: &LateContext<'tcx>, body: &Body<'tcx>) -> bool {
    // Traverse the complete body because totalization may occur behind a local binding.
    let mut finder = TotalCmpFinder {
        cx,
        has_found: false,
        returned_relations: RelationResultCollector::collect(body.value),
    };
    finder.visit_expr(body.value);
    finder.has_found
}

/// Extracts one plain parameter binding for provenance analysis.
const fn comparison_source_binding(pattern: &Pat<'_>) -> Option<HirId> {
    // Provenance analysis needs an unmodified parameter binding.
    let PatKind::Binding(_, binding, _, None) = pattern.kind else {
        return None;
    };
    Some(binding)
}

// -----------------------------------------------------------------------------
// RelationEvidence: Operand flow proof
// -----------------------------------------------------------------------------

/// First operation proving interaction between both comparison operands.
struct RelationEvidence {
    /// Expression where both operand families interact.
    span: Span,
    /// Whether that expression directly delegates to a standard comparison trait.
    has_standard_trait_delegation: bool,
}

impl RelationEvidence {
    /// Proves that both parameter families reach one comparison operation.
    fn analyze<'tcx>(
        cx: &LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
        left: HirId,
        right: HirId,
        type_def_id: LocalDefId,
    ) -> Option<Self> {
        // Seed independent provenance sets from the two authored operands.
        let left = HashSet::from([left]);
        let right = HashSet::from([right]);

        // Traverse once while retaining independent aliases for both operands.
        let mut analyzer = RelationEvidenceAnalyzer {
            cx,
            left,
            right,
            returned_relations: RelationResultCollector::collect(body.value),
            type_def_id,
            evidence: None,
        };

        // Return the first proven relation discovered during the single traversal.
        analyzer.visit_expr(body.value);
        analyzer.evidence
    }
}

/// Tracks local aliases derived from each side of a binary relation.
struct RelationEvidenceAnalyzer<'analysis, 'tcx> {
    /// Compiler context used to resolve local paths and methods.
    cx: &'analysis LateContext<'tcx>,
    /// Bindings derived from the first operand.
    left: HashSet<HirId>,
    /// Bindings derived from the second operand.
    right: HashSet<HirId>,
    /// Relation expressions contributing to a tail value or explicit return.
    returned_relations: HashSet<HirId>,
    /// Local type whose occupied standard contract may be delegated to.
    type_def_id: LocalDefId,
    /// First expression proving interaction between both sides.
    evidence: Option<RelationEvidence>,
}

impl<'tcx> RelationEvidenceAnalyzer<'_, 'tcx> {
    /// Returns whether an expression references one provenance family.
    fn is_using_bindings(&self, expression: &'tcx Expr<'tcx>, bindings: &HashSet<HirId>) -> bool {
        let mut finder = ComparisonBindingFinder {
            cx: self.cx,
            bindings,
            has_found: false,
        };

        // Traverse aliases and projections before returning provenance membership.
        finder.visit_expr(expression);
        finder.has_found
    }

    /// Returns whether the expressions derive from opposite operands.
    fn has_opposite_operands(&self, left: &'tcx Expr<'tcx>, right: &'tcx Expr<'tcx>) -> bool {
        (self.is_using_bindings(left, &self.left) && self.is_using_bindings(right, &self.right))
            || (self.is_using_bindings(left, &self.right)
                && self.is_using_bindings(right, &self.left))
    }

    /// Returns whether an operation resolves into the standard comparison trait stack.
    fn is_delegating_to_standard_trait(
        &self,
        expression: &Expr<'_>,
        left: &Expr<'_>,
        right: &Expr<'_>,
    ) -> bool {
        let owns_operand = |operand: &Expr<'_>| {
            comparison_type_local_adt(self.cx.typeck_results().expr_ty(operand))
                == Some(self.type_def_id)
        };

        // Both operands must retain the candidate's local nominal type.
        if !owns_operand(left) || !owns_operand(right) {
            return false;
        }
        let typeck = self.cx.typeck_results();

        // A resolved method definition is required to identify trait delegation.
        let Some(definition) = typeck.type_dependent_def_id(expression.hir_id) else {
            return false;
        };

        // Match the resolved definition against the complete standard trait stack.
        let path = self.cx.tcx.def_path_str(definition);
        ["::PartialEq::", "::PartialOrd::", "::Ord::"]
            .iter()
            .any(|marker| path.contains(marker))
    }

    /// Retains the first relation proven to combine both operand families.
    fn record_relation(
        &mut self,
        expression: &'tcx Expr<'tcx>,
        left: &'tcx Expr<'tcx>,
        right: &'tcx Expr<'tcx>,
    ) {
        // Only the first returned operation that combines both operand families is evidence.
        if self.evidence.is_some()
            || !self.returned_relations.contains(&expression.hir_id)
            || !self.has_opposite_operands(left, right)
        {
            return;
        }
        let has_standard_trait_delegation =
            self.is_delegating_to_standard_trait(expression, left, right);
        self.evidence = Some(RelationEvidence {
            span: expression.span,
            has_standard_trait_delegation,
        });
    }

    /// Propagates operand provenance through one plain local binding.
    fn record_alias(&mut self, initializer: Option<&'tcx Expr<'tcx>>, pattern: &Pat<'_>) {
        // Bindings without an initializer cannot inherit operand provenance.
        let Some(initializer) = initializer else {
            return;
        };

        // Only plain bindings can retain a stable provenance identity.
        let PatKind::Binding(_, binding, _, None) = pattern.kind else {
            return;
        };

        // Preserve membership in either independent operand family.
        if self.is_using_bindings(initializer, &self.left) {
            self.left.insert(binding);
        }

        // Absence from the right family ends propagation for this alias.
        if !self.is_using_bindings(initializer, &self.right) {
            return;
        }
        self.right.insert(binding);
    }
}

impl<'tcx> Visitor<'tcx> for RelationEvidenceAnalyzer<'_, 'tcx> {
    fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
        if let StmtKind::Let(local) = statement.kind {
            self.record_alias(local.init, local.pat);
        }
        intravisit::walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Record built-in binary relations combining both operand families.
        if let ExprKind::Binary(operator, left, right) = expression.kind
            && COMPARISON_VOCABULARY_BINARY_OPERATORS.contains(&operator.node)
        {
            self.record_relation(expression, left, right);
        }

        // Record standard-looking method relations with one opposite operand.
        if let ExprKind::MethodCall(segment, receiver, arguments, _) = expression.kind
            && let [argument] = arguments
            && COMPARISON_VOCABULARY_RELATION_METHODS.contains(&segment.ident.name.as_str())
        {
            self.record_relation(expression, receiver, argument);
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// ComparisonBindingFinder: Operand provenance membership query
// -----------------------------------------------------------------------------

/// Finds a reference to any binding in one comparison provenance family.
struct ComparisonBindingFinder<'set, 'analysis, 'tcx> {
    /// Compiler context used to resolve local paths.
    cx: &'analysis LateContext<'tcx>,
    /// Local bindings belonging to the queried provenance family.
    bindings: &'set HashSet<HirId>,
    /// Whether traversal reached a binding in the queried family.
    has_found: bool,
}

impl<'tcx> Visitor<'tcx> for ComparisonBindingFinder<'_, '_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // The first matching binding completes this existence query.
        if let ExprKind::Path(path) = expression.kind
            && let Res::Local(binding) = self.cx.qpath_res(&path, expression.hir_id)
            && self.bindings.contains(&binding)
        {
            self.has_found = true;
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// RelationResultCollector: Returned relation ownership
// -----------------------------------------------------------------------------

/// Finds comparison expressions that contribute to the function's boolean result.
#[derive(Default)]
struct RelationResultCollector {
    /// Relation expressions reached through tail values and explicit returns.
    relations: HashSet<HirId>,
    /// Comparison operations carried by each local binding.
    bindings: HashMap<HirId, HashSet<HirId>>,
}

impl RelationResultCollector {
    /// Returns whether an expression is comparison syntax recognized by the analyzer.
    fn is_relation(expression: &Expr<'_>) -> bool {
        match expression.kind {
            ExprKind::Binary(operator, _, _) => {
                COMPARISON_VOCABULARY_BINARY_OPERATORS.contains(&operator.node)
            }
            ExprKind::MethodCall(segment, _, arguments, _) => {
                arguments.len() == 1
                    && COMPARISON_VOCABULARY_RELATION_METHODS.contains(&segment.ident.name.as_str())
            }
            _ => false,
        }
    }

    /// Finds relations contained in an expression, including through known aliases.
    fn relations_in<'tcx>(&self, expression: &'tcx Expr<'tcx>) -> HashSet<HirId> {
        let mut finder = RelationFinder {
            bindings: &self.bindings,
            relations: HashSet::new(),
        };
        finder.visit_expr(expression);
        finder.relations
    }

    /// Follows only expression positions whose value contributes to the returned boolean.
    fn visit_result_expr<'tcx>(&mut self, expression: &'tcx Expr<'tcx>) {
        // A bound relation contributes all of its recorded relation expressions.
        if let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = expression.kind
            && let Res::Local(binding) = path.res
            && let Some(relations) = self.bindings.get(&binding)
        {
            self.relations.extend(relations);
            return;
        }

        // A direct relation expression is a complete result-flow leaf.
        if Self::is_relation(expression) {
            self.relations.insert(expression.hir_id);
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
            ExprKind::Binary(_, left, right) => {
                self.visit_result_expr(left);
                self.visit_result_expr(right);
            }
            ExprKind::Unary(_, value) | ExprKind::DropTemps(value) | ExprKind::Ret(Some(value)) => {
                self.visit_result_expr(value);
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
            _ => self.visit_expr(expression),
        }
    }

    /// Collects returned relation expressions from one function body value.
    fn collect(expression: &Expr<'_>) -> HashSet<HirId> {
        let mut collector = Self::default();
        collector.visit_result_expr(expression);
        collector.relations
    }
}

impl<'tcx> Visitor<'tcx> for RelationResultCollector {
    fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
        if let StmtKind::Let(local) = statement.kind
            && let Some(initializer) = local.init
            && let PatKind::Binding(_, binding, _, None) = local.pat.kind
        {
            let relations = self.relations_in(initializer);
            if !relations.is_empty() {
                self.bindings.insert(binding, relations);
            }
        }
        intravisit::walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Assignment handling updates relation bindings before ending this traversal branch.
        if let ExprKind::Assign(left, right, _) = expression.kind {
            let relations = self.relations_in(right);
            self.visit_expr(right);
            if let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = left.kind
                && let Res::Local(binding) = path.res
            {
                if relations.is_empty() {
                    self.bindings.remove(&binding);
                } else {
                    self.bindings.insert(binding, relations);
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

// -----------------------------------------------------------------------------
// RelationFinder: Direct and aliased returned-relation discovery
// -----------------------------------------------------------------------------

/// Finds direct and aliased relation expressions in one result subtree.
struct RelationFinder<'set> {
    /// Comparison operations carried by each local binding.
    bindings: &'set HashMap<HirId, HashSet<HirId>>,
    /// Relation expressions discovered in the result subtree.
    relations: HashSet<HirId>,
}

impl<'tcx> Visitor<'tcx> for RelationFinder<'_> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Direct comparison syntax completes this traversal branch.
        if RelationResultCollector::is_relation(expression) {
            self.relations.insert(expression.hir_id);
            return;
        }

        // Known aliases contribute their previously recorded relation expressions.
        if let ExprKind::Path(rustc_hir::QPath::Resolved(_, path)) = expression.kind
            && let Res::Local(binding) = path.res
            && let Some(relations) = self.bindings.get(&binding)
        {
            self.relations.extend(relations);
            return;
        }

        // Nested closures return from a different callable.
        if matches!(expression.kind, ExprKind::Closure(_)) {
            return;
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}
