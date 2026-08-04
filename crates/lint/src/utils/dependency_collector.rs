extern crate rustc_hir;
extern crate rustc_middle;

use std::collections::HashSet;

use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::Visitor;
use rustc_hir::{BodyId, ImplItem, Item, ItemKind, intravisit};
use rustc_middle::ty::TyCtxt;

struct DependencyCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    definitions: HashSet<LocalDefId>,
    body_owner: Option<LocalDefId>,
}

impl<'tcx> DependencyCollector<'tcx> {
    fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            definitions: HashSet::new(),
            body_owner: None,
        }
    }
}

impl<'tcx> intravisit::Visitor<'tcx> for DependencyCollector<'tcx> {
    fn visit_path(&mut self, path: &rustc_hir::Path<'tcx>, _: rustc_hir::HirId) {
        if let Some(definition) = path
            .res
            .opt_def_id()
            .and_then(rustc_hir::def_id::DefId::as_local)
        {
            self.definitions.insert(definition);
        }
        intravisit::walk_path(self, path);
    }

    fn visit_nested_body(&mut self, id: BodyId) {
        let previous = self.body_owner.replace(self.tcx.hir_body_owner_def_id(id));
        self.visit_body(self.tcx.hir_body(id));
        self.body_owner = previous;
    }

    fn visit_expr(&mut self, expression: &'tcx rustc_hir::Expr<'tcx>) {
        if matches!(expression.kind, rustc_hir::ExprKind::MethodCall(..))
            && let Some(owner) = self.body_owner
            && let Some(definition) = self
                .tcx
                .typeck(owner)
                .type_dependent_def_id(expression.hir_id)
                .and_then(rustc_hir::def_id::DefId::as_local)
        {
            self.definitions.insert(definition);
        }
        intravisit::walk_expr(self, expression);
    }
}

pub(super) fn collect_item_dependencies<'tcx>(
    tcx: TyCtxt<'tcx>,
    item: &'tcx Item<'tcx>,
) -> HashSet<LocalDefId> {
    let mut collector = DependencyCollector::new(tcx);
    collector.visit_item(item);
    match item.kind {
        ItemKind::Trait(.., items) => {
            for id in items {
                collector.visit_trait_item(collector.tcx.hir_trait_item(*id));
            }
        }
        ItemKind::Impl(implementation) => {
            for id in implementation.items {
                collector.visit_impl_item(collector.tcx.hir_impl_item(*id));
            }
        }
        ItemKind::Mod(_, module) => {
            for id in module.item_ids {
                collector.visit_item(collector.tcx.hir_item(*id));
            }
        }
        ItemKind::ForeignMod { items, .. } => {
            for id in items {
                collector.visit_foreign_item(collector.tcx.hir_foreign_item(*id));
            }
        }
        _ => {}
    }
    collector.definitions
}

pub(super) fn collect_impl_item_dependencies<'tcx>(
    tcx: TyCtxt<'tcx>,
    item: &'tcx ImplItem<'tcx>,
) -> HashSet<LocalDefId> {
    let mut collector = DependencyCollector::new(tcx);
    collector.visit_impl_item(item);
    collector.definitions
}
