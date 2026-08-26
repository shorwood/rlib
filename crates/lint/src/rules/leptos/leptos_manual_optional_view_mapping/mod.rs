extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::DefId;
use rustc_span::symbol::sym;

use super::utils::view_contract::ViewContract;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// OptionViewOperation: Supported optional view mappings
// -----------------------------------------------------------------------------

/// Standard option operation that constructs the present view.
#[derive(Clone, Copy)]
enum OptionViewOperation {
    /// Maps presence and leaves absence as the empty view.
    Map,
    /// Maps presence and supplies an eager fallback view.
    MapOr,
    /// Maps presence and lazily constructs the fallback view.
    MapOrElse,
}

impl OptionViewOperation {
    /// Returns the associated item spelling used by the standard option implementation.
    const fn name(self) -> &'static str {
        match self {
            Self::Map => "map",
            Self::MapOr => "map_or",
            Self::MapOrElse => "map_or_else",
        }
    }
}

// -----------------------------------------------------------------------------
// ResolvedCall: Syntax-independent option call evidence
// -----------------------------------------------------------------------------

/// One resolved associated call with its semantic receiver.
struct ResolvedCall<'hir> {
    /// Compiler-selected associated item.
    definition: DefId,
    /// Standard option value being mapped.
    receiver: &'hir Expr<'hir>,
}

// -----------------------------------------------------------------------------
// Violation: Optional view hidden in mapping
// -----------------------------------------------------------------------------

/// View presence and fallback encoded through standard option mapping.
struct Violation {
    /// Mapping expression used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored mapping highlighted by the diagnostic.
    span: Span,
    /// Resolved standard option operation.
    operation: &'static str,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "optional view is constructed through `Option::{}`",
            self.operation
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "option mapping hides the presence condition, bound payload, and fallback outside the declarative view tree",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("replace this mapping with `<ShowLet some=... let:value fallback=...>`")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_MANUAL_OPTIONAL_VIEW_MAPPING,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this option selects the rendered view");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosManualOptionalViewMapping: Declarative optional rendering
// -----------------------------------------------------------------------------

/// Late lint pass that requires optional views to use Leptos `ShowLet`.
struct LeptosManualOptionalViewMapping;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_MANUAL_OPTIONAL_VIEW_MAPPING,
    Warn,
    "requires declarative ShowLet components instead of mapped optional views",
    LeptosManualOptionalViewMapping
}

impl LeptosManualOptionalViewMapping {
    /// Resolves a direct function or associated-function path.
    fn path_definition(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<DefId> {
        // Other expression shapes cannot identify a direct callable definition.
        let ExprKind::Path(path) = expression.kind else {
            return None;
        };

        // Only callable definitions can represent the supported UFCS operations.
        let Res::Def(DefKind::Fn | DefKind::AssocFn, definition) =
            cx.qpath_res(&path, expression.hir_id)
        else {
            return None;
        };
        Some(definition)
    }

    /// Normalizes method and UFCS syntax into one option call shape.
    fn resolved_call<'hir>(
        cx: &LateContext<'_>,
        expression: &'hir Expr<'hir>,
    ) -> Option<ResolvedCall<'hir>> {
        match expression.kind {
            ExprKind::MethodCall(_, receiver, _, _) => {
                let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
                Some(ResolvedCall {
                    definition: cx
                        .tcx
                        .typeck(owner)
                        .type_dependent_def_id(expression.hir_id)?,
                    receiver,
                })
            }
            ExprKind::Call(callee, arguments) => {
                let (receiver, _) = arguments.split_first()?;
                Some(ResolvedCall {
                    definition: Self::path_definition(cx, callee)?,
                    receiver,
                })
            }
            _ => None,
        }
    }

    /// Returns whether an associated item belongs to standard `Option`'s inherent impl.
    fn is_option_method(cx: &LateContext<'_>, definition: DefId) -> bool {
        // Trait methods and free functions cannot own the standard inherent operation.
        let Some(implementation) = cx.tcx.impl_of_assoc(definition) else {
            return false;
        };
        let self_ty = cx.tcx.type_of(implementation).instantiate_identity();
        matches!(
            self_ty.kind(),
            ty::Adt(option, _) if cx.tcx.is_diagnostic_item(sym::Option, option.did())
        )
    }

    /// Returns whether the source span is an authored call rather than expansion glue.
    fn is_authored_call(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        cx.sess()
            .source_map()
            .span_to_snippet(expression.span)
            .is_ok_and(|source| {
                syn::parse_str::<syn::Expr>(&source).is_ok_and(|source| {
                    matches!(source, syn::Expr::MethodCall(_) | syn::Expr::Call(_))
                })
            })
    }

    /// Resolves the supported operation and proves that its mapped result is a view.
    fn operation(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<OptionViewOperation> {
        let call = Self::resolved_call(cx, expression)?;

        // Foreign mapping operations do not encode standard option presence.
        if !Self::is_option_method(cx, call.definition) {
            return None;
        }

        // The receiver must itself retain the standard option contract.
        let owner = cx.tcx.hir_enclosing_body_owner(call.receiver.hir_id);
        ViewContract::option_inner(cx, cx.tcx.typeck(owner).expr_ty(call.receiver))?;

        let operation = match cx.tcx.item_name(call.definition).as_str() {
            "map" => Some(OptionViewOperation::Map),
            "map_or" => Some(OptionViewOperation::MapOr),
            "map_or_else" => Some(OptionViewOperation::MapOrElse),
            _ => None,
        }?;

        // Resolve the mapped output through the operation's standard return contract.
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        let output = cx.tcx.typeck(owner).expr_ty(expression);
        let mapped = match operation {
            OptionViewOperation::Map => ViewContract::option_inner(cx, output)?,
            OptionViewOperation::MapOr | OptionViewOperation::MapOrElse => output,
        };
        ViewContract::is_view_type(cx, mapped).then_some(operation)
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosManualOptionalViewMapping {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Expansion glue cannot be migrated at the surfaced expression span.
        if !Self::is_authored_call(cx, expression) {
            return;
        }

        // Calls without a standard optional-view contract are unrelated to this lint.
        let Some(operation) = Self::operation(cx, expression) else {
            return;
        };
        Violation {
            owner: expression.hir_id,
            span: expression.span,
            operation: operation.name(),
        }
        .emit(cx);
    }
}
