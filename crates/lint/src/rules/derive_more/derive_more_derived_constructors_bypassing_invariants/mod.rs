extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, Expr, FnDecl, Item};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::utils::contracts::DeriveMoreContractCatalog;
use crate::utils::construction_analysis::{ConstructionAnalysis, ConstructionOrigin};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Derived constructor bypassing invariants
// -----------------------------------------------------------------------------

/// Generated constructor that exposes construction protected by field visibility.
struct Violation {
    /// Type declaration whose generated API may bypass its invariant.
    struct_span: Span,
    /// Constructor whose validation establishes the invariant boundary.
    constructor_span: Span,
    /// Local type name used to identify the affected derive contract.
    name: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "derive_more generates raw construction for `{}` despite its checked constructor",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the derived public `new` method can assemble restricted fields without passing through the fallible invariant boundary",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "remove `derive_more::Constructor` and retain the checked constructor as the construction boundary",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_DERIVED_CONSTRUCTORS_BYPASSING_INVARIANTS,
            self.struct_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.struct_span, "this derive exposes raw field assembly");
                diag.span_label(
                    self.constructor_span,
                    "this constructor owns fallible validation",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// DeriveMoreDerivedConstructorsBypassingInvariants: Safe construction policy
// -----------------------------------------------------------------------------

/// Rejects generated constructors that expose types with restricted fields.
#[derive(Default)]
struct DeriveMoreDerivedConstructorsBypassingInvariants {
    /// Authored type contracts and `derive_more` expansions consulted by this rule.
    catalog: DeriveMoreContractCatalog,
    /// Construction sites correlated with the owning type.
    constructions: ConstructionAnalysis,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_DERIVED_CONSTRUCTORS_BYPASSING_INVARIANTS,
    Warn,
    "rejects derive_more constructors that bypass checked construction",
    DeriveMoreDerivedConstructorsBypassingInvariants::default()
}

impl<'tcx> LateLintPass<'tcx> for DeriveMoreDerivedConstructorsBypassingInvariants {
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
        for constructor in &self.constructions.candidates {
            if constructor.ownership.origin != ConstructionOrigin::Inherent
                || !constructor.is_fallible_direct()
            {
                continue;
            }

            let Some(contract) = self
                .catalog
                .derived_type(constructor.target.def_id, "Constructor")
            else {
                continue;
            };

            if !contract.has_restricted_fields {
                continue;
            }

            Violation {
                struct_span: contract.span,
                constructor_span: constructor.function.name_span,
                name: contract.name.to_string(),
            }
            .emit(cx);
        }
    }
}
