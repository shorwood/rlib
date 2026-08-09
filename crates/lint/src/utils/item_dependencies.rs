extern crate rustc_hir;
extern crate rustc_middle;

use std::collections::HashSet;

use rustc_hir::Item;
use rustc_hir::ItemKind;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::Visitor;
use rustc_middle::ty::TyCtxt;

use super::dependency_collector::DependencyCollector;

// -----------------------------------------------------------------------------
// DependenciesExt: Item dependency queries
// -----------------------------------------------------------------------------

/// Dependency queries colocated with compiler items.
pub trait DependenciesExt {
    /// Resolves references made by this item and its owned declarations in the current crate.
    fn dependencies<'tcx>(&'tcx self, tcx: TyCtxt<'tcx>) -> HashSet<LocalDefId>;
}

impl DependenciesExt for Item<'_> {
    fn dependencies<'tcx>(&'tcx self, tcx: TyCtxt<'tcx>) -> HashSet<LocalDefId> {
        // Collect references from the declaration and all owned associated items.
        let mut collector = DependencyCollector::new(tcx);
        collector.visit_item(self);

        // Traverse associated declarations owned by the item's outer HIR node.
        match self.kind {
            ItemKind::Trait(.., items) => {
                for id in items {
                    collector.visit_trait_item(tcx.hir_trait_item(*id));
                }
            }
            ItemKind::Impl(implementation) => {
                for id in implementation.items {
                    collector.visit_impl_item(tcx.hir_impl_item(*id));
                }
            }
            ItemKind::Mod(_, module) => {
                for id in module.item_ids {
                    collector.visit_item(tcx.hir_item(*id));
                }
            }
            ItemKind::ForeignMod { items, .. } => {
                for id in items {
                    collector.visit_foreign_item(tcx.hir_foreign_item(*id));
                }
            }
            _ => {}
        }

        // Return the identities accumulated across the complete declaration group.
        collector.finish()
    }
}
