extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;

use rustc_hir::{Item, ItemKind, def_id::LocalDefId};
use rustc_lint::LateContext;
use rustc_middle::ty;

/// Returns the local struct implemented directly by an impl block.
///
/// Aliases are followed to their struct, while references and containers are different self-types
/// and therefore return `None`.
pub(crate) fn direct_impl_struct(cx: &LateContext<'_>, item: &Item<'_>) -> Option<LocalDefId> {
    let ItemKind::Impl(_) = item.kind else {
        return None;
    };
    let self_type = cx.tcx.type_of(item.owner_id).instantiate_identity();
    let ty::Adt(adt, _) = self_type.kind() else {
        return None;
    };
    adt.is_struct().then_some(adt.did().as_local()).flatten()
}
