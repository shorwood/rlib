extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, Expr, FieldDef, FnDecl, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::standard_interface_analysis::{
    FormattingFinding, FormattingProblem, StandardInterfaceAnalysis,
};

// -----------------------------------------------------------------------------
// Violation: Canonical presentation diagnostic
// -----------------------------------------------------------------------------

/// One canonical-looking presentation family without coherent `Display` ownership.
struct Violation {
    /// Formatting family and its protocol conflict.
    finding: FormattingFinding,
}

impl Violation {
    /// Describes a canonical-looking family without standard ownership.
    fn missing_display_primary(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` has an ad hoc canonical text representation but no `Display` implementation",
            self.finding.target_name
        ))
    }

    /// Describes several helpers competing for standard ownership.
    fn ambiguous_primary(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` has {} competing canonical-looking text helpers",
            self.finding.target_name,
            self.finding.candidates.len()
        ))
    }

    /// Describes a helper duplicating an existing standard representation.
    fn redundant_display_primary(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "text helpers for `{}` duplicate its existing `Display` representation",
            self.finding.target_name
        ))
    }

    /// Recommends assigning missing canonical ownership to `Display`.
    fn missing_display_remediation(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "implement `std::fmt::Display` for `{}` and retain a named helper only when it adds a distinct format or policy",
            self.finding.target_name
        ))
    }

    /// Recommends resolving competition through ownership or explicit policy names.
    fn ambiguous_remediation(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "choose one ordinary `Display` representation for `{}` or rename every helper to state its distinct format or policy",
            self.finding.target_name
        ))
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        // Describe the exact ownership problem established by the family.
        match self.finding.problem {
            FormattingProblem::MissingDisplay => self.missing_display_primary(),
            FormattingProblem::Ambiguous => self.ambiguous_primary(),
            FormattingProblem::RedundantDisplay => self.redundant_display_primary(),
        }
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "one ordinary representation belongs in `Display`, where formatting, logging, interpolation, generic bounds, and allocation-aware writers can discover it consistently for `{}`",
            self.finding.target_name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        // Keep remediation specific to missing, competing, or redundant ownership.
        match self.finding.problem {
            FormattingProblem::MissingDisplay => self.missing_display_remediation(),
            FormattingProblem::Ambiguous => self.ambiguous_remediation(),
            FormattingProblem::RedundantDisplay => Cow::Borrowed(
                "remove the redundant helpers and use the existing formatting contract directly",
            ),
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render all messages before moving the retained diagnostic evidence.
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();
        let first = &self.finding.candidates[0];

        // Emit one family diagnostic and label every competing helper.
        cx.tcx.emit_node_span_lint(
            AD_HOC_FORMATTING,
            first.source.hir_id,
            first.source.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                for candidate in self.finding.candidates {
                    diag.span_label(
                        candidate.source.span,
                        format!(
                            "`{}` contributes this representation",
                            candidate.source.name
                        ),
                    );
                }
                diag.note(rationale);
                diag.help(remediation);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// AdHocFormatting: Crate wide lint pass
// -----------------------------------------------------------------------------

/// Collects text-producing APIs, standard implementations, and error-specific ownership.
#[derive(Default)]
struct AdHocFormatting {
    /// Shared crate-wide standard-interface evidence.
    interfaces: StandardInterfaceAnalysis,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub AD_HOC_FORMATTING,
    Warn,
    "requires canonical human-readable formatting to use Display",
    AdHocFormatting::default()
}

impl<'tcx> LateLintPass<'tcx> for AdHocFormatting {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.interfaces.record_item(cx, item);
    }

    fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &'tcx FieldDef<'tcx>) {
        self.interfaces.record_field(cx, field);
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
        self.interfaces.record_function(cx, kind, body, def_id);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        self.interfaces.record_expression(cx, expression);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for finding in self.interfaces.formatting_findings(cx) {
            Violation { finding }.emit(cx);
        }
    }
}
