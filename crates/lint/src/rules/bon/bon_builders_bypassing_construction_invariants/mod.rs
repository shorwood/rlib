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

use super::utils::contracts::BonContractCatalog;
use crate::utils::construction_analysis::{ConstructionAnalysis, ConstructionOrigin};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Builder bypassing a checked constructor
// -----------------------------------------------------------------------------

/// Derived builder that can assemble a restricted type without validation.
struct Violation {
    /// Struct declaration that derives raw-field construction.
    struct_span: Span,
    /// Fallible constructor that owns the type's invariant checks.
    constructor_span: Span,
    /// Restricted type named in the diagnostic.
    name: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Bon derives raw-field construction for `{}` despite its checked constructor",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the derived finishing method can assemble restricted fields without passing through the fallible invariant boundary",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "remove the struct derive and apply Bon's `#[builder]` to the checked constructor inside a `#[bon]` inherent impl",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            BON_BUILDERS_BYPASSING_CONSTRUCTION_INVARIANTS,
            self.struct_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.struct_span,
                    "this builder constructs restricted fields directly",
                );
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
// BonBuildersBypassingConstructionInvariants: Checked construction policy
// -----------------------------------------------------------------------------

/// Correlates Bon derives with fallible inherent constructors.
#[derive(Default)]
struct BonBuildersBypassingConstructionInvariants {
    /// Bon derive contracts indexed by their target type.
    catalog: BonContractCatalog,
    /// Authored constructors and their construction origins.
    constructions: ConstructionAnalysis,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub BON_BUILDERS_BYPASSING_CONSTRUCTION_INVARIANTS,
    Warn,
    "rejects Bon derives that bypass checked constructors",
    BonBuildersBypassingConstructionInvariants::default()
}

impl<'tcx> LateLintPass<'tcx> for BonBuildersBypassingConstructionInvariants {
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
            // Only inherent fallible constructors can serve as invariant boundaries.
            if constructor.ownership.origin != ConstructionOrigin::Inherent
                || !constructor.is_fallible_direct()
            {
                continue;
            }

            let Some(contract) = self.catalog.derived_struct(constructor.target.def_id) else {
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
