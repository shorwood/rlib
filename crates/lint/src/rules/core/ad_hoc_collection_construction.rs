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

use crate::utils::collection_construction_analysis::{
    CollectionConstructionAnalysis, CollectionFamilyFinding, CollectionProblem,
};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Sequence ingestion outside collection traits
// -----------------------------------------------------------------------------

/// Proven sequence-to-storage operation with exact trait-family context.
struct Violation {
    /// Complete collection and remediation context discovered by the analyzer.
    candidate: crate::utils::collection_construction_analysis::CollectionFamilyCandidate,
    /// Missing, ambiguous, or competing trait ownership.
    problem: CollectionProblem,
}

impl From<CollectionFamilyFinding<'_>> for Violation {
    fn from(finding: CollectionFamilyFinding<'_>) -> Self {
        Self {
            candidate: finding.candidate.clone(),
            problem: finding.problem,
        }
    }
}

impl Violation {
    /// Describes one collection family with no standard owner.
    fn missing_primary(&self, trait_name: &str) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` reproduces `{}<{}>` for `{}`",
            self.candidate.source.name,
            trait_name,
            self.candidate.protocol.item_name,
            self.candidate.protocol.target_name
        ))
    }

    /// Describes several APIs competing to ingest the same item family.
    fn ambiguous_primary(&self, count: usize) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` is one of {count} APIs competing to ingest `{}` into `{}`",
            self.candidate.source.name,
            self.candidate.protocol.item_name,
            self.candidate.protocol.target_name
        ))
    }

    /// Describes detached ingestion beside an existing standard owner.
    fn competing_primary(&self, trait_name: &str) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` ingests `{}` separately from `{}`'s `{}` contract",
            self.candidate.source.name,
            self.candidate.protocol.item_name,
            self.candidate.protocol.target_name,
            trait_name
        ))
    }

    /// Guides migration of one unowned standard ingestion behavior.
    fn missing_remediation(&self, trait_name: &str) -> Cow<'_, str> {
        Cow::Owned(format!(
            "implement `{}<{}>` for `{}` and accept `IntoIterator<Item = {}>` at convenience boundaries; retain a named API only when its policy is explicit and cannot be represented by the item type",
            trait_name,
            self.candidate.protocol.item_name,
            self.candidate.protocol.target_name,
            self.candidate.protocol.item_name
        ))
    }

    /// Guides disambiguation of several apparent standard ingestion behaviors.
    fn ambiguous_remediation(&self, trait_name: &str) -> Cow<'_, str> {
        Cow::Owned(format!(
            "select one intrinsic `{}<{}>` behavior for `{}` and expose alternative ordering, duplicate, validation, or truncation policies explicitly",
            trait_name, self.candidate.protocol.item_name, self.candidate.protocol.target_name
        ))
    }

    /// Guides reconciliation with an existing collection trait implementation.
    fn competing_remediation(&self, trait_name: &str) -> Cow<'_, str> {
        Cow::Owned(format!(
            "delegate to `{}`'s existing `{}<{}>` implementation or make the different ingestion policy explicit in the method name and source item type",
            self.candidate.protocol.target_name, trait_name, self.candidate.protocol.item_name
        ))
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        // Select wording that distinguishes absent, ambiguous, and competing ownership.
        let trait_name = self.candidate.protocol.contract.trait_name();
        match self.problem {
            CollectionProblem::MissingTrait => self.missing_primary(trait_name),
            CollectionProblem::AmbiguousFamily { count } => self.ambiguous_primary(count),
            CollectionProblem::CompetingTrait => self.competing_primary(trait_name),
        }
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the complete infallible flow from an item sequence into target-owned storage is the standard `{}` protocol; a named duplicate hides collection and extension support from generic code",
            self.candidate.protocol.contract.trait_name()
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        // Keep the exact target, item, and standard trait in every remediation path.
        let trait_name = self.candidate.protocol.contract.trait_name();
        match self.problem {
            CollectionProblem::MissingTrait => self.missing_remediation(trait_name),
            CollectionProblem::AmbiguousFamily { .. } => self.ambiguous_remediation(trait_name),
            CollectionProblem::CompetingTrait => self.competing_remediation(trait_name),
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            AD_HOC_COLLECTION_CONSTRUCTION,
            self.candidate.source.hir_id,
            self.candidate.source.name_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.candidate.source.source_span,
                    "this source supplies the stored item sequence",
                );
                diag.span_label(
                    self.candidate.source.evidence_span,
                    "items flow into target-owned collection storage here",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// AdHocCollectionConstruction: Standard sequence ingestion policy
// -----------------------------------------------------------------------------

/// Collects collection storage, trait occupancy, and sequence-flow evidence.
#[derive(Default)]
struct AdHocCollectionConstruction {
    /// Shared analyzer for storage flow and existing collection traits.
    collections: CollectionConstructionAnalysis,
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds free and inherent constructors that completely consume an iterable into a local
    /// collection wrapper, plus free and inherent extension operations that do the same through a
    /// mutable target. Source items must flow into target-owned standard collection storage.
    ///
    /// ### Why is this bad?
    ///
    /// Project-specific collection protocols cannot use `collect`, `extend`, generic bounds, or
    /// standard iterator composition. Several names for the same item family also conceal whether
    /// ordering, duplicates, validation, and truncation are intrinsic or accidental.
    ///
    /// ```rust
    /// struct Report { entries: Vec<Entry> }
    /// # struct Entry;
    /// impl Report {
    ///     fn from_entries(entries: impl IntoIterator<Item = Entry>) -> Self {
    ///         let mut report = Self { entries: Vec::new() };
    ///         report.entries.extend(entries);
    ///         report
    ///     }
    /// }
    /// ```
    ///
    /// Fallible validation, filtering, truncation, early exit, explicit policy arguments, and
    /// builder finalization remain named. Multiple distinct item types may each implement the
    /// standard traits independently.
    pub AD_HOC_COLLECTION_CONSTRUCTION,
    Warn,
    "requires canonical sequence construction and extension to use standard collection traits",
    AdHocCollectionConstruction::default()
}

impl<'tcx> LateLintPass<'tcx> for AdHocCollectionConstruction {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.collections.record_item(cx, item);
    }

    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _: Span,
        def_id: LocalDefId,
    ) {
        self.collections.record_function(cx, kind, body, def_id);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for finding in self.collections.findings() {
            Violation::from(finding).emit(cx);
        }
    }
}
