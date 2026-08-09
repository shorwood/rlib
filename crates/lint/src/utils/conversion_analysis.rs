extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, FnKind, Visitor};
use rustc_hir::{
    Body, Expr, ExprKind, HirId, Item, ItemKind, Mutability, Pat, PatKind, Stmt, StmtKind,
};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::symbol::sym;
use rustc_span::{Span, Symbol};

use super::construction_analysis::{ConstructionCandidate, ConstructionReturn};
use super::identifier_case;

// -----------------------------------------------------------------------------
// ConversionType: Lifetime erased semantic identity
// -----------------------------------------------------------------------------

/// Concrete source or target identity used to group equivalent conversion APIs.
#[derive(Clone, PartialEq, Eq, Hash)]
enum ConversionType {
    /// Nominal type with every concrete type and constant argument retained.
    Adt {
        /// Compiler identity of the nominal definition.
        definition: rustc_hir::def_id::DefId,
        /// Lifetime-erased concrete arguments in declaration order.
        arguments: Vec<ConversionTypeArgument>,
    },
    /// Built-in scalar or string type identified by its compiler spelling.
    Primitive {
        /// Stable compiler display name used for equality within one lint run.
        name: String,
    },
    /// Ordinary shared or mutable reference with its lifetime erased.
    Reference {
        /// Borrow mutability that remains semantically significant.
        mutability: Mutability,
        /// Referenced concrete type.
        referent: Box<Self>,
    },
    /// Dynamically sized slice of one concrete element type.
    Slice {
        /// Concrete slice element type.
        element: Box<Self>,
    },
    /// Fixed-length array with a resolved element and length identity.
    Array {
        /// Concrete array element type.
        element: Box<Self>,
        /// Compiler identity of the concrete array length.
        length: String,
    },
    /// Product type whose concrete components retain their order.
    Tuple {
        /// Concrete tuple elements in positional order.
        elements: Vec<Self>,
    },
}

/// Concrete generic argument retained in a nominal conversion type.
#[derive(Clone, PartialEq, Eq, Hash)]
enum ConversionTypeArgument {
    /// One concrete type argument.
    Type {
        /// Resolved lifetime-erased type identity.
        ty: ConversionType,
    },
    /// One concrete constant argument.
    Const {
        /// Compiler identity of the resolved constant.
        value: String,
    },
}

impl ConversionType {
    /// Converts one nominal type and its concrete arguments.
    fn from_adt(definition: ty::AdtDef<'_>, arguments: ty::GenericArgsRef<'_>) -> Option<Self> {
        Some(Self::Adt {
            definition: definition.did(),
            arguments: Self::adt_arguments(arguments)?,
        })
    }

    /// Preserves a reference while erasing only its lifetime.
    fn from_reference(inner: Ty<'_>, mutability: Mutability) -> Option<Self> {
        Some(Self::Reference {
            mutability,
            referent: Box::new(Self::from_ty(inner)?),
        })
    }

    /// Converts slice, array, and tuple shapes delegated by the primary classifier.
    fn from_sequence(ty: Ty<'_>) -> Option<Self> {
        match ty.kind() {
            ty::Slice(inner) => Self::from_slice(*inner),
            ty::Array(inner, length) => Self::from_array(*inner, *length),
            ty::Tuple(elements) => Self::from_tuple(elements),
            _ => None,
        }
    }

    /// Converts one concrete slice element.
    fn from_slice(inner: Ty<'_>) -> Option<Self> {
        Some(Self::Slice {
            element: Box::new(Self::from_ty(inner)?),
        })
    }

    /// Converts one concrete array element and length.
    fn from_array(inner: Ty<'_>, length: ty::Const<'_>) -> Option<Self> {
        Some(Self::Array {
            element: Box::new(Self::from_ty(inner)?),
            length: format!("{length:?}"),
        })
    }

    /// Converts every concrete tuple element while preserving position.
    fn from_tuple(elements: &ty::List<Ty<'_>>) -> Option<Self> {
        let elements = elements
            .iter()
            .map(Self::from_ty)
            .collect::<Option<Vec<_>>>()?;
        Some(Self::Tuple { elements })
    }

    /// Builds a stable family key while deliberately erasing lifetimes.
    fn from_ty(ty: Ty<'_>) -> Option<Self> {
        match ty.kind() {
            ty::Adt(definition, arguments) => Self::from_adt(*definition, arguments),
            ty::Bool | ty::Char | ty::Int(_) | ty::Uint(_) | ty::Float(_) | ty::Str => {
                Some(Self::Primitive {
                    name: ty.to_string(),
                })
            }
            ty::Ref(_, inner, mutability) => Self::from_reference(*inner, *mutability),
            _ => Self::from_sequence(ty),
        }
    }

    /// Converts nominal generic arguments while deliberately dropping lifetimes.
    fn adt_arguments(arguments: ty::GenericArgsRef<'_>) -> Option<Vec<ConversionTypeArgument>> {
        let mut converted = Vec::new();
        for argument in arguments {
            if let Some(ty) = argument.as_type() {
                converted.push(ConversionTypeArgument::Type {
                    ty: Self::from_ty(ty)?,
                });
                continue;
            }
            let Some(constant) = argument.as_const() else {
                continue;
            };
            converted.push(ConversionTypeArgument::Const {
                value: Self::concrete_const_identity(constant)?,
            });
        }
        Some(converted)
    }

    /// Retains only resolved constant identities suitable for an exact family key.
    fn concrete_const_identity(constant: ty::Const<'_>) -> Option<String> {
        // Identify constants whose value is supplied by a generic binder.
        let is_generic = matches!(
            constant.kind(),
            ty::ConstKind::Param(_) | ty::ConstKind::Placeholder(_) | ty::ConstKind::Bound(..)
        );

        // Identify constants that type checking has not resolved to a value.
        let is_unresolved = matches!(
            constant.kind(),
            ty::ConstKind::Infer(_) | ty::ConstKind::Error(_)
        );

        // Retain only identities independent of unresolved generic context.
        (!is_generic && !is_unresolved).then(|| format!("{constant:?}"))
    }

    /// Returns whether this is exactly an immutable string slice.
    fn is_str_slice(&self) -> bool {
        matches!(
            self,
            Self::Reference { mutability: Mutability::Not, referent }
                if matches!(referent.as_ref(), Self::Primitive { name } if name == "str")
        )
    }
}

// -----------------------------------------------------------------------------
// Conversion: Report and ownership context
// -----------------------------------------------------------------------------

/// Exact semantic source and target pair used for ambiguity and trait checks.
#[derive(Clone, PartialEq, Eq, Hash)]
struct ConversionPair {
    /// Concrete input family offered by the conversion API.
    source: ConversionType,
    /// Concrete result family owned by the conversion API.
    target: ConversionType,
}

/// Standard conversion contract implied by a candidate's exact return shape.
#[derive(Clone)]
pub enum ConversionContract {
    /// Direct result suitable for `From` and reciprocal `Into`.
    Infallible,
    /// Exact `Result` contract suitable for `TryFrom` and reciprocal `TryInto`.
    Fallible {
        /// Concrete error type retained for remediation guidance.
        error: String,
    },
}

impl ConversionContract {
    /// Names the standard trait appropriate for this conversion contract.
    pub const fn trait_name(&self) -> &'static str {
        match self {
            Self::Infallible => "From",
            Self::Fallible { .. } => "TryFrom",
        }
    }
}

/// Resolved target and standard contract extracted from one return type.
struct ConversionReturn<'tcx> {
    /// Exact target type returned directly or through `Result` success.
    target: Ty<'tcx>,
    /// Standard conversion trait selected by the outer return shape.
    contract: ConversionContract,
}

impl<'tcx> ConversionReturn<'tcx> {
    /// Creates the direct infallible return contract.
    const fn infallible(target: Ty<'tcx>) -> Self {
        Self {
            target,
            contract: ConversionContract::Infallible,
        }
    }

    /// Extracts an exact standard `Result<Target, Error>` contract.
    fn fallible(cx: &LateContext<'tcx>, output: Ty<'tcx>) -> Option<Self> {
        let ty::Adt(definition, arguments) = output.kind() else {
            return None;
        };
        cx.tcx
            .is_diagnostic_item(sym::Result, definition.did())
            .then(|| Self {
                target: arguments.type_at(0),
                contract: ConversionContract::Fallible {
                    error: arguments.type_at(1).to_string(),
                },
            })
    }
}

/// How directly the authored name claims to be a general conversion.
#[derive(Clone, Copy)]
pub enum ConversionConfidence {
    /// Authored name explicitly claims a general source-to-target conversion.
    Conventional,
    /// Structure implies conversion ownership while the name remains domain-specific.
    Structural,
}

impl ConversionConfidence {
    /// Classifies authored words against neutral conversion and type vocabulary.
    fn for_conversion_vocabulary(authored: &[String], semantic: &HashSet<String>) -> Self {
        /// Neutral words that explicitly claim general conversion semantics.
        const CONVERSION: &[&str] = &["Conversion", "Convert", "From", "Into", "To", "Try"];
        let has_marker = Self::has_conversion_marker(authored, CONVERSION);
        let has_only_neutral_words = Self::has_only_neutral_words(authored, semantic, CONVERSION);
        match (has_marker, has_only_neutral_words) {
            (true, true) => Self::Conventional,
            _ => Self::Structural,
        }
    }

    /// Returns whether authored words explicitly claim conversion semantics.
    fn has_conversion_marker(authored: &[String], conversion: &[&str]) -> bool {
        authored
            .iter()
            .any(|word| conversion.contains(&word.as_str()))
    }

    /// Returns whether every authored word is neutral conversion or type vocabulary.
    fn has_only_neutral_words(
        authored: &[String],
        semantic: &HashSet<String>,
        conversion: &[&str],
    ) -> bool {
        authored
            .iter()
            .all(|word| conversion.contains(&word.as_str()) || semantic.contains(word))
    }
}

// -----------------------------------------------------------------------------
// ConversionCandidate: Diagnostic record
// -----------------------------------------------------------------------------

/// Authored function identity and source ranges for one conversion candidate.
#[derive(Clone)]
pub struct ConversionCandidateIdentity {
    /// Function definition used for cross-lint precedence.
    pub def_id: LocalDefId,
    /// Function HIR node used to anchor lint levels.
    pub hir_id: HirId,
    /// Authored function identifier.
    pub name: Symbol,
    /// Function identifier source range.
    pub name_span: Span,
    /// Sole source parameter range.
    pub source_span: Span,
}

impl ConversionCandidateIdentity {
    /// Captures authored identity and source ranges from construction discovery.
    fn from_construction(
        cx: &LateContext<'_>,
        body: &Body<'_>,
        candidate: &ConstructionCandidate,
    ) -> Self {
        Self {
            def_id: candidate.function.def_id,
            hir_id: cx.tcx.local_def_id_to_hir_id(candidate.function.def_id),
            name: candidate.function.name,
            name_span: candidate.function.name_span,
            source_span: body.params[0].span,
        }
    }
}

/// Concrete type and trait context retained for one conversion diagnostic.
#[derive(Clone)]
pub struct ConversionCandidateSemantics {
    /// Concrete source type shown in diagnostics.
    pub source: String,
    /// Concrete target type shown in diagnostics.
    pub target: String,
    /// Standard infallible or fallible contract.
    pub contract: ConversionContract,
    /// Strength of the authored conversion claim.
    pub confidence: ConversionConfidence,
}

impl ConversionCandidateSemantics {
    /// Captures concrete display and standard-contract context.
    const fn new(
        source: String,
        target: String,
        contract: ConversionContract,
        confidence: ConversionConfidence,
    ) -> Self {
        Self {
            source,
            target,
            contract,
            confidence,
        }
    }
}

/// One unique, effect-free conversion that should use a standard trait.
#[derive(Clone)]
pub struct ConversionCandidate {
    /// Function identity and authored source ranges.
    pub identity: ConversionCandidateIdentity,
    /// Concrete source, target, contract, and confidence context.
    pub semantics: ConversionCandidateSemantics,
    /// Exact semantic pair used for ambiguity and trait suppression.
    pair: ConversionPair,
    /// Whether effects, policy, and parser precedence permit reporting.
    is_reportable: bool,
}

// -----------------------------------------------------------------------------
// ConversionAnalysis: Crate wide family selection
// -----------------------------------------------------------------------------

/// Discovers conversion ownership independently from individual lint passes.
#[derive(Default)]
pub struct ConversionAnalysis {
    /// Structurally proven one-source conversions in traversal order.
    candidates: Vec<ConversionCandidate>,
    /// Exact pairs already governed by `From` or `TryFrom`.
    occupied_pairs: HashSet<ConversionPair>,
    /// Functions whose sole source reaches a distinct constructed target.
    target_owned_definitions: HashSet<LocalDefId>,
}

impl ConversionAnalysis {
    /// Extracts only direct and exact `Result<T, E>` conversion contracts.
    fn return_contract<'tcx>(
        cx: &LateContext<'tcx>,
        output: Ty<'tcx>,
        candidate: &ConstructionCandidate,
    ) -> Option<ConversionReturn<'tcx>> {
        // Map the construction shape to its precise standard conversion contract.
        match candidate.target.return_shape {
            ConstructionReturn::Direct => Some(ConversionReturn::infallible(output)),
            ConstructionReturn::FallibleDirect => ConversionReturn::fallible(cx, output),
            ConstructionReturn::Contained => None,
        }
    }

    /// Rejects names that explicitly advertise policy, context, or effects.
    fn has_hard_name_exclusion(name: &str) -> bool {
        /// Vocabulary that signals a noncanonical policy, context, or effect.
        const EXCLUDED: &[&str] = &[
            "Canonicalize",
            "Canonicalized",
            "Ceil",
            "Clamp",
            "Clamped",
            "Fetch",
            "Filter",
            "Filtered",
            "First",
            "Floor",
            "For",
            "Last",
            "Load",
            "Lookup",
            "Lossy",
            "Missing",
            "Normalize",
            "Normalized",
            "Open",
            "Partial",
            "Prefix",
            "Query",
            "Read",
            "Resolve",
            "Round",
            "Rounded",
            "Saturating",
            "Save",
            "Select",
            "Selected",
            "Store",
            "Truncate",
            "Truncated",
            "Unchecked",
            "Using",
            "With",
            "Wrapping",
            "Write",
        ];
        identifier_case::words(name)
            .iter()
            .any(|word| EXCLUDED.contains(&word.as_str()))
    }

    /// Classifies names as explicitly conventional or structurally suspicious.
    fn confidence(name: &str, source: Ty<'_>, target: Ty<'_>) -> ConversionConfidence {
        // Extract authored and semantic vocabulary independently.
        let authored = identifier_case::words(name);
        let semantic = format!("{source} {target}")
            .split(|character: char| !character.is_alphanumeric() && character != '_')
            .flat_map(identifier_case::words)
            .collect::<HashSet<_>>();

        // Delegate the binary classification to its owning enum.
        ConversionConfidence::for_conversion_vocabulary(&authored, &semantic)
    }

    /// Records an existing standard conversion implementation for pair suppression.
    pub fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Restrict pair occupancy to authored standard conversion implementations.
        let ItemKind::Impl(_) = item.kind else {
            return;
        };
        let def_id = item.owner_id.def_id;
        let Some(trait_ref) = cx.tcx.impl_opt_trait_ref(def_id) else {
            return;
        };
        let trait_ref = trait_ref.instantiate_identity();

        // Accept either standard conversion trait as occupancy for the pair.
        if !cx.tcx.is_diagnostic_item(sym::From, trait_ref.def_id)
            && !cx.tcx.is_diagnostic_item(sym::TryFrom, trait_ref.def_id)
        {
            return;
        }

        // Resolve both sides into the same lifetime-erased family identity.
        let target_ty = cx.tcx.type_of(def_id).instantiate_identity();
        let Some(target) = ConversionType::from_ty(target_ty) else {
            return;
        };
        let Some(source_ty) = trait_ref.args.types().nth(1) else {
            return;
        };
        let Some(source) = ConversionType::from_ty(source_ty) else {
            return;
        };

        // Occupy the pair for both standard recommendations.
        self.occupied_pairs
            .insert(ConversionPair { source, target });
    }

    /// Records a one-source function when its source reaches target construction.
    pub fn record_function<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        body: &'tcx Body<'tcx>,
        candidate: &ConstructionCandidate,
    ) {
        // Reject closures and function contracts outside ordinary safe runtime Rust.
        let header = match kind {
            FnKind::ItemFn(_, _, header) => header,
            FnKind::Method(_, signature) => signature.header,
            FnKind::Closure => return,
        };

        // Exclude unsafe and compile-time contracts from automatic trait guidance.
        if matches!(
            header.safety,
            rustc_hir::HeaderSafety::Normal(rustc_hir::Safety::Unsafe)
        ) || matches!(header.constness, rustc_hir::Constness::Const)
        {
            return;
        }

        // Reject unresolved type and constant parameters while permitting erased lifetimes.
        let generics = cx.tcx.generics_of(candidate.function.def_id);
        let has_unresolved_family_parameter = generics
            .own_params
            .iter()
            .any(|parameter| !matches!(parameter.kind, ty::GenericParamDefKind::Lifetime));
        if has_unresolved_family_parameter {
            return;
        }

        // Require exactly one semantic source and one authored source pattern.
        let signature = cx
            .tcx
            .fn_sig(candidate.function.def_id)
            .instantiate_identity()
            .skip_binder();
        if signature.inputs().len() != 1 || body.params.len() != 1 {
            return;
        }

        // Resolve concrete distinct source and target family identities.
        let source_ty = signature.inputs()[0];
        let Some(source) = ConversionType::from_ty(source_ty) else {
            return;
        };

        // Resolve the exact direct or fallible target contract.
        let Some(return_) = Self::return_contract(cx, signature.output(), candidate) else {
            return;
        };
        let target_ty = return_.target;
        let Some(target) = ConversionType::from_ty(target_ty) else {
            return;
        };
        if source == target {
            return;
        }

        // Prove that source-derived data participates in target construction.
        let mut source_bindings = HashSet::new();
        ConversionEvidenceBindingCollector {
            bindings: &mut source_bindings,
        }
        .visit_pat(body.params[0].pat);

        // Follow derived bindings and effects through the complete function body.
        let mut evidence = ConversionEvidence::new(cx, candidate.target.def_id, source_bindings);
        evidence.visit_expr(body.value);
        if !evidence.has_source_reached_target {
            return;
        }
        self.target_owned_definitions
            .insert(candidate.function.def_id);

        // Apply parser, effect, policy, and confidence classification.
        let name = candidate.function.name;
        let has_hard_exclusion = Self::has_hard_name_exclusion(name.as_str());
        let parser_owned = candidate.is_text_parser() && source.is_str_slice();
        let is_reportable = !has_hard_exclusion && !evidence.has_effect && !parser_owned;
        let pair = ConversionPair { source, target };
        let confidence = Self::confidence(name.as_str(), source_ty, target_ty);

        // Retain complete diagnostic and family context until crate traversal ends.
        let identity = ConversionCandidateIdentity::from_construction(cx, body, candidate);
        let semantics = ConversionCandidateSemantics::new(
            source_ty.to_string(),
            target_ty.to_string(),
            return_.contract,
            confidence,
        );

        // Store the compact candidate beside its pair-selection policy.
        self.candidates.push(ConversionCandidate {
            identity,
            semantics,
            pair,
            is_reportable,
        });
    }

    /// Returns unique, unoccupied families eligible for diagnostics.
    pub fn reportable_candidates(&self) -> Vec<&ConversionCandidate> {
        // Group policy-eligible candidates by their exact semantic pair.
        let mut families = HashMap::<&ConversionPair, Vec<&ConversionCandidate>>::new();
        for candidate in self
            .candidates
            .iter()
            .filter(|candidate| candidate.is_reportable)
        {
            families.entry(&candidate.pair).or_default().push(candidate);
        }

        // Keep only unique families that no standard implementation already owns.
        let families = families.into_iter();
        let unoccupied = families
            .filter(|(pair, family)| family.len() == 1 && !self.occupied_pairs.contains(*pair))
            .map(|(_, family)| family[0]);
        let mut candidates = unoccupied.collect::<Vec<_>>();

        // Stabilize emission independently from hash-map iteration order.
        candidates.sort_unstable_by_key(|candidate| candidate.identity.name_span.lo());
        candidates
    }

    /// Returns functions whose result construction semantically consumes their sole source.
    pub const fn target_owned_definitions(&self) -> &HashSet<LocalDefId> {
        &self.target_owned_definitions
    }
}

// -----------------------------------------------------------------------------
// ConversionEvidence: Source flow and effect proof
// -----------------------------------------------------------------------------

/// Collects every binding introduced by the sole source parameter or a derived pattern.
struct ConversionEvidenceBindingCollector<'set> {
    /// Binding identities populated from one source-derived pattern.
    bindings: &'set mut HashSet<HirId>,
}

impl<'tcx> Visitor<'tcx> for ConversionEvidenceBindingCollector<'_> {
    fn visit_pat(&mut self, pattern: &'tcx Pat<'tcx>) {
        if let PatKind::Binding(_, binding, _, _) = pattern.kind {
            self.bindings.insert(binding);
        }
        intravisit::walk_pat(self, pattern);
    }
}

/// Resolved item whose identity can classify ambient or standard effects.
struct ConversionEvidenceEffectDefinition {
    /// Compiler classification used to recognize ambient static state.
    kind: DefKind,
    /// Resolved definition used to inspect the standard-library path.
    def_id: rustc_hir::def_id::DefId,
}

/// Finds one reference to a currently source-derived local.
struct ConversionEvidenceTaintedUse<'analysis, 'tcx> {
    /// Compiler context used to resolve local paths.
    cx: &'analysis LateContext<'tcx>,
    /// Binding identities currently derived from the source parameter.
    tainted: &'analysis HashSet<HirId>,
    /// Whether traversal has found a source-derived local reference.
    has_found: bool,
}

impl<'tcx> Visitor<'tcx> for ConversionEvidenceTaintedUse<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Path(path) = expression.kind
            && let Res::Local(binding) = self.cx.qpath_res(&path, expression.hir_id)
            && self.tainted.contains(&binding)
        {
            self.has_found = true;
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

/// Standard effect namespaces recognized without relying on authored names.
const CONVERSION_EVIDENCE_EFFECT_PREFIXES: &[&str] = &[
    "std::env::",
    "std::fs::",
    "std::io::",
    "std::net::",
    "std::process::",
];

/// Tracks derived locals and proves that they participate in target construction.
struct ConversionEvidence<'analysis, 'tcx> {
    /// Compiler context whose typeck results cover the visited body.
    cx: &'analysis LateContext<'tcx>,
    /// Nominal type promised by the candidate's return contract.
    target: LocalDefId,
    /// Local bindings transitively derived from the sole source parameter.
    tainted: HashSet<HirId>,
    /// Whether a source-derived value participates in target construction.
    has_source_reached_target: bool,
    /// Whether the body observes ambient state or known standard effects.
    has_effect: bool,
}

impl<'analysis, 'tcx> ConversionEvidence<'analysis, 'tcx> {
    /// Starts source-flow and effect analysis for one candidate body.
    const fn new(
        cx: &'analysis LateContext<'tcx>,
        target: LocalDefId,
        tainted: HashSet<HirId>,
    ) -> Self {
        Self {
            cx,
            target,
            tainted,
            has_source_reached_target: false,
            has_effect: false,
        }
    }

    /// Converts a resolved path into effect-classification context.
    fn path_effect_definition(
        &self,
        path: &rustc_hir::QPath<'tcx>,
        hir_id: HirId,
    ) -> Option<ConversionEvidenceEffectDefinition> {
        let Res::Def(kind, def_id) = self.cx.qpath_res(path, hir_id) else {
            return None;
        };
        Some(ConversionEvidenceEffectDefinition { kind, def_id })
    }

    /// Resolves the associated function selected by a method call.
    fn method_effect_definition(
        &self,
        expression: &Expr<'_>,
    ) -> Option<ConversionEvidenceEffectDefinition> {
        let typeck = self.cx.typeck_results();
        let def_id = typeck.type_dependent_def_id(expression.hir_id)?;
        Some(ConversionEvidenceEffectDefinition {
            kind: DefKind::AssocFn,
            def_id,
        })
    }

    /// Resolves the definition directly invoked by one expression.
    fn effect_definition(
        &self,
        expression: &'tcx Expr<'tcx>,
    ) -> Option<ConversionEvidenceEffectDefinition> {
        match expression.kind {
            ExprKind::Path(path) => self.path_effect_definition(&path, expression.hir_id),
            ExprKind::Call(callee, _) => self.effect_definition(callee),
            ExprKind::MethodCall(..) => self.method_effect_definition(expression),
            _ => None,
        }
    }

    /// Returns whether a path resolves to a fieldless or tuple constructor.
    fn is_unit_constructor(&self, path: &rustc_hir::QPath<'_>, hir_id: HirId) -> bool {
        matches!(
            self.cx.qpath_res(path, hir_id),
            Res::Def(DefKind::Ctor(..), _)
        )
    }

    /// Returns whether syntax denotes an operation capable of constructing a value.
    fn is_construction_operation(&self, expression: &Expr<'_>) -> bool {
        match expression.kind {
            ExprKind::Call(..) | ExprKind::MethodCall(..) | ExprKind::Struct(..) => true,
            ExprKind::Path(path) => self.is_unit_constructor(&path, expression.hir_id),
            _ => false,
        }
    }

    /// Returns whether an expression contains any source-derived local reference.
    fn uses_tainted(&self, expression: &'tcx Expr<'tcx>) -> bool {
        let mut finder = ConversionEvidenceTaintedUse {
            cx: self.cx,
            tainted: &self.tainted,
            has_found: false,
        };
        finder.visit_expr(expression);
        finder.has_found
    }

    /// Returns whether this expression constructs the promised target.
    fn produces_target(&self, expression: &Expr<'_>) -> bool {
        // Require an actual construction operation before comparing its resolved result.
        let operation = self.is_construction_operation(expression);

        // Compare the operation result with the promised local nominal target.
        let ty = self.cx.typeck_results().expr_ty(expression).peel_refs();
        let ty::Adt(definition, _) = ty.kind() else {
            return false;
        };
        operation && definition.did().as_local() == Some(self.target)
    }

    /// Marks known ambient-state and standard-library effect APIs.
    fn record_effect(&mut self, expression: &'tcx Expr<'tcx>) {
        // Resolve direct paths, calls, and method calls to their defining item.
        let definition = self.effect_definition(expression);

        // Classify ambient statics before checking known effectful API namespaces.
        let Some(definition) = definition else {
            return;
        };
        if matches!(definition.kind, DefKind::Static { .. }) {
            self.has_effect = true;
            return;
        }
        let path = self.cx.tcx.def_path_str(definition.def_id);
        self.has_effect |= CONVERSION_EVIDENCE_EFFECT_PREFIXES
            .iter()
            .any(|prefix| path.starts_with(prefix));
    }
}

impl<'tcx> Visitor<'tcx> for ConversionEvidence<'_, 'tcx> {
    fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
        let StmtKind::Let(local) = statement.kind else {
            intravisit::walk_stmt(self, statement);
            return;
        };
        if let Some(initializer) = local.init {
            self.visit_expr(initializer);
            if self.uses_tainted(initializer) {
                ConversionEvidenceBindingCollector {
                    bindings: &mut self.tainted,
                }
                .visit_pat(local.pat);
            }
        }
        let Some(else_block) = local.els else {
            return;
        };
        self.visit_block(else_block);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        self.record_effect(expression);
        if self.produces_target(expression) && self.uses_tainted(expression) {
            self.has_source_reached_target = true;
        }

        if let ExprKind::Assign(left, right, _) = expression.kind {
            self.visit_expr(right);
            if self.uses_tainted(right)
                && let ExprKind::Path(path) = left.kind
                && let Res::Local(binding) = self.cx.qpath_res(&path, left.hir_id)
            {
                self.tainted.insert(binding);
            }
            self.visit_expr(left);
            return;
        }
        if let ExprKind::Match(scrutinee, arms, _) = expression.kind
            && self.uses_tainted(scrutinee)
        {
            for arm in arms {
                ConversionEvidenceBindingCollector {
                    bindings: &mut self.tainted,
                }
                .visit_pat(arm.pat);
            }
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, body: rustc_hir::BodyId) {
        self.visit_body(self.cx.tcx.hir_body(body));
    }
}
