extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::utils::collection_construction_analysis::CollectionConstructionAnalysis;
use crate::utils::construction_analysis::ConstructionAnalysis;
use crate::utils::conversion_analysis::{
    ConversionAnalysis, ConversionCandidate, ConversionConfidence, ConversionContract,
};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Nonstandard conversion contract
// -----------------------------------------------------------------------------

/// Unique source-to-target construction with its exact standard-trait remediation.
struct ViolationLocation {
    /// Function HIR node used to anchor lint-level configuration.
    hir_id: rustc_hir::HirId,
    /// Function identifier used as the primary diagnostic range.
    span: Span,
    /// Sole source parameter range used for provenance context.
    source_span: Span,
}

impl From<&ConversionCandidate> for ViolationLocation {
    fn from(candidate: &ConversionCandidate) -> Self {
        Self {
            hir_id: candidate.identity.hir_id,
            span: candidate.identity.name_span,
            source_span: candidate.identity.source_span,
        }
    }
}

/// Concrete source and target spellings retained for diagnostic guidance.
struct ViolationTypes {
    /// Concrete source type shown in the trait sketch.
    source: String,
    /// Concrete target type shown in the trait sketch.
    target: String,
}

impl From<&ConversionCandidate> for ViolationTypes {
    fn from(candidate: &ConversionCandidate) -> Self {
        Self {
            source: candidate.semantics.source.clone(),
            target: candidate.semantics.target.clone(),
        }
    }
}

/// Unique source-to-target construction with its exact standard-trait remediation.
struct Violation {
    /// Function and source ranges used by the diagnostic.
    location: ViolationLocation,
    /// Authored function name shown in the primary message.
    function_name: String,
    /// Concrete source and target type spellings.
    types: ViolationTypes,
    /// Standard trait and optional error contract selected by return shape.
    contract: ConversionContract,
    /// Whether the name explicitly or only structurally claims conversion semantics.
    confidence: ConversionConfidence,
}

impl From<&ConversionCandidate> for Violation {
    fn from(candidate: &ConversionCandidate) -> Self {
        // Carry complete remediation context across the diagnostic boundary.
        Self {
            location: ViolationLocation::from(candidate),
            function_name: candidate.identity.name.to_string(),
            types: ViolationTypes::from(candidate),
            contract: candidate.semantics.contract.clone(),
            confidence: candidate.semantics.confidence,
        }
    }
}

impl Violation {
    /// Renders the exact standard trait and reciprocal caller contract.
    fn contract_remediation(&self) -> String {
        match &self.contract {
            ConversionContract::Infallible => format!(
                "implement `From<{}> for {}`; callers also receive `Into<{}> for {}`",
                self.types.source, self.types.target, self.types.target, self.types.source
            ),
            ConversionContract::Fallible { error } => format!(
                "implement `TryFrom<{}> for {}` with `Error = {error}`; callers also receive `TryInto<{}> for {}`",
                self.types.source, self.types.target, self.types.target, self.types.source
            ),
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        match self.confidence {
            ConversionConfidence::Conventional => Cow::Owned(format!(
                "`{}` is a canonical-looking conversion from `{}` to `{}` outside `{}`",
                self.function_name,
                self.types.source,
                self.types.target,
                self.contract.trait_name()
            )),
            ConversionConfidence::Structural => Cow::Owned(format!(
                "`{}` constructs `{}` from one `{}` value through an ad hoc conversion API",
                self.function_name, self.types.target, self.types.source
            )),
        }
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the `{}` contract makes this conversion discoverable, usable through generic bounds, and available through its reciprocal standard conversion trait",
            self.contract.trait_name()
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        // Render the exact standard trait pair and fallible error contract.
        let contract = self.contract_remediation();
        match self.confidence {
            ConversionConfidence::Conventional => Cow::Owned(contract),
            ConversionConfidence::Structural => Cow::Owned(format!(
                "if this is canonical, {contract}; otherwise retain a named API but make its format, policy, or required context explicit in the name or source wrapper"
            )),
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render stable diagnostic layers before moving owned violation context.
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Attach provenance beside the standard contract and rationale.
        cx.tcx.emit_node_span_lint(
            AD_HOC_CONVERSIONS,
            self.location.hir_id,
            self.location.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.span_label(
                    self.location.source_span,
                    format!(
                        "the sole `{}` source reaches `{}` construction",
                        self.types.source, self.types.target
                    ),
                );

                // Finish with trait rationale and concrete remediation guidance.
                diag.note(rationale);
                diag.help(remediation);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// AdHocConversions: Canonical conversion policy
// -----------------------------------------------------------------------------

/// Collects construction and trait evidence before selecting unique conversion families.
#[derive(Default)]
struct AdHocConversions {
    /// Construction discovery supplying local target and parser evidence.
    constructions: ConstructionAnalysis,
    /// Conversion-specific provenance, effect, family, and trait evidence.
    conversions: ConversionAnalysis,
    /// Collection ingestion that owns iterable-to-storage conversions more precisely.
    collections: CollectionConstructionAnalysis,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub AD_HOC_CONVERSIONS,
    Warn,
    "requires unique canonical source-to-target conversions to use From or TryFrom",
    AdHocConversions::default()
}

impl<'tcx> LateLintPass<'tcx> for AdHocConversions {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.constructions.record_item(cx, item);
        self.conversions.record_item(cx, item);
        self.collections.record_item(cx, item);
    }

    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        def_id: LocalDefId,
    ) {
        self.constructions
            .record_function(cx, kind, body, span, def_id);
        self.collections.record_function(cx, kind, body, def_id);
        let Some(candidate) = self.constructions.candidate(def_id) else {
            return;
        };
        self.conversions.record_function(cx, kind, body, candidate);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let collection_definitions = self.collections.reportable_definitions();
        for candidate in self.conversions.reportable_candidates() {
            if collection_definitions.contains(&candidate.identity.def_id) {
                continue;
            }
            Violation::from(candidate).emit(cx);
        }
    }
}
