extern crate rustc_hir;
extern crate rustc_middle;

use std::collections::HashSet;

use rustc_hir::Item;
use rustc_hir::def_id::LocalDefId;
use rustc_middle::ty::TyCtxt;

use super::dependency_collector::collect_item_dependencies;

/// Resolves references made by an item to declarations in the current crate.
pub(crate) fn item_dependencies<'tcx>(
    tcx: TyCtxt<'tcx>,
    item: &'tcx Item<'tcx>,
) -> HashSet<LocalDefId> {
    collect_item_dependencies(tcx, item)
}
