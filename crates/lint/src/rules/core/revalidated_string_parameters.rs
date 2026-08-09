extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FieldDef, FnDecl, Item};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::parameter_analysis::ParameterSignature;
use crate::utils::string_domain_analysis::DomainAnalyzer;

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
            cx.emit_span_lint(
                REVALIDATED_STRING_PARAMETERS,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(format!(
                        "raw string consumers repeatedly establish `{}` invariants",
                        finding.domain
                    ));
                    for label in finding.labels {
                        diag.span_label(label.span, label.message);
                    }
                    diag.help(format!(
                        "introduce the domain type `{}` with private storage, establish its invariant during construction, and accept that type downstream",
                        finding.domain
                    ));
                }),
            );
        }
    }
}
