extern crate rustc_abi;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use convert_case::{Case, Casing};
use rustc_abi::ExternAbi;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, Expr, FieldDef, HirId, Item, ItemKind, PatKind};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::symbol::sym;
use rustc_span::{Span, Symbol};

use self::text_body_evidence::TextBodyEvidence;
use super::identifier_case;

mod text_body_evidence;

// -----------------------------------------------------------------------------
// FormattingProblem: Canonical ownership problem
// -----------------------------------------------------------------------------
#[derive(Clone, Copy)]
/// Why one formatting family is reportable.
pub enum FormattingProblem {
    /// One canonical-looking family has no `Display` owner.
    MissingDisplay,
    /// Several neutral helpers compete for canonical ownership.
    Ambiguous,
    /// Helpers merely reproduce an existing `Display` implementation.
    RedundantDisplay,
}

// -----------------------------------------------------------------------------
// FormattingCandidate: Authored textual helper evidence
// -----------------------------------------------------------------------------
#[derive(Clone)]
/// One authored textual helper retained for diagnostic evidence.
pub struct FormattingCandidateSource {
    /// HIR node used for lint-level attributes.
    pub hir_id: HirId,
    /// Authored helper name.
    pub name: Symbol,
    /// Identifier span used by diagnostics.
    pub span: Span,
}
#[derive(Clone)]
/// One authored textual helper retained for diagnostic evidence.
pub struct FormattingCandidate {
    /// Function definition used for cross-lint precedence.
    def_id: LocalDefId,
    /// Authored helper identity and source location.
    pub source: FormattingCandidateSource,
    /// Whether the body directly delegates to the standard presentation.
    has_display_delegation: bool,
    /// Local type whose representation is produced.
    target: LocalDefId,
}

// -----------------------------------------------------------------------------
// FormattingFinding: Canonical presentation diagnostic evidence
// -----------------------------------------------------------------------------

/// One formatting family and the standard-contract problem it demonstrates.
pub struct FormattingFinding {
    /// Human-readable target name.
    pub target_name: String,
    /// All helpers participating in the finding.
    pub candidates: Vec<FormattingCandidate>,
    /// Missing, ambiguous, or redundant ownership.
    pub problem: FormattingProblem,
}

// -----------------------------------------------------------------------------
// FormattingVocabulary: Canonical and policy name vocabulary
// -----------------------------------------------------------------------------

/// Names that claim one ordinary human-readable representation.
const FORMATTING_VOCABULARY_NEUTRAL_WORDS: &[&str] =
    &["Display", "Format", "Render", "Text", "To", "Value"];

/// Vocabulary that makes a textual representation explicitly noncanonical.
const FORMATTING_VOCABULARY_POLICY_WORDS: &[&str] = &[
    "Ansi",
    "Base64",
    "Canonical",
    "Compact",
    "Css",
    "Csv",
    "Debug",
    "Encode",
    "Hex",
    "Html",
    "Json",
    "Lossy",
    "Pretty",
    "Redact",
    "Serialize",
    "Sql",
    "Url",
    "Wire",
    "With",
    "Xml",
    "Yaml",
];

// -----------------------------------------------------------------------------
// SecretTypeMarkers: Sensitive type vocabulary
// -----------------------------------------------------------------------------

/// Compound secret-bearing type vocabulary that must not gain ordinary formatting accidentally.
const SECRET_TYPE_MARKERS: &[&str] = &[
    "password",
    "credential",
    "access_token",
    "refresh_token",
    "api_key",
    "private_key",
    "seed_phrase",
];

// -----------------------------------------------------------------------------
// ErrorInterface: Standard error diagnostic evidence
// -----------------------------------------------------------------------------
#[derive(Clone, Copy, Eq, PartialEq)]
/// One standard error contract that structural evidence requires.
pub enum ErrorInterfaceContract {
    /// `Debug`, required by `Error`.
    Debug,
    /// Canonical human-readable presentation.
    Display,
    /// The standard error trait.
    Error,
    /// Standard causal traversal.
    Source,
}

/// Supporting spans retained for one error-interface diagnostic.
pub struct ErrorInterfaceEvidence {
    /// Representative active `Result` uses.
    pub result_uses: Vec<Span>,
    /// Authored message or formatting evidence.
    pub presentation_spans: Vec<Span>,
    /// Authored causal accessors with proven error targets.
    pub causal_spans: Vec<Span>,
}

/// One error type missing one or more standard interfaces.
pub struct ErrorInterfaceFinding {
    /// Declaration HIR node used for lint-level attributes.
    pub hir_id: HirId,
    /// Authored type identifier.
    pub declaration_span: Span,
    /// Human-readable type name.
    pub type_name: String,
    /// Active-use, presentation, and causal diagnostic evidence.
    pub evidence: ErrorInterfaceEvidence,
    /// Missing contracts in prerequisite order.
    pub missing: Vec<ErrorInterfaceContract>,
}

// -----------------------------------------------------------------------------
// CausalEvidence: Proven causal accessor
// -----------------------------------------------------------------------------

/// One direct causal accessor and the error type it exposes.
struct CausalEvidence {
    /// Authored accessor range.
    span: Span,
    /// Concrete local error returned by the accessor.
    target: Option<LocalDefId>,
    /// Whether the accessor returns a resolved `dyn Error` object.
    is_trait_object: bool,
}

impl CausalEvidence {
    /// Resolves a direct shared error reference, optionally wrapped in `Option`.
    fn from_output(cx: &LateContext<'_>, output: Ty<'_>, span: Span) -> Option<Self> {
        // Unwrap only the standard optional causal contract.
        let output = match output.kind() {
            ty::Adt(definition, arguments)
                if cx.tcx.is_diagnostic_item(sym::Option, definition.did()) =>
            {
                arguments.type_at(0)
            }
            _ => output,
        };
        let ty::Ref(_, inner, rustc_hir::Mutability::Not) = output.kind() else {
            return None;
        };

        // Resolve trait-object causes independently from concrete local causes.
        let is_trait_object = matches!(inner.kind(), ty::Dynamic(predicates, ..) if predicates.iter().any(|predicate| {
            matches!(predicate.skip_binder(), ty::ExistentialPredicate::Trait(reference) if cx.tcx.def_path_str(reference.def_id).ends_with("::error::Error"))
        }));
        let target = match inner.kind() {
            ty::Adt(definition, _) => definition.did().as_local(),
            _ => None,
        };

        // Retain only outputs whose causal contract can be proven later.
        (is_trait_object || target.is_some()).then_some(Self {
            span,
            target,
            is_trait_object,
        })
    }
}

// -----------------------------------------------------------------------------
// InterfaceType: Eligible authored nominal type
// -----------------------------------------------------------------------------

/// Stable classification of one eligible local type.
struct InterfaceTypeClassification {
    /// Whether the name ends in `Error` or `Failure`.
    is_error_named: bool,
    /// Whether the type belongs to a known structured-data family.
    is_structured_data: bool,
    /// Whether ordinary formatting could expose secret material.
    is_secret: bool,
}

/// Authored convention evidence accumulated for one local type.
struct InterfaceTypeEvidence {
    /// Authored textual fields and accessors.
    presentation_spans: Vec<Span>,
    /// Authored accessors that may expose one causal predecessor.
    causal: Vec<CausalEvidence>,
}

/// Authored local type eligible for interface analysis.
struct InterfaceType {
    /// Declaration HIR node used for lint-level attributes.
    hir_id: HirId,
    /// Authored type identifier.
    span: Span,
    /// Authored type name.
    name: Symbol,
    /// Stable name and policy classification.
    classification: InterfaceTypeClassification,
    /// Authored message and causal conventions.
    evidence: InterfaceTypeEvidence,
}

// -----------------------------------------------------------------------------
// ErrorInterfaceStructuredDataSuffixes: Preserved data families
// -----------------------------------------------------------------------------

/// Data-oriented families that should not be forced into Rust's causal error model.
const ERROR_INTERFACE_STRUCTURED_DATA_SUFFIXES: &[&str] = &[
    "Diagnostic",
    "Issue",
    "Rejection",
    "Report",
    "Response",
    "Violation",
];

// -----------------------------------------------------------------------------
// LocalAdt: Local nominal type resolution
// -----------------------------------------------------------------------------

/// Resolves a possibly referenced type to one local nominal definition.
fn local_adt(ty: Ty<'_>) -> Option<LocalDefId> {
    let ty::Adt(definition, _) = ty.peel_refs().kind() else {
        return None;
    };
    definition.did().as_local()
}

// -----------------------------------------------------------------------------
// StandardInterfaceEligibility: Eligible declarations and functions
// -----------------------------------------------------------------------------

/// Returns whether a declaration owns unresolved type or const parameters.
fn standard_interface_eligibility_has_type_or_const_parameters(
    cx: &LateContext<'_>,
    def_id: LocalDefId,
) -> bool {
    cx.tcx
        .generics_of(def_id)
        .own_params
        .iter()
        .any(|parameter| !matches!(parameter.kind, ty::GenericParamDefKind::Lifetime))
}

/// Returns whether a function belongs to a trait declaration or implementation.
fn standard_interface_eligibility_is_trait_method(
    cx: &LateContext<'_>,
    def_id: LocalDefId,
) -> bool {
    cx.tcx.opt_local_parent(def_id).is_some_and(|parent| {
        matches!(
            cx.tcx.def_kind(parent),
            DefKind::Impl { of_trait: true } | DefKind::Trait
        )
    })
}

/// Returns whether an associated function belongs to an inherent implementation.
fn standard_interface_eligibility_is_inherent_method(
    cx: &LateContext<'_>,
    def_id: LocalDefId,
) -> bool {
    cx.tcx
        .opt_local_parent(def_id)
        .is_some_and(|parent| matches!(cx.tcx.def_kind(parent), DefKind::Impl { of_trait: false }))
}

// -----------------------------------------------------------------------------
// StandardInterfaceText: Textual contract recognition
// -----------------------------------------------------------------------------

/// Recognizes exact `String` and `Cow<str>` return contracts.
fn standard_interface_text_is_owned(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    let ty::Adt(definition, arguments) = ty.kind() else {
        return false;
    };
    let path = cx.tcx.def_path_str(definition.did());
    path.ends_with("::string::String")
        || (path.ends_with("::borrow::Cow") && arguments.types().next().is_some_and(Ty::is_str))
}

/// Recognizes owned text and immutable string slices.
fn standard_interface_text_is_textual(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    standard_interface_text_is_owned(cx, ty)
        || matches!(ty.kind(), ty::Ref(_, inner, rustc_hir::Mutability::Not) if inner.is_str())
}

/// Returns whether a helper name carries only presentation and target vocabulary.
fn standard_interface_text_has_neutral_name(
    cx: &LateContext<'_>,
    name: Symbol,
    target: LocalDefId,
) -> bool {
    // Reject explicit formats before accepting target-derived vocabulary.
    let words = identifier_case::words(name.as_str());
    if words.is_empty()
        || words
            .iter()
            .any(|word| FORMATTING_VOCABULARY_POLICY_WORDS.contains(&word.as_str()))
    {
        return false;
    }

    // Accept only ordinary presentation words and the represented type's own vocabulary.
    let type_words = identifier_case::words(cx.tcx.item_name(target.to_def_id()).as_str());
    words.into_iter().all(|word| {
        FORMATTING_VOCABULARY_NEUTRAL_WORDS.contains(&word.as_str()) || type_words.contains(&word)
    })
}

// -----------------------------------------------------------------------------
// StandardInterfaceVocabularyIs: Authored convention names
// -----------------------------------------------------------------------------

/// Recognizes project-specific message accessor conventions.
fn standard_interface_vocabulary_is_message_accessor(name: &str) -> bool {
    ["description", "display_message", "message", "reason"].contains(&name)
}

/// Recognizes project-specific causal accessor conventions.
fn standard_interface_vocabulary_is_causal_accessor(name: &str) -> bool {
    ["cause", "inner", "source"].contains(&name)
}

/// Returns whether a type name carries strong secret-bearing vocabulary.
fn standard_interface_vocabulary_is_secret_type(name: &str) -> bool {
    let snake = name.to_case(Case::Snake);
    SECRET_TYPE_MARKERS
        .iter()
        .any(|marker| snake == *marker || snake.ends_with(&format!("_{marker}")))
}

// -----------------------------------------------------------------------------
// StandardInterfaceAnalysis: Crate wide protocol evidence
// -----------------------------------------------------------------------------
#[derive(Default)]
/// Crate-wide evidence for canonical formatting and error interfaces.
pub struct StandardInterfaceAnalysis {
    /// Eligible authored local types.
    types: HashMap<LocalDefId, InterfaceType>,
    /// Canonical-looking textual helpers.
    formatting: Vec<FormattingCandidate>,
    /// Representative active `Result` uses keyed by error type.
    result_uses: HashMap<LocalDefId, Vec<Span>>,
    /// Types implementing `Display`.
    display: HashSet<LocalDefId>,
    /// Types implementing `Debug`.
    debug: HashSet<LocalDefId>,
    /// Types implementing `std::error::Error`.
    error: HashSet<LocalDefId>,
    /// Error implementations overriding `source`.
    error_source: HashSet<LocalDefId>,
    /// Types implementing Serde serialization.
    serialized: HashSet<LocalDefId>,
}

impl StandardInterfaceAnalysis {
    /// Records textual fields that provide authored presentation evidence.
    pub fn record_field(&mut self, cx: &LateContext<'_>, field: &FieldDef<'_>) {
        // Resolve one authored named field back to its local nominal type.
        if field.span.from_expansion() || field.is_positional() {
            return;
        }
        let parent = cx.tcx.parent(field.def_id.to_def_id());

        // Resolve struct and enum variant ownership.
        let target = match cx.tcx.def_kind(parent) {
            DefKind::Struct => parent.as_local(),
            DefKind::Variant => cx.tcx.parent(parent).as_local(),
            _ => None,
        };

        // Require a declaration retained by the crate-wide analyzer.
        let Some(target) = target else {
            return;
        };
        let Some(declaration) = self.types.get_mut(&target) else {
            return;
        };

        // Retain public or error-named textual message conventions.
        let name = field.ident.name.as_str();
        let is_message = ["context", "description", "message", "reason"].contains(&name);
        let field_ty = cx.tcx.type_of(field.def_id).instantiate_identity();
        if !is_message || !standard_interface_text_is_textual(cx, field_ty) {
            return;
        }

        // Require public evidence unless error naming already supplies intent.
        let is_public = cx.tcx.visibility(field.def_id).is_public();
        if !declaration.classification.is_error_named && !is_public {
            return;
        }
        declaration
            .evidence
            .presentation_spans
            .push(field.ident.span);
    }

    /// Returns local error types missing their exact proven standard contracts.
    pub fn error_findings(&self) -> Vec<ErrorInterfaceFinding> {
        let mut findings = Vec::new();
        for (target, declaration) in &self.types {
            // Require active result use.
            let Some(result_uses) = self.result_uses.get(target) else {
                continue;
            };

            // Preserve known structured data contracts.
            if declaration.classification.is_structured_data || self.serialized.contains(target) {
                continue;
            }

            // Combine authored message fields, accessors, and canonical formatters.
            let formatting = self
                .formatting
                .iter()
                .filter(|candidate| candidate.target == *target)
                .map(|candidate| candidate.source.span);
            let mut presentation_spans = declaration.evidence.presentation_spans.clone();
            presentation_spans.extend(formatting);

            // Normalize repeated evidence before diagnostic selection.
            presentation_spans.sort_by_key(|span| span.lo());
            presentation_spans.dedup();

            // Retain only direct causes whose standard error contract is proven.
            let proven_causes = declaration.evidence.causal.iter().filter(|evidence| {
                evidence.is_trait_object
                    || evidence
                        .target
                        .is_some_and(|target| self.error.contains(&target))
            });
            let causal_spans = proven_causes
                .map(|evidence| evidence.span)
                .collect::<Vec<_>>();

            // Infer the exact interface tier from the retained evidence.
            let requires_error = !causal_spans.is_empty()
                || (declaration.classification.is_error_named && !presentation_spans.is_empty());
            let requires_display = requires_error || !presentation_spans.is_empty();
            if !requires_display {
                continue;
            }

            // Report only standard contracts not already implemented.
            let mut missing = Vec::new();
            if requires_error && !self.debug.contains(target) {
                missing.push(ErrorInterfaceContract::Debug);
            }
            if !self.display.contains(target) {
                missing.push(ErrorInterfaceContract::Display);
            }
            if requires_error && !self.error.contains(target) {
                missing.push(ErrorInterfaceContract::Error);
            }
            if !causal_spans.is_empty() && !self.error_source.contains(target) {
                missing.push(ErrorInterfaceContract::Source);
            }
            if missing.is_empty() {
                continue;
            }

            // Preserve the complete supporting span families.
            let evidence = ErrorInterfaceEvidence {
                result_uses: result_uses.clone(),
                presentation_spans,
                causal_spans,
            };

            // Preserve the full rationale and representative active use.
            let finding = ErrorInterfaceFinding {
                hir_id: declaration.hir_id,
                declaration_span: declaration.span,
                type_name: declaration.name.to_string(),
                evidence,
                missing,
            };

            // Add the complete type diagnostic to deterministic crate output.
            findings.push(finding);
        }
        findings.sort_by_key(|finding| finding.declaration_span.lo());
        findings
    }

    /// Returns targets claimed by the more specific error-interface diagnostic.
    fn reportable_error_targets(&self) -> HashSet<LocalDefId> {
        let findings = self.error_findings();
        findings
            .into_iter()
            .map(|finding| finding.hir_id.owner.def_id)
            .collect()
    }

    /// Returns formatting families not owned by the more specific error diagnostic.
    pub fn formatting_findings(&self, cx: &LateContext<'_>) -> Vec<FormattingFinding> {
        let error_targets = self.reportable_error_targets();
        let mut families = HashMap::<LocalDefId, Vec<FormattingCandidate>>::new();
        for candidate in &self.formatting {
            // Error-specific and secret-bearing ownership supersede formatting advice.
            let is_error = error_targets.contains(&candidate.target);
            let declaration = self.types.get(&candidate.target);
            let is_secret = declaration.is_some_and(|ty| ty.classification.is_secret);
            if is_error || is_secret {
                continue;
            }

            // Group every remaining helper by represented local type.
            families
                .entry(candidate.target)
                .or_default()
                .push(candidate.clone());
        }
        let mut findings = Vec::new();
        for (target, mut family) in families {
            // Classify missing, competing, or redundant canonical ownership.
            let problem = if self.display.contains(&target) {
                family.retain(|candidate| candidate.has_display_delegation);
                FormattingProblem::RedundantDisplay
            } else if family.len() > 1 {
                FormattingProblem::Ambiguous
            } else {
                FormattingProblem::MissingDisplay
            };
            if family.is_empty() {
                continue;
            }

            // Retain one deterministically ordered family diagnostic.
            family.sort_by_key(|candidate| candidate.source.span.lo());
            findings.push(FormattingFinding {
                target_name: cx.tcx.def_path_str(target.to_def_id()),
                candidates: family,
                problem,
            });
        }
        findings.sort_by_key(|finding| finding.candidates[0].source.span.lo());
        findings
    }

    /// Returns function definitions claimed by reportable formatting families.
    pub fn reportable_formatting_definitions(&self, cx: &LateContext<'_>) -> HashSet<LocalDefId> {
        let findings = self.formatting_findings(cx);
        let candidates = findings.into_iter().flat_map(|finding| finding.candidates);
        candidates.map(|candidate| candidate.def_id).collect()
    }

    /// Records one eligible authored local struct or enum.
    fn record_type(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Reject generated and open generic declarations.
        if item.span.from_expansion()
            || standard_interface_eligibility_has_type_or_const_parameters(cx, item.owner_id.def_id)
        {
            return;
        }
        let Some(ident) = item.kind.ident() else {
            return;
        };

        // Classify strong naming and structured-data evidence.
        let name = ident.name.as_str();
        let is_error_named = name.ends_with("Error") || name.ends_with("Failure");

        // Preserve known data families and sensitive type names.
        let is_structured_data = ERROR_INTERFACE_STRUCTURED_DATA_SUFFIXES
            .iter()
            .any(|suffix| name.ends_with(suffix));

        // Combine the independent policy classification facts.
        let classification = InterfaceTypeClassification {
            is_error_named,
            is_structured_data,
            is_secret: standard_interface_vocabulary_is_secret_type(name),
        };

        // Initialize empty authored convention evidence.
        let evidence = InterfaceTypeEvidence {
            presentation_spans: Vec::new(),
            causal: Vec::new(),
        };

        // Retain the eligible declaration for crate-post analysis.
        let declaration = InterfaceType {
            hir_id: item.hir_id(),
            span: ident.span,
            name: ident.name,
            classification,
            evidence,
        };

        // Index the complete declaration by its local definition.
        self.types.insert(item.owner_id.def_id, declaration);
    }

    /// Records standard trait occupancy for one local implementation target.
    fn record_implementation(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Resolve only trait implementations on concrete local types.
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };
        let Some(trait_ref) = cx.tcx.impl_opt_trait_ref(item.owner_id.def_id) else {
            return;
        };
        let self_ty = cx.tcx.type_of(item.owner_id.def_id).instantiate_identity();
        let Some(target) = local_adt(self_ty) else {
            return;
        };

        // Record simple presentation and serialization occupancy.
        let path = cx.tcx.def_path_str(trait_ref.instantiate_identity().def_id);
        if path.ends_with("::fmt::Display") {
            self.display.insert(target);
            return;
        }
        if path.ends_with("::fmt::Debug") {
            self.debug.insert(target);
            return;
        }
        if path == "serde::ser::Serialize" || path == "serde_core::ser::Serialize" {
            self.serialized.insert(target);
            return;
        }

        // Record error occupancy and whether source traversal is implemented.
        if path.ends_with("::error::Error") {
            self.error.insert(target);
            let has_source = implementation.items.iter().any(|reference| {
                cx.tcx
                    .associated_item(reference.owner_id.to_def_id())
                    .name()
                    .as_str()
                    == "source"
            });
            if has_source {
                self.error_source.insert(target);
            }
            return;
        }

        // Treat an explicit secret-exposure protocol as sensitive ownership.
        if !path.ends_with("::ExposeSecret") {
            return;
        }
        let Some(declaration) = self.types.get_mut(&target) else {
            return;
        };
        declaration.classification.is_secret = true;
    }

    /// Records eligible declarations and standard trait implementations, including derives.
    pub fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        match item.kind {
            ItemKind::Struct(..) | ItemKind::Enum(..) => self.record_type(cx, item),
            ItemKind::Impl(_) => self.record_implementation(cx, item),
            _ => {}
        }
    }

    /// Records the first active standard Result use for one local error type.
    fn record_result_type(&mut self, cx: &LateContext<'_>, ty: Ty<'_>, span: Span) {
        // Resolve only exact standard result values.
        let ty::Adt(definition, arguments) = ty.peel_refs().kind() else {
            return;
        };
        if !cx.tcx.is_diagnostic_item(sym::Result, definition.did()) {
            return;
        }
        let error = arguments.type_at(1);
        let Some(target) = local_adt(error) else {
            return;
        };

        // Require a concrete eligible local error.
        if !self.types.contains_key(&target) {
            return;
        }

        // Keep one representative active use for a compact diagnostic.
        let uses = self.result_uses.entry(target).or_default();
        if !uses.is_empty() {
            return;
        }
        uses.push(span);
    }

    /// Records canonical text helpers, message accessors, causal accessors, and signature uses.
    pub fn record_function<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        body: &'tcx Body<'tcx>,
        def_id: LocalDefId,
    ) {
        // Resolve the authored function.
        let (name, header, is_free) = match kind {
            FnKind::ItemFn(ident, _, header) => (ident.name, header, true),
            FnKind::Method(ident, signature) => (ident.name, signature.header, false),
            FnKind::Closure => return,
        };
        let signature = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();

        // Record active signature level result use.
        let input_types = signature.inputs().iter().copied();
        for ty in input_types.chain([signature.output()]) {
            self.record_result_type(cx, ty, cx.tcx.def_span(def_id));
        }

        // Restrict interface candidates to simple authored Rust functions.
        let has_unsupported_header = header.abi != ExternAbi::Rust
            || header.is_unsafe()
            || header.is_async()
            || matches!(header.constness, rustc_hir::Constness::Const);

        // Reject generated, generic, trait-owned, and multi-input functions.
        let is_ineligible = cx.tcx.def_span(def_id).from_expansion()
            || standard_interface_eligibility_has_type_or_const_parameters(cx, def_id)
            || standard_interface_eligibility_is_trait_method(cx, def_id)
            || body.params.len() != 1;
        if has_unsupported_header || is_ineligible {
            return;
        }

        // Require exactly one input after rejecting unsupported headers.
        let [input] = signature.inputs() else {
            return;
        };

        // Resolve the single shared local input and its authored binding.
        let ty::Ref(_, receiver, rustc_hir::Mutability::Not) = input.kind() else {
            return;
        };
        let Some(target) = local_adt(*receiver) else {
            return;
        };

        // Require an eligible target and free or inherent ownership.
        if !self.types.contains_key(&target) {
            return;
        }
        if !is_free && !standard_interface_eligibility_is_inherent_method(cx, def_id) {
            return;
        }

        // Prove the authored binding contributes to the returned text.
        let PatKind::Binding(_, binding, _, None) = body.params[0].pat.kind else {
            return;
        };
        let evidence = TextBodyEvidence::analyze(cx, body, binding);
        if !evidence.has_input_use {
            return;
        }

        // Record project-specific presentation and causal accessors first.
        let output = signature.output();
        if standard_interface_text_is_textual(cx, output)
            && standard_interface_vocabulary_is_message_accessor(name.as_str())
            && let Some(declaration) = self.types.get_mut(&target)
        {
            declaration
                .evidence
                .presentation_spans
                .push(cx.tcx.def_span(def_id));
        }
        if standard_interface_vocabulary_is_causal_accessor(name.as_str())
            && let Some(causal) = CausalEvidence::from_output(cx, output, cx.tcx.def_span(def_id))
            && let Some(declaration) = self.types.get_mut(&target)
        {
            declaration.evidence.causal.push(causal);
        }

        // Retain only canonical-looking owned text helpers.
        if !standard_interface_text_is_owned(cx, output)
            || name.as_str() == "to_string"
            || !standard_interface_text_has_neutral_name(cx, name, target)
        {
            return;
        }

        // Preserve the complete helper as family evidence.
        let source = FormattingCandidateSource {
            hir_id: cx.tcx.local_def_id_to_hir_id(def_id),
            name,
            span: cx.tcx.def_span(def_id),
        };

        // Combine source identity with protocol classification.
        let candidate = FormattingCandidate {
            def_id,
            source,
            has_display_delegation: evidence.has_display_delegation,
            target,
        };
        self.formatting.push(candidate);
    }

    /// Records inferred standard `Result` expressions as active error-interface evidence.
    pub fn record_expression(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        if expression.span.from_expansion() {
            return;
        }
        self.record_result_type(cx, cx.typeck_results().expr_ty(expression), expression.span);
    }
}
