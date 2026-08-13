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
// Violation: String domain family diagnostic
// -----------------------------------------------------------------------------

/// Declarations collectively acting as a missing string-backed domain type.
struct Violation {
    /// Primary declaration span for the inferred family.
    span: Span,
    /// Inferred domain type name used throughout the diagnostic.
    domain: String,
    /// Existing same-module type that can own the remaining behavior.
    existing_type: Option<String>,
    /// Declaration-specific evidence supporting the inferred family.
    labels: Vec<DomainFindingLabel>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "these declarations collectively define a stringly typed `{}` domain",
            self.domain
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the `{}` concept has related invariant and behavior but no single type owns or exposes them",
            self.domain
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        self.existing_type.as_ref().map_or_else(
            || {
                Cow::Owned(format!(
                    "introduce a string-backed `{}` type and move its invariant and behavior onto that type",
                    self.domain
                ))
            },
            |existing_type| {
                Cow::Owned(format!(
                "accept the existing `{existing_type}` type and move the remaining domain behavior onto it"
                ))
            },
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary_message = self.primary_message().into_owned();
        let rationale_message = self.rationale_message().into_owned();
        let remediation_message = self.remediation_message().into_owned();
        cx.emit_span_lint(
            STRINGLY_TYPED_DOMAIN_FUNCTION_FAMILIES,
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
// StringlyTypedDomainFunctionFamilies
// -----------------------------------------------------------------------------
#[derive(Default)]
/// Collects free functions and raw textual fields into module-local domain families.
struct StringlyTypedDomainFunctionFamilies {
    /// Shared module-local domain analysis.
    analyzer: DomainAnalyzer,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRINGLY_TYPED_DOMAIN_FUNCTION_FAMILIES,
    Warn,
    "detects free-function families that imply a missing string-backed domain type",
    StringlyTypedDomainFunctionFamilies::default()
}

impl<'tcx> LateLintPass<'tcx> for StringlyTypedDomainFunctionFamilies {
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
        for finding in self.analyzer.family_findings() {
            Violation {
                span: finding.span,
                domain: finding.domain,
                existing_type: finding.existing_type,
                labels: finding.labels,
            }
            .emit(cx);
        }
    }
}
