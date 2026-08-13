extern crate rustc_errors;
extern crate rustc_hir;
use std::borrow::Cow;
use std::mem::take;

use rustc_errors::DiagDecorator;
use rustc_hir::{AmbigArg, Item, PolyTraitRef, Ty};
use rustc_lint::{LateContext, LateLintPass};

use crate::utils::diagnostic::LateViolation;
use crate::utils::single_implementation_trait_analysis::{
    SingleImplementationTraitAnalyzer, SingleImplementationTraitFinding,
};

// -----------------------------------------------------------------------------
// Violation: Single implementation diagnostic
// -----------------------------------------------------------------------------

/// One trait declaration and sole implementation presented as abstraction evidence.
struct Violation(
    /// Complete semantic finding used by the diagnostic.
    SingleImplementationTraitFinding,
);

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "trait `{}` has a single concrete implementation and no polymorphic consumer",
            self.0.declaration.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` is the only concrete local implementation; no generic bound, `impl Trait`, trait object, associated-type projection, dependent supertrait, or trait alias uses this contract polymorphically",
            self.0.implementation.target
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "move the contract onto `{}` and use the concrete type directly; retain the trait when a real polymorphic boundary is introduced",
            self.0.implementation.target
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Attach declaration and implementation evidence without proposing a mechanical rewrite.
        cx.tcx.emit_node_span_lint(
            SINGLE_IMPLEMENTATION_TRAITS,
            self.0.declaration.hir_id,
            self.0.declaration.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.span_label(
                    self.0.implementation.span,
                    format!("sole implementation for `{}`", self.0.implementation.target),
                );
                diag.note(rationale);
                if self.0.is_closed_package_public {
                    diag.note(
                        "this public trait is analyzed because the package declares `publish = false`",
                    );
                }
                diag.help(remediation);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// SingleImplementationTraits: Crate wide abstraction policy
// -----------------------------------------------------------------------------
/// Late pass collecting declarations, implementations, and polymorphic type boundaries.
#[derive(Default)]
struct SingleImplementationTraits {
    /// Shared crate-wide semantic evidence collector.
    analyzer: SingleImplementationTraitAnalyzer,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SINGLE_IMPLEMENTATION_TRAITS,
    Warn,
    "questions local traits with one concrete implementation and no polymorphic consumer",
    SingleImplementationTraits::default()
}

impl<'tcx> LateLintPass<'tcx> for SingleImplementationTraits {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.analyzer.record_item(cx, item);
    }

    fn check_poly_trait_ref(
        &mut self,
        cx: &LateContext<'tcx>,
        trait_ref: &'tcx PolyTraitRef<'tcx>,
    ) {
        self.analyzer.record_poly_trait_ref(cx, trait_ref);
    }

    fn check_ty(&mut self, cx: &LateContext<'tcx>, ty: &'tcx Ty<'tcx, AmbigArg>) {
        self.analyzer.record_ty(cx, ty);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for finding in take(&mut self.analyzer).findings(cx) {
            Violation(finding).emit(cx);
        }
    }
}
