extern crate rustc_hir;
extern crate rustc_middle;

use std::collections::HashSet;

use rustc_hir::ImplItem;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::Visitor;
use rustc_middle::ty::TyCtxt;

use super::dependency_collector::DependencyCollector;

/// Resolves references made by an associated impl item to declarations in the current crate.
pub(crate) fn impl_item_dependencies<'tcx>(
    tcx: TyCtxt<'tcx>,
    item: &'tcx ImplItem<'tcx>,
) -> HashSet<LocalDefId> {
    let mut collector = DependencyCollector::new(tcx);
    collector.visit_impl_item(item);
    collector.finish()
}
