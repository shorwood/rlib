extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, Expr, FnDecl, Item};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::contracts::DeriveMoreContractCatalog;
use crate::utils::construction_analysis::{ConstructionAnalysis, ConstructionOrigin};
use crate::utils::diagnostic::LateViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `struct_span` value used by this analysis.
    struct_span: Span,
    /// Stores the `constructor_span` value used by this analysis.
    constructor_span: Span,
    /// Stores the `name` value used by this analysis.
    name: String,
    /// Stores the `derives` value used by this analysis.
    derives: Vec<&'static str>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "derive_more generates unchecked conversion into `{}`",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the structural conversion constructs restricted state without passing through the type's fallible invariant boundary",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "remove the {} derive{} and route conversion through the checked constructor or an authored `TryFrom` implementation",
            self.derives.join(", "),
            if self.derives.len() == 1 { "" } else { "s" }
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_DERIVED_CONVERSIONS_BYPASSING_INVARIANTS,
            self.struct_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.struct_span,
                    "unchecked structural conversion is derived here",
                );
                diag.span_label(
                    self.constructor_span,
                    "this constructor establishes fallible validation",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
/// Carries the `DeriveMoreDerivedConversionsBypassingInvariants` state used by this analysis.
struct DeriveMoreDerivedConversionsBypassingInvariants {
    /// Stores the `catalog` value used by this analysis.
    catalog: DeriveMoreContractCatalog,
    /// Stores the `constructions` value used by this analysis.
    constructions: ConstructionAnalysis,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_DERIVED_CONVERSIONS_BYPASSING_INVARIANTS,
    Warn,
    "rejects derive_more conversions that bypass checked construction",
    DeriveMoreDerivedConversionsBypassingInvariants::default()
}

impl<'tcx> LateLintPass<'tcx> for DeriveMoreDerivedConversionsBypassingInvariants {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.catalog.check_item(cx, item);
        self.constructions.record_item(cx, item);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        self.constructions.record_expression(cx, expression);
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
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let mut reported = HashSet::new();
        for constructor in &self.constructions.candidates {
            // Reject inputs that do not satisfy this stage.
            if constructor.ownership.origin != ConstructionOrigin::Inherent
                || !constructor.is_fallible_direct()
                || !reported.insert(constructor.target.def_id)
            {
                continue;
            }

            // Prepare the values used by this stage.
            let derives = self
                .catalog
                .derives_for(constructor.target.def_id, &["From", "TryFrom"]);
            if derives.is_empty() {
                continue;
            }

            // Prepare the values used by this stage.
            let Some(contract) = self.catalog.type_contract(constructor.target.def_id) else {
                continue;
            };
            if !contract.has_restricted_fields {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                struct_span: contract.span,
                constructor_span: constructor.function.name_span,
                name: contract.name.to_string(),
                derives,
            }
            .emit(cx);
        }
    }
}
