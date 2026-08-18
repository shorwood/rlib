extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind, Item};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

use super::utils::contracts::DeriveMoreContractCatalog;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Opaque generated display contract
// -----------------------------------------------------------------------------

/// Derived display contract awaiting authored-format inspection.
struct Candidate {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
}

/// Display contract whose generated grammar is not explicit in source.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Local type name used to identify the affected derive contract.
    name: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "derived `Display` for `{}` is used as machine identity",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "using presentation output as a standard map key makes formatting changes alter lookup, cache, or persistence identity",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "replace `to_string()` here with a named encoding method or a dedicated serializable key type whose compatibility contract is explicit",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_OPAQUE_DERIVED_DISPLAY_CONTRACTS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "presentation output becomes a map key here");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// DeriveMoreOpaqueDerivedDisplayContracts: Transparent display policy
// -----------------------------------------------------------------------------

/// Rejects public derived display formats whose authored grammar is not explicit.
#[derive(Default)]
struct DeriveMoreOpaqueDerivedDisplayContracts {
    /// Authored type contracts and `derive_more` expansions consulted by this rule.
    catalog: DeriveMoreContractCatalog,
    /// Authored declarations awaiting association with `derive_more` expansions.
    candidates: Vec<Candidate>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_OPAQUE_DERIVED_DISPLAY_CONTRACTS,
    Warn,
    "rejects derive_more Display output used as opaque machine identity",
    DeriveMoreOpaqueDerivedDisplayContracts::default()
}

impl<'tcx> LateLintPass<'tcx> for DeriveMoreOpaqueDerivedDisplayContracts {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.catalog.check_item(cx, item);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Generated calls are not authored choices of machine identity.
        if expression.span.from_expansion() {
            return;
        }

        // Map insertion must be expressed as a receiver method call.
        let ExprKind::MethodCall(_, _, arguments, _) = expression.kind else {
            return;
        };

        // Unresolved methods cannot establish a standard map insertion boundary.
        let Some(target) = cx
            .tcx
            .typeck(expression.hir_id.owner.def_id)
            .type_dependent_def_id(expression.hir_id)
        else {
            return;
        };
        let path = cx.tcx.def_path_str(target);

        // Exclude calls outside standard hash-map and tree-map insertion.
        if cx.tcx.item_name(target).as_str() != "insert"
            || (!path.contains("collections::HashMap") && !path.contains("collections::BTreeMap"))
        {
            return;
        }

        // A map insertion without a key argument provides no identity expression.
        let Some(key) = arguments.first() else {
            return;
        };

        // Only a direct conversion method can turn display output into the key.
        let ExprKind::MethodCall(_, source, to_string_arguments, _) = key.kind else {
            return;
        };

        // Conversion arguments indicate a different key-building contract.
        if !to_string_arguments.is_empty() {
            return;
        }

        // Unresolved conversions cannot prove use of the standard string interface.
        let Some(to_string) = cx
            .tcx
            .typeck(key.hir_id.owner.def_id)
            .type_dependent_def_id(key.hir_id)
        else {
            return;
        };

        // Exclude similarly named methods outside the standard string conversion trait.
        if cx.tcx.item_name(to_string).as_str() != "to_string"
            || !cx.tcx.trait_of_assoc(to_string).is_some_and(|trait_id| {
                cx.tcx
                    .is_diagnostic_item(Symbol::intern("ToString"), trait_id)
            })
        {
            return;
        }

        // Only local nominal source types can join the derive contract catalog.
        let Some(definition) = cx
            .tcx
            .typeck(key.hir_id.owner.def_id)
            .expr_ty(source)
            .peel_refs()
            .ty_adt_def()
            .and_then(|definition| definition.did().as_local())
        else {
            return;
        };
        self.candidates.push(Candidate {
            definition,
            span: key.span,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        self.candidates.sort_by_key(|candidate| candidate.span.lo());
        for candidate in self.candidates.drain(..) {
            // Sources without generated display output have no opaque derived grammar.
            let Some(contract) = self.catalog.derived_type(candidate.definition, "Display") else {
                continue;
            };

            Violation {
                span: candidate.span,
                name: contract.name.to_string(),
            }
            .emit(cx);
        }
    }
}
