extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FieldDef, FnDecl, Item};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::diagnostic::LateViolation;
use crate::utils::parameter_analysis::ParameterSignature;
use crate::utils::string_domain_analysis::{DomainAnalyzer, DomainFindingLabel};

// -----------------------------------------------------------------------------
// Violation: Repeated string invariant diagnostic
// -----------------------------------------------------------------------------

/// Raw string domain whose invariant is repeatedly established by consumers.
struct Violation {
    /// Primary parameter span for the repeated domain.
    span: Span,
    /// Inferred domain type name used in the remediation.
    domain: String,
    /// Consumer-specific evidence explaining each repeated validation site.
    labels: Vec<DomainFindingLabel>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "raw string consumers repeatedly establish `{}` invariants",
            self.domain
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the `{}` invariant can drift or be bypassed because validity is not represented by the accepted type",
            self.domain
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "introduce the domain type `{}` with private storage, establish its invariant during construction, and accept that type downstream",
            self.domain
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render the stable diagnostic layers before moving consumer labels.
        let primary_message = self.primary_message().into_owned();
        let rationale_message = self.rationale_message().into_owned();
        let remediation_message = self.remediation_message().into_owned();

        // Emit the stable layers before consuming consumer-specific labels.
        cx.emit_span_lint(
            REVALIDATED_STRING_PARAMETERS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary_message);
                for label in self.labels {
                    diag.span_label(label.span, label.message);
                }
                diag.note(rationale_message);
                diag.help(remediation_message);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// RevalidatedStringParameters
// -----------------------------------------------------------------------------

/// Collects consumer-side string validation across one crate.
#[derive(Default)]
struct RevalidatedStringParameters {
    /// Shared module-local domain and dataflow analysis.
    analyzer: DomainAnalyzer,
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds domain-shaped textual parameters whose invariants are established near the start of
    /// at least two consumers in the same module. Validation and normalization helper calls and
    /// parameter-dependent early rejection guards count as evidence. The constructor, parser, or
    /// validator that establishes the boundary is not itself a consumer.
    ///
    /// ### Why is this bad?
    ///
    /// Revalidating raw text at every use means the type permits invalid states throughout the
    /// program. Every new consumer must remember the same checks, normalization can drift, and an
    /// agent can accidentally bypass the invariant by adding one apparently harmless `&str`
    /// parameter.
    ///
    /// ```rust
    /// fn send_invitation(email: &str) -> Result<()> {
    ///     validate_email(email)?;
    ///     deliver(email)
    /// }
    ///
    /// fn subscribe(email: &str) -> Result<()> {
    ///     validate_email(email)?;
    ///     store(email)
    /// }
    /// ```
    ///
    /// Establish validity once and make invalid construction impossible:
    ///
    /// ```rust
    /// struct EmailAddress(String);
    ///
    /// fn send_invitation(email: &EmailAddress) -> Result<()> {
    ///     deliver(email)
    /// }
    /// ```
    pub REVALIDATED_STRING_PARAMETERS,
    Warn,
    "detects raw string parameters whose domain invariants are repeatedly re-established",
    RevalidatedStringParameters::default()
}

impl<'tcx> LateLintPass<'tcx> for RevalidatedStringParameters {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _: Span,
        def_id: LocalDefId,
    ) {
        let Some(signature) = ParameterSignature::from_body(cx, kind, body, def_id) else {
            return;
        };
        self.analyzer.record_function(cx, &signature, body);
    }

    fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &'tcx FieldDef<'tcx>) {
        self.analyzer.record_field(cx, field);
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.analyzer.record_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for finding in self.analyzer.revalidation_findings() {
            Violation {
                span: finding.span,
                domain: finding.domain,
                labels: finding.labels,
            }
            .emit(cx);
        }
    }
}
