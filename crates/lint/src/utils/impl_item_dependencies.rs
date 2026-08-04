extern crate rustc_hir;
extern crate rustc_middle;

use std::collections::HashSet;

use rustc_hir::ImplItem;
use rustc_hir::def_id::LocalDefId;
use rustc_middle::ty::TyCtxt;

use super::dependency_collector::collect_impl_item_dependencies;

/// Resolves references made by an associated impl item to declarations in the current crate.
pub(crate) fn impl_item_dependencies<'tcx>(
    tcx: TyCtxt<'tcx>,
    item: &'tcx ImplItem<'tcx>,
) -> HashSet<LocalDefId> {
    collect_impl_item_dependencies(tcx, item)
}
