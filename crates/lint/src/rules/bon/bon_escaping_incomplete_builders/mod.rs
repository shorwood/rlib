extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{FieldDef, FnRetTy, ImplItem, ImplItemKind, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::BonContractCatalog;
use crate::utils::diagnostic::LateViolation;

struct Violation {
    span: Span,
    boundary: &'static str,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "incomplete Bon builder escapes through a {} boundary",
            self.boundary
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "moving generated typestate away from its focused construction expression obscures which scope owns the remaining required members",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "finish construction before crossing the boundary or define a named domain state for intentional staged configuration",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            BON_ESCAPING_INCOMPLETE_BUILDERS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this boundary carries an unfinished builder");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct BonEscapingIncompleteBuilders {
    catalog: BonContractCatalog,
    function_returns: Vec<(LocalDefId, Span)>,
    fields: Vec<(LocalDefId, Span)>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub BON_ESCAPING_INCOMPLETE_BUILDERS,
    Warn,
    "rejects incomplete Bon builders escaping focused construction scope",
    BonEscapingIncompleteBuilders::default()
}

impl<'tcx> LateLintPass<'tcx> for BonEscapingIncompleteBuilders {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || cx.tcx.visibility(item.owner_id.def_id).is_public() {
            return;
        }
        if let ItemKind::Fn { sig, .. } = item.kind
            && let FnRetTy::Return(output) = sig.decl.output
        {
            self.function_returns
                .push((item.owner_id.def_id, output.span));
        }
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        if item.span.from_expansion() || cx.tcx.visibility(item.owner_id.def_id).is_public() {
            return;
        }
        if let ImplItemKind::Fn(signature, _) = item.kind
            && let FnRetTy::Return(output) = signature.decl.output
        {
            self.function_returns
                .push((item.owner_id.def_id, output.span));
        }
    }

    fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &'tcx FieldDef<'tcx>) {
        if !field.span.from_expansion() && !cx.tcx.visibility(field.def_id).is_public() {
            self.fields.push((field.def_id, field.ty.span));
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for (function, span) in &self.function_returns {
            let output = cx
                .tcx
                .fn_sig(*function)
                .instantiate_identity()
                .skip_binder()
                .output();
            if self.catalog.generated_builder_in_type(output).is_some() {
                Violation {
                    span: *span,
                    boundary: "return",
                }
                .emit(cx);
            }
        }
        for (field, span) in &self.fields {
            let ty = cx.tcx.type_of(*field).instantiate_identity();
            if self.catalog.generated_builder_in_type(ty).is_some() {
                Violation {
                    span: *span,
                    boundary: "stored field",
                }
                .emit(cx);
            }
        }
    }
}
