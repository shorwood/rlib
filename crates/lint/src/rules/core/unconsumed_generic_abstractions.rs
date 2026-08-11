extern crate rustc_errors;
extern crate rustc_hir;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{AmbigArg, Expr, Item, Ty};
use rustc_lint::{LateContext, LateLintPass};

use crate::utils::diagnostic::LateViolation;
use crate::utils::generic_abstraction_analysis::{
    GenericAbstractionAnalyzer, GenericAbstractionFinding,
};

// -----------------------------------------------------------------------------
// Violation: Unconsumed generic abstraction diagnostic
// -----------------------------------------------------------------------------

/// One generic parameter with only one observed concrete substitution.
struct Violation(
    /// Complete declaration and use evidence.
    GenericAbstractionFinding,
);

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "type parameter `{}` on {} `{}` has only one concrete substitution",
            self.0.parameter.name, self.0.declaration.kind, self.0.declaration.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "every active authored use supplies `{}`; no generic forwarding, inferred argument, projection, opaque type, trait object, alias, or second concrete substitution demonstrates that this parameter is consumed polymorphically",
            self.0.concrete_type
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "replace `{}` with `{}` until a second real substitution is required",
            self.0.parameter.name, self.0.concrete_type
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render stable diagnostic layers before moving authored use evidence.
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Keep representative substitutions adjacent to the parameter-level recommendation.
        cx.tcx.emit_node_span_lint(
            UNCONSUMED_GENERIC_ABSTRACTIONS,
            self.0.declaration.hir_id,
            self.0.parameter.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                for use_span in self.0.use_spans {
                    diag.span_label(
                        use_span,
                        format!("`{}` is supplied here", self.0.concrete_type),
                    );
                }
                diag.note(rationale);
                if self.0.is_closed_package_public {
                    diag.note(
                        "this public declaration is analyzed because the package declares `publish = false`",
                    );
                }
                diag.help(remediation);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// UnconsumedGenericAbstractions: Nominal substitution policy
// -----------------------------------------------------------------------------

/// Late pass collecting generic nominal declarations and active authored substitutions.
#[derive(Default)]
struct UnconsumedGenericAbstractions {
    /// Shared crate-wide substitution analyzer.
    analyzer: GenericAbstractionAnalyzer,
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds authored type parameters on local structs, enums, unions, and type aliases when every
    /// active use supplies the same fully concrete type. Parameters are analyzed independently.
    /// Inferred, projected, opaque, aliased, trait-object, or generically forwarded arguments are
    /// treated as open boundaries. Exported APIs in publishable libraries are preserved.
    ///
    /// ### Why is this bad?
    ///
    /// A generic parameter with one concrete substitution introduces abstract vocabulary,
    /// monomorphized surface, and propagation through constructors and impls without supporting an
    /// observed variant. Specializing the declaration keeps the design concrete until another
    /// substitution is genuinely required.
    pub UNCONSUMED_GENERIC_ABSTRACTIONS,
    Warn,
    "questions nominal type parameters with only one concrete substitution",
    UnconsumedGenericAbstractions::default()
}

impl<'tcx> LateLintPass<'tcx> for UnconsumedGenericAbstractions {
    fn check_item(&mut self, _cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.analyzer.record_item(item);
    }

    fn check_ty(&mut self, cx: &LateContext<'tcx>, ty: &'tcx Ty<'tcx, AmbigArg>) {
        self.analyzer.record_ty(cx, ty);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        self.analyzer.record_expr(cx, expression);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for finding in std::mem::take(&mut self.analyzer).findings(cx) {
            Violation(finding).emit(cx);
        }
    }
}
