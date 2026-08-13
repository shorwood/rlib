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

use crate::utils::comparison_analysis::{
    ComparisonAnalysis, ComparisonFamilyCandidate, ComparisonFamilyFinding,
    ComparisonFamilySelection, ComparisonProblem,
};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Canonical equality outside partial equality
// -----------------------------------------------------------------------------

/// Canonical-looking equality with complete relation and family context.
struct Violation {
    /// Complete relation and remediation context discovered by the analyzer.
    candidate: ComparisonFamilyCandidate,
    /// Missing, ambiguous, or competing trait ownership.
    problem: ComparisonProblem,
}

impl From<ComparisonFamilyFinding<'_>> for Violation {
    fn from(finding: ComparisonFamilyFinding<'_>) -> Self {
        Self {
            candidate: finding.candidate.clone(),
            problem: finding.problem,
        }
    }
}

impl Violation {
    /// Describes one inferred equality family with no standard owner.
    fn missing_primary(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` defines canonical-looking equality for `{}` outside `PartialEq`",
            self.candidate.source.name, self.candidate.protocol.type_name
        ))
    }

    /// Describes several APIs competing to define canonical equality.
    fn ambiguous_primary(&self, count: usize) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` is one of {count} APIs competing to define equality for `{}`",
            self.candidate.source.name, self.candidate.protocol.type_name
        ))
    }

    /// Describes a detached equality beside an existing standard owner.
    fn competing_primary(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` implements equality for `{}` separately from its `PartialEq` contract",
            self.candidate.source.name, self.candidate.protocol.type_name
        ))
    }

    /// Guides migration of one unowned canonical equality relation.
    fn missing_remediation(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "if this is `{}`'s canonical equivalence relation, implement `PartialEq` there and add `Eq` only after confirming reflexivity; otherwise give the operation a contextual name or introduce a comparison wrapper",
            self.candidate.protocol.type_name
        ))
    }

    /// Guides disambiguation of several apparent canonical relations.
    fn ambiguous_remediation(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "choose one lawful `PartialEq` relation for `{}`; name every contextual relation by its key or policy, or encode it in a wrapper type",
            self.candidate.protocol.type_name
        ))
    }

    /// Guides reconciliation with an existing standard relation.
    fn competing_remediation(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "delegate to `{}`'s `PartialEq` implementation or make the different relation explicit in the API or a wrapper; ensure every existing `Hash` implementation follows the canonical equality fields",
            self.candidate.protocol.type_name
        ))
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        match self.problem {
            ComparisonProblem::MissingTrait => self.missing_primary(),
            ComparisonProblem::AmbiguousFamily { count } => self.ambiguous_primary(count),
            ComparisonProblem::CompetingTrait => self.competing_primary(),
        }
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "canonical equality determines generic comparisons, deduplication, sets, and map keys for `{}`; detached implementations can disagree with `Hash` or with each other",
            self.candidate.protocol.type_name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        match self.problem {
            ComparisonProblem::MissingTrait => self.missing_remediation(),
            ComparisonProblem::AmbiguousFamily { .. } => self.ambiguous_remediation(),
            ComparisonProblem::CompetingTrait => self.competing_remediation(),
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            AD_HOC_EQUALITY,
            self.candidate.source.hir_id,
            self.candidate.source.name_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.candidate.evidence.left,
                    "first value in the inferred equality relation",
                );
                diag.span_label(
                    self.candidate.evidence.right,
                    "second value in the same relation",
                );
                diag.span_label(
                    self.candidate.evidence.relation,
                    "both values determine this boolean result",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// AdHocEquality: Standard equality ownership policy
// -----------------------------------------------------------------------------
#[derive(Default)]
/// Collects equality families and existing trait ownership before reporting.
struct AdHocEquality {
    /// Shared relation analyzer used to select canonical equality families.
    comparisons: ComparisonAnalysis,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub AD_HOC_EQUALITY,
    Warn,
    "requires canonical equality relations to use PartialEq",
    AdHocEquality::default()
}

impl<'tcx> LateLintPass<'tcx> for AdHocEquality {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.comparisons.record_item(cx, item);
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
        self.comparisons.record_function(cx, kind, body, def_id);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for finding in self
            .comparisons
            .findings(ComparisonFamilySelection::Equality)
        {
            Violation::from(finding).emit(cx);
        }
    }
}
