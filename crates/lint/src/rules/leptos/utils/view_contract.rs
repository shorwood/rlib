extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::Expr;
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::def_id::{DefId, LocalDefId};
use rustc_span::symbol::sym;

// -----------------------------------------------------------------------------
// ViewContract: Explicit Leptos view boundary recognition
// -----------------------------------------------------------------------------

/// Semantic queries for types explicitly exposed as Leptos views.
pub struct ViewContract;

impl ViewContract {
    /// Returns whether a resolved type is explicitly represented as a Leptos view.
    pub fn is_view_type<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> bool {
        match ty.kind() {
            // Preserve the nominal framework wrappers used by authored concrete signatures.
            ty::Adt(definition, _) => Self::is_view_wrapper(cx, definition.did()),

            // Opaque component and helper results expose their intended contract in their bounds.
            ty::Alias(alias) => cx
                .tcx
                .explicit_item_bounds(alias.kind.def_id())
                .iter_instantiated_copied(cx.tcx, alias.args)
                .any(|(clause, _)| {
                    matches!(
                        clause.kind().skip_binder(),
                        ty::ClauseKind::Trait(predicate)
                            if Self::is_into_view_trait(cx, predicate.trait_ref.def_id)
                    )
                }),

            // Blanket-renderable scalars and arbitrary user types are not explicit view contracts.
            _ => false,
        }
    }

    /// Returns whether an expression resolves to an explicit Leptos view type.
    pub fn is_view_expression(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        Self::is_view_type(cx, cx.tcx.typeck(owner).expr_ty(expression))
    }

    /// Extracts the represented value from the standard option type.
    pub fn option_inner<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
        // Only nominal option types carry the payload contract this query represents.
        let ty::Adt(definition, arguments) = ty.kind() else {
            return None;
        };
        cx.tcx
            .is_diagnostic_item(sym::Option, definition.did())
            .then(|| arguments.type_at(0))
    }

    /// Returns the resolved output of a synchronous authored function.
    pub fn function_output<'tcx>(cx: &LateContext<'tcx>, definition: LocalDefId) -> Ty<'tcx> {
        cx.tcx
            .fn_sig(definition)
            .instantiate_identity()
            .skip_binder()
            .output()
    }

    /// Recognizes the exact public trait that declares an opaque view contract.
    fn is_into_view_trait(cx: &LateContext<'_>, definition: DefId) -> bool {
        cx.tcx.crate_name(definition.krate).as_str() == "leptos"
            && cx.tcx.item_name(definition).as_str() == "IntoView"
    }

    /// Recognizes concrete framework wrappers without following blanket rendering traits.
    fn is_view_wrapper(cx: &LateContext<'_>, definition: DefId) -> bool {
        matches!(
            (
                cx.tcx.crate_name(definition.krate).as_str(),
                cx.tcx.item_name(definition).as_str(),
            ),
            ("leptos", "View") | ("tachys", "AnyView")
        )
    }
}
