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

// -----------------------------------------------------------------------------
// Violation: Generated operator bypassing invariants
// -----------------------------------------------------------------------------

/// Generated operator that can construct values outside the wrapper invariant.
struct Violation {
    /// Type declaration whose generated API may bypass its invariant.
    struct_span: Span,
    /// Constructor whose validation establishes the invariant boundary.
    constructor_span: Span,
    /// Local type name used to identify the affected derive contract.
    name: String,
    /// Derive macros that establish the generated behavior.
    derives: Vec<&'static str>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "derive_more operators can bypass `{}` invariants",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the derived component-wise operations reconstruct restricted state without passing through the type's fallible invariant boundary",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "remove the {} derive{} and retain only operations that preserve or revalidate the invariant",
            self.derives.join(", "),
            if self.derives.len() == 1 { "" } else { "s" }
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_OPERATOR_DERIVES_BYPASSING_INVARIANTS,
            self.struct_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.struct_span, "unchecked operations are derived here");
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

/// Operator derives that can reconstruct a wrapper without invoking its constructor.
const OPERATIONS: &[&str] = &[
    "Add",
    "AddAssign",
    "Sub",
    "SubAssign",
    "Mul",
    "MulAssign",
    "Div",
    "DivAssign",
    "Rem",
    "RemAssign",
    "BitAnd",
    "BitAndAssign",
    "BitOr",
    "BitOrAssign",
    "BitXor",
    "BitXorAssign",
    "Shl",
    "ShlAssign",
    "Shr",
    "ShrAssign",
    "Neg",
    "Not",
    "Sum",
    "Product",
];

// -----------------------------------------------------------------------------
// DeriveMoreOperatorDerivesBypassingInvariants: Safe operator policy
// -----------------------------------------------------------------------------

/// Rejects generated operators that can construct values outside wrapper invariants.
#[derive(Default)]
struct DeriveMoreOperatorDerivesBypassingInvariants {
    /// Authored type contracts and `derive_more` expansions consulted by this rule.
    catalog: DeriveMoreContractCatalog,
    /// Construction sites correlated with the owning type.
    constructions: ConstructionAnalysis,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_OPERATOR_DERIVES_BYPASSING_INVARIANTS,
    Warn,
    "rejects derive_more operations that bypass checked construction",
    DeriveMoreOperatorDerivesBypassingInvariants::default()
}

impl<'tcx> LateLintPass<'tcx> for DeriveMoreOperatorDerivesBypassingInvariants {
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
            if constructor.ownership.origin != ConstructionOrigin::Inherent
                || !constructor.is_fallible_direct()
                || !reported.insert(constructor.target.def_id)
            {
                continue;
            }

            let derives = self
                .catalog
                .derives_for(constructor.target.def_id, OPERATIONS);
            if derives.is_empty() {
                continue;
            }

            let Some(contract) = self.catalog.type_contract(constructor.target.def_id) else {
                continue;
            };
            if !contract.has_restricted_fields {
                continue;
            }

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
