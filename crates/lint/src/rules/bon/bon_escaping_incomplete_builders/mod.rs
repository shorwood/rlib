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

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `boundary` value used by this analysis.
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

/// Private function return retained for post-analysis.
struct FunctionReturn {
    /// Function definition owning the return type.
    function: LocalDefId,
    /// Authored return type span.
    span: Span,
}

/// Private field retained for post-analysis.
struct StoredField {
    /// Field definition owning the stored type.
    field: LocalDefId,
    /// Authored field type span.
    span: Span,
}

#[derive(Default)]
/// Carries the `BonEscapingIncompleteBuilders` state used by this analysis.
struct BonEscapingIncompleteBuilders {
    /// Stores the `catalog` value used by this analysis.
    catalog: BonContractCatalog,
    /// Stores the `function_returns` value used by this analysis.
    function_returns: Vec<FunctionReturn>,
    /// Stores the `fields` value used by this analysis.
    fields: Vec<StoredField>,
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
        let ItemKind::Fn { sig, .. } = item.kind else {
            return;
        };
        let FnRetTy::Return(output) = sig.decl.output else {
            return;
        };
        self.function_returns.push(FunctionReturn {
            function: item.owner_id.def_id,
            span: output.span,
        });
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        if item.span.from_expansion() || cx.tcx.visibility(item.owner_id.def_id).is_public() {
            return;
        }
        let ImplItemKind::Fn(signature, _) = item.kind else {
            return;
        };
        let FnRetTy::Return(output) = signature.decl.output else {
            return;
        };
        self.function_returns.push(FunctionReturn {
            function: item.owner_id.def_id,
            span: output.span,
        });
    }

    fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &'tcx FieldDef<'tcx>) {
        if !(!field.span.from_expansion() && !cx.tcx.visibility(field.def_id).is_public()) {
            return;
        }
        self.fields.push(StoredField {
            field: field.def_id,
            span: field.ty.span,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for FunctionReturn { function, span } in &self.function_returns {
            let output = cx
                .tcx
                // Reject inputs that do not satisfy this stage.
                .fn_sig(*function)
                .instantiate_identity()
                .skip_binder()
                // Perform the next step of the analysis.
                .output();
            if self.catalog.generated_builder_in_type(output).is_none() {
                continue;
            }
            Violation {
                span: *span,
                boundary: "return",
            }
            .emit(cx);
        }
        for StoredField { field, span } in &self.fields {
            let ty = cx.tcx.type_of(*field).instantiate_identity();
            if self.catalog.generated_builder_in_type(ty).is_none() {
                continue;
            }
            Violation {
                span: *span,
                boundary: "stored field",
            }
            .emit(cx);
        }
    }
}
