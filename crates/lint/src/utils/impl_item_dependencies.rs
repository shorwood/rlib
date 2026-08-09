extern crate rustc_hir;
extern crate rustc_middle;

use std::collections::HashSet;

use rustc_hir::ImplItem;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::Visitor;
use rustc_middle::ty::TyCtxt;

use super::dependency_collector::DependencyCollector;

// -----------------------------------------------------------------------------
// DependenciesExt: Associated item dependency queries
// -----------------------------------------------------------------------------

/// Dependency queries colocated with compiler impl items.
pub trait DependenciesExt {
    /// Resolves references made by this impl item to declarations in the current crate.
    fn dependencies<'tcx>(&'tcx self, tcx: TyCtxt<'tcx>) -> HashSet<LocalDefId>;
}

impl DependenciesExt for ImplItem<'_> {
    fn dependencies<'tcx>(&'tcx self, tcx: TyCtxt<'tcx>) -> HashSet<LocalDefId> {
        let mut collector = DependencyCollector::new(tcx);
        collector.visit_impl_item(self);
        collector.finish()
    }
}
