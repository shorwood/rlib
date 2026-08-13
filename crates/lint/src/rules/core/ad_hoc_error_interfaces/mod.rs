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
    ErrorInterfaceContract, ErrorInterfaceFinding, StandardInterfaceAnalysis,
};

// -----------------------------------------------------------------------------
// Violation: Standard error interface diagnostic
// -----------------------------------------------------------------------------

/// One active `Result` error with proven presentation or causal conventions.
struct Violation {
    /// Error evidence and the exact missing standard contracts.
    finding: ErrorInterfaceFinding,
}

impl Violation {
    /// Maximum labels retained for each supporting evidence category.
    const MAX_EVIDENCE_LABELS: usize = 3;

    /// Renders the exact missing standard interfaces in dependency order.
    fn missing_contracts(&self) -> String {
        let mut contracts = Vec::new();
        for contract in &self.finding.missing {
            contracts.push(match contract {
                ErrorInterfaceContract::Debug => "`Debug`",
                ErrorInterfaceContract::Display => "`Display`",
                ErrorInterfaceContract::Error => "`std::error::Error`",
                ErrorInterfaceContract::Source => "`Error::source`",
            });
        }
        contracts.join(", ")
    }

    /// Recommends completing a proven causal error contract.
    fn causal_remediation(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "implement the missing standard contracts for `{}` and expose only the proven causal predecessor through `Error::source`",
            self.finding.type_name
        ))
    }

    /// Recommends completing an error-like type's standard contracts.
    fn error_remediation(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "implement the missing `Debug`, `Display`, and `std::error::Error` contracts for `{}`",
            self.finding.type_name
        ))
    }

    /// Recommends canonical presentation for presentation-only evidence.
    fn presentation_remediation(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "implement `std::fmt::Display` for `{}` and keep a named message accessor only for a distinct structured-data contract",
            self.finding.type_name
        ))
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "error type `{}` exposes an ad hoc interface but is missing {}",
            self.finding.type_name,
            self.missing_contracts()
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "standard error interfaces make presentation, reporting, generic bounds, and causal traversal available without project-specific conventions",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        // Match the recommendation to the strongest structurally proven contract.
        let needs_source = self
            .finding
            .missing
            .contains(&ErrorInterfaceContract::Source);

        // Distinguish complete error contracts from presentation-only contracts.
        let needs_error = self
            .finding
            .missing
            .contains(&ErrorInterfaceContract::Error);

        // Select the strongest remediation tier proved by missing contracts.
        match (needs_source, needs_error) {
            (true, _) => self.causal_remediation(),
            (false, true) => self.error_remediation(),
            (false, false) => self.presentation_remediation(),
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render all messages before moving the retained diagnostic evidence.
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Emit one type-level diagnostic with bounded representative evidence.
        cx.tcx.emit_node_span_lint(
            AD_HOC_ERROR_INTERFACES,
            self.finding.hir_id,
            self.finding.declaration_span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                for span in self.finding.evidence.result_uses {
                    diag.span_label(span, "this active `Result` uses the type as its error");
                }
                for span in self
                    .finding
                    .evidence
                    .presentation_spans
                    .into_iter()
                    .take(Self::MAX_EVIDENCE_LABELS)
                {
                    diag.span_label(span, "ad hoc presentation evidence");
                }
                for span in self
                    .finding
                    .evidence
                    .causal_spans
                    .into_iter()
                    .take(Self::MAX_EVIDENCE_LABELS)
                {
                    diag.span_label(span, "ad hoc causal accessor");
                }
                diag.note(rationale);
                diag.help(remediation);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// AdHocErrorInterfaces: Crate wide lint pass
// -----------------------------------------------------------------------------
#[derive(Default)]
/// Collects active `Result`, message, cause, and standard-trait evidence crate-wide.
struct AdHocErrorInterfaces {
    /// Shared crate-wide standard-interface evidence.
    interfaces: StandardInterfaceAnalysis,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub AD_HOC_ERROR_INTERFACES,
    Warn,
    "requires structurally proven Result errors to use standard error interfaces",
    AdHocErrorInterfaces::default()
}

impl<'tcx> LateLintPass<'tcx> for AdHocErrorInterfaces {
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
        for finding in self.interfaces.error_findings() {
            Violation { finding }.emit(cx);
        }
    }
}
