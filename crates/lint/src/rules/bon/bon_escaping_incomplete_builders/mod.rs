extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{FieldDef, FnRetTy, ImplItem, ImplItemKind, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::BonContractCatalog;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Incomplete builder crossing a boundary
// -----------------------------------------------------------------------------

/// Generated typestate that escapes its focused construction scope.
struct Violation {
    /// Return type or field type carrying the unfinished builder.
    span: Span,
    /// Kind of boundary named in the diagnostic.
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

// -----------------------------------------------------------------------------
// EscapeBoundary: Deferred type evidence
// -----------------------------------------------------------------------------

/// Private function return retained until type checking is complete.
struct EscapeBoundaryFunctionReturn {
    /// Function definition owning the return type.
    function: LocalDefId,
    /// Authored return type span.
    span: Span,
}

/// Private field retained until type checking is complete.
struct EscapeBoundaryStoredField {
    /// Field definition owning the stored type.
    field: LocalDefId,
    /// Authored field type span.
    span: Span,
}

// -----------------------------------------------------------------------------
// BonEscapingIncompleteBuilders: Focused construction policy
// -----------------------------------------------------------------------------

/// Finds generated builder types returned or stored before construction finishes.
#[derive(Default)]
struct BonEscapingIncompleteBuilders {
    /// Bon-generated builder definitions recognized in resolved types.
    catalog: BonContractCatalog,
    /// Private returns whose resolved output types are checked after collection.
    function_returns: Vec<EscapeBoundaryFunctionReturn>,
    /// Private fields whose resolved types are checked after collection.
    fields: Vec<EscapeBoundaryStoredField>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub BON_ESCAPING_INCOMPLETE_BUILDERS,
    Warn,
    "rejects incomplete Bon builders escaping focused construction scope",
    BonEscapingIncompleteBuilders::default()
}

impl<'tcx> LateLintPass<'tcx> for BonEscapingIncompleteBuilders {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.catalog.check_item(cx, item);

        // Generated and public items are not private escape boundaries owned by this lint.
        if item.span.from_expansion() || cx.tcx.visibility(item.owner_id.def_id).is_public() {
            return;
        }

        // Only functions can return an incomplete builder across a call boundary.
        let ItemKind::Fn { sig, .. } = item.kind else {
            return;
        };

        // An implicit unit return cannot carry an incomplete builder value.
        let FnRetTy::Return(output) = sig.decl.output else {
            return;
        };
        self.function_returns.push(EscapeBoundaryFunctionReturn {
            function: item.owner_id.def_id,
            span: output.span,
        });
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        // Generated and public methods are not private escape boundaries owned by this lint.
        if item.span.from_expansion() || cx.tcx.visibility(item.owner_id.def_id).is_public() {
            return;
        }

        // Only associated functions can return an incomplete builder.
        let ImplItemKind::Fn(signature, _) = item.kind else {
            return;
        };

        // An implicit unit return cannot carry an incomplete builder value.
        let FnRetTy::Return(output) = signature.decl.output else {
            return;
        };
        self.function_returns.push(EscapeBoundaryFunctionReturn {
            function: item.owner_id.def_id,
            span: output.span,
        });
    }

    fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &'tcx FieldDef<'tcx>) {
        // Only authored private fields represent internal storage escape boundaries.
        if !(!field.span.from_expansion() && !cx.tcx.visibility(field.def_id).is_public()) {
            return;
        }
        self.fields.push(EscapeBoundaryStoredField {
            field: field.def_id,
            span: field.ty.span,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for EscapeBoundaryFunctionReturn { function, span } in &self.function_returns {
            let output = cx
                .tcx
                .fn_sig(*function)
                .instantiate_identity()
                .skip_binder()
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
        for EscapeBoundaryStoredField { field, span } in &self.fields {
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
