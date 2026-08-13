extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind, Item};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::DeriveMoreContractCatalog;
use crate::utils::diagnostic::LateViolation;

struct Candidate {
    definition: LocalDefId,
    span: Span,
}

struct Violation {
    span: Span,
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

#[derive(Default)]
struct DeriveMoreOpaqueDerivedDisplayContracts {
    catalog: DeriveMoreContractCatalog,
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
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
        let ExprKind::MethodCall(_, _, arguments, _) = expression.kind else {
            return;
        };
        let Some(target) = cx
            .tcx
            .typeck(expression.hir_id.owner.def_id)
            .type_dependent_def_id(expression.hir_id)
        else {
            return;
        };
        let path = cx.tcx.def_path_str(target);
        if cx.tcx.item_name(target).as_str() != "insert"
            || (!path.contains("collections::HashMap") && !path.contains("collections::BTreeMap"))
        {
            return;
        }
        let Some(key) = arguments.first() else {
            return;
        };
        let ExprKind::MethodCall(_, source, to_string_arguments, _) = key.kind else {
            return;
        };
        if !to_string_arguments.is_empty() {
            return;
        }
        let Some(to_string) = cx
            .tcx
            .typeck(key.hir_id.owner.def_id)
            .type_dependent_def_id(key.hir_id)
        else {
            return;
        };
        if cx.tcx.item_name(to_string).as_str() != "to_string"
            || cx
                .tcx
                .trait_of_assoc(to_string)
                .is_none_or(|trait_id| cx.tcx.item_name(trait_id).as_str() != "ToString")
        {
            return;
        }
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
        for candidate in self.candidates.drain(..) {
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
