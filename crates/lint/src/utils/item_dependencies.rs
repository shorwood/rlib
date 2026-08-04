extern crate rustc_hir;
extern crate rustc_middle;

use std::collections::HashSet;

use rustc_hir::Item;
use rustc_hir::ItemKind;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::Visitor;
use rustc_middle::ty::TyCtxt;

use super::dependency_collector::DependencyCollector;

/// Resolves references made by an item to declarations in the current crate.
pub(crate) fn item_dependencies<'tcx>(
    tcx: TyCtxt<'tcx>,
    item: &'tcx Item<'tcx>,
) -> HashSet<LocalDefId> {
    let mut collector = DependencyCollector::new(tcx);
    collector.visit_item(item);
    match item.kind {
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
    collector.finish()
}
