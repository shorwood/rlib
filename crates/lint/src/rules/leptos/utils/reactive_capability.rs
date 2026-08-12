extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;
extern crate rustc_trait_selection;

use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty, TypingEnv};
use rustc_span::def_id::{DefId, LocalDefId};
use rustc_trait_selection::infer::TyCtxtInferExt;
use rustc_trait_selection::traits;

// -----------------------------------------------------------------------------
// ReactiveCapability: Leptos reactive authority analysis
// -----------------------------------------------------------------------------

/// Semantic queries for capabilities supplied by the Leptos reactive graph.
pub struct ReactiveCapability;

impl ReactiveCapability {
    /// Returns whether `ty`, optionally through `Option`, is proven to implement `Write`.
    pub fn carries_write<'tcx>(cx: &LateContext<'tcx>, owner: LocalDefId, ty: Ty<'tcx>) -> bool {
        // Preserve absence as transparent while excluding arbitrary capability containers.
        if let Some(inner) = Self::option_inner(cx, ty) {
            return Self::carries_write(cx, owner, inner);
        }
        let Some(write_trait) = Self::write_trait(cx) else {
            return false;
        };

        // Ask the trait solver under the component's own generic typing environment.
        let typing_env = TypingEnv::post_analysis(cx.tcx, owner);
        let (infcx, param_env) = cx.tcx.infer_ctxt().build_with_typing_env(typing_env);
        traits::type_known_to_meet_bound_modulo_regions(&infcx, param_env, ty, write_trait)
    }

    /// Resolves the reactive graph's mutation trait by semantic crate and item identity.
    fn write_trait(cx: &LateContext<'_>) -> Option<DefId> {
        cx.tcx.all_traits_including_private().find(|trait_id| {
            cx.tcx.crate_name(trait_id.krate).as_str() == "reactive_graph"
                && cx.tcx.item_name(*trait_id).as_str() == "Write"
        })
    }

    /// Extracts the represented value from the standard option type.
    fn option_inner<'tcx>(cx: &LateContext<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
        let ty::Adt(definition, arguments) = ty.kind() else {
            return None;
        };
        let def_id = definition.did();
        (cx.tcx.crate_name(def_id.krate).as_str() == "core"
            && cx.tcx.item_name(def_id).as_str() == "Option")
            .then(|| arguments.type_at(0))
    }
}
