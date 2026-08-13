extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: rustc_hir::HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `ty` value used by this analysis.
    ty: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Leptos context type `{}` has no domain identity",
            self.ty
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "context lookup is type-based, so broad types can collide and do not name the capability descendants depend on",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "wrap this value in a local domain type and expose the narrowest capability descendants require",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_PRIMITIVE_CONTEXT_VALUES,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Carries the `LeptosPrimitiveContextValues` state used by this analysis.
struct LeptosPrimitiveContextValues;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_PRIMITIVE_CONTEXT_VALUES,
    Warn,
    "rejects Leptos contexts without nominal domain identity",
    LeptosPrimitiveContextValues
}

impl LateLintPass<'_> for LeptosPrimitiveContextValues {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Reject inputs that do not satisfy this stage.
        if expression.span.from_expansion() {
            return;
        }
        let ExprKind::Call(callee, arguments) = expression.kind else {
            return;
        };

        // Prepare the values used by this stage.
        let Some(operation) = ContextOperation::analyze_context_operation(cx, callee) else {
            return;
        };

        // Prepare the values used by this stage.
        let ty = match operation {
            ContextOperation::Provide => {
                let [value] = arguments else { return };
                cx.typeck_results().expr_ty(value)
            }
            ContextOperation::Consume => {
                let generic_arguments = cx.typeck_results().node_args(callee.hir_id);
                let Some(ty) = generic_arguments.types().next() else {
                    return;
                };
                ty
            }
        };

        // Reject inputs that do not satisfy this stage.
        if !(ambiguous_context_type(cx, ty)) {
            return;
        }
        Violation {
            owner: expression.hir_id,
            span: expression.span.source_callsite(),
            ty: ty.to_string(),
        }
        .emit(cx);
    }
}

#[derive(Clone, Copy)]
/// Classifies `ContextOperation` cases used by this analysis.
enum ContextOperation {
    /// Represents the `Provide` case.
    Provide,
    /// Represents the `Consume` case.
    Consume,
}

impl ContextOperation {
    /// Performs the `analyze_context_operation` step of the lint analysis.
    fn analyze_context_operation(cx: &LateContext<'_>, callee: &Expr<'_>) -> Option<Self> {
        // Prepare the values used by this stage.
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };
        let Res::Def(_, definition) = cx.qpath_res(&path, callee.hir_id) else {
            return None;
        };
        let path = cx.tcx.def_path_str(definition);

        // Reject inputs that do not satisfy this stage.
        if !path.contains("context")
            || !matches!(
                cx.tcx.crate_name(definition.krate).as_str(),
                "reactive_graph" | "leptos"
            )
        {
            return None;
        }

        // Classify the current analyze_candidate.
        match cx.tcx.item_name(definition).as_str() {
            "provide_context" => Some(Self::Provide),
            "use_context" | "expect_context" => Some(Self::Consume),
            _ => None,
        }
    }
}

/// Performs the `ambiguous_context_type` step of the lint analysis.
fn ambiguous_context_type(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    // Prepare the values used by this stage.
    let ty = ty.peel_refs();
    if ty.is_bool()
        || ty.is_char()
        || ty.is_integral()
        || ty.is_floating_point()
        || ty.is_str()
        || matches!(ty.kind(), ty::Tuple(_) | ty::Array(..) | ty::Slice(_))
    // Perform the next step of the analysis.
    {
        return true;
    }
    let ty::Adt(definition, _) = ty.kind() else {
        return matches!(ty.kind(), ty::FnDef(..) | ty::FnPtr(..) | ty::Dynamic(..));
    };
    let name = cx.tcx.item_name(definition.did());

    // Prepare the values used by this stage.
    let crate_name = cx.tcx.crate_name(definition.did().krate);

    // Perform the next step of the analysis.
    matches!(
        (crate_name.as_str(), name.as_str()),
        (
            "alloc",
            "String" | "Vec" | "VecDeque" | "BTreeMap" | "BTreeSet"
        ) | ("std", "HashMap" | "HashSet")
            | ("leptos", "Callback")
            | (
                "reactive_graph",
                "ReadSignal"
                    | "WriteSignal"
                    | "RwSignal"
                    | "ArcReadSignal"
                    | "ArcWriteSignal"
                    | "ArcRwSignal"
                    | "Signal"
                    | "ArcSignal"
                    | "StoredValue"
                    | "ArcStoredValue"
            )
    )
}
