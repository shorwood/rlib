extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;

use rustc_hir::def_id::LocalDefId;
use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_middle::ty;

// -----------------------------------------------------------------------------
// ImplTargetExt: Direct implementation target queries
// -----------------------------------------------------------------------------

/// Contextual queries about the nominal type targeted by an implementation item.
pub trait ImplTargetExt {
    /// Returns the local struct implemented directly by this item.
    ///
    /// Aliases are followed to their struct, while references and containers are different
    /// self-types and therefore return `None`.
    fn direct_struct(&self, cx: &LateContext<'_>) -> Option<LocalDefId>;
}

impl ImplTargetExt for Item<'_> {
    fn direct_struct(&self, cx: &LateContext<'_>) -> Option<LocalDefId> {
        // Restrict semantic type inspection to implementation items.
        let ItemKind::Impl(_) = self.kind else {
            return None;
        };

        // Resolve the implementation target through aliases before checking its kind.
        let self_type = cx.tcx.type_of(self.owner_id).instantiate_identity();

        // Non-ADT targets cannot resolve to a concrete struct declaration.
        let ty::Adt(adt, _) = self_type.kind() else {
            return None;
        };
        adt.is_struct().then_some(adt.did().as_local()).flatten()
    }
}
