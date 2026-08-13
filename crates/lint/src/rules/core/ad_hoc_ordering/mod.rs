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
// Violation: Canonical ordering outside standard traits
// -----------------------------------------------------------------------------

/// Canonical-looking ordering with exact totality and family context.
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
    /// Describes one inferred ordering family with no standard owner.
    fn missing_primary(&self, trait_name: &str) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` defines canonical-looking ordering for `{}` outside `{trait_name}`",
            self.candidate.source.name, self.candidate.protocol.type_name
        ))
    }

    /// Describes several APIs competing to define canonical ordering.
    fn ambiguous_primary(&self, count: usize) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` is one of {count} APIs competing to define ordering for `{}`",
            self.candidate.source.name, self.candidate.protocol.type_name
        ))
    }

    /// Describes a detached ordering beside an existing standard owner.
    fn competing_primary(&self, trait_name: &str) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` implements ordering for `{}` separately from its `{trait_name}` contract",
            self.candidate.source.name, self.candidate.protocol.type_name
        ))
    }

    /// Guides migration of one unowned canonical ordering relation.
    fn missing_remediation(&self, trait_name: &str) -> Cow<'_, str> {
        Cow::Owned(format!(
            "if this is the canonical relation, implement `{trait_name}` for `{}` and keep its equality stack consistent; otherwise name the ordering context or introduce a wrapper type",
            self.candidate.protocol.type_name
        ))
    }

    /// Guides disambiguation of several apparent canonical orderings.
    fn ambiguous_remediation(&self, trait_name: &str) -> Cow<'_, str> {
        Cow::Owned(format!(
            "select one lawful `{trait_name}` relation for `{}` and represent display, priority, version, or key orderings with explicit names or wrapper types",
            self.candidate.protocol.type_name
        ))
    }

    /// Guides reconciliation with an existing standard ordering relation.
    fn competing_remediation(&self, trait_name: &str) -> Cow<'_, str> {
        Cow::Owned(format!(
            "delegate to `{}`'s `{trait_name}` implementation or make this distinct ordering explicit; do not let `cmp == Equal` disagree with canonical equality",
            self.candidate.protocol.type_name
        ))
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        // Name the inferred trait before classifying its ownership problem.
        let trait_name = self.candidate.protocol.contract.trait_name();
        match self.problem {
            ComparisonProblem::MissingTrait => self.missing_primary(trait_name),
            ComparisonProblem::AmbiguousFamily { count } => self.ambiguous_primary(count),
            ComparisonProblem::CompetingTrait => self.competing_primary(trait_name),
        }
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "canonical ordering controls sorting, ordered collections, min/max operations, and equality coherence for `{}`; detached comparators can silently disagree",
            self.candidate.protocol.type_name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        // Keep totality-specific guidance beside the selected standard trait.
        let trait_name = self.candidate.protocol.contract.trait_name();
        match self.problem {
            ComparisonProblem::MissingTrait => self.missing_remediation(trait_name),
            ComparisonProblem::AmbiguousFamily { .. } => self.ambiguous_remediation(trait_name),
            ComparisonProblem::CompetingTrait => self.competing_remediation(trait_name),
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            AD_HOC_ORDERING,
            self.candidate.source.hir_id,
            self.candidate.source.name_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.candidate.evidence.left,
                    "first value in the inferred ordering relation",
                );
                diag.span_label(
                    self.candidate.evidence.right,
                    "second value in the same relation",
                );
                diag.span_label(
                    self.candidate.evidence.relation,
                    "both values determine this ordering result",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// AdHocOrdering: Standard ordering ownership policy
// -----------------------------------------------------------------------------
#[derive(Default)]
/// Collects ordering families and existing trait ownership before reporting.
struct AdHocOrdering {
    /// Shared relation analyzer used to select canonical ordering families.
    comparisons: ComparisonAnalysis,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub AD_HOC_ORDERING,
    Warn,
    "requires canonical ordering relations to use Ord or PartialOrd",
    AdHocOrdering::default()
}

impl<'tcx> LateLintPass<'tcx> for AdHocOrdering {
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
            .findings(ComparisonFamilySelection::Ordering)
        {
            Violation::from(finding).emit(cx);
        }
    }
}
