extern crate rustc_hir;
extern crate rustc_middle;

use std::collections::HashSet;

use rustc_hir::def_id::LocalDefId;
use rustc_hir::{BodyId, intravisit};
use rustc_middle::ty::TyCtxt;

pub(super) struct DependencyCollector<'tcx> {
    tcx: TyCtxt<'tcx>,
    definitions: HashSet<LocalDefId>,
    body_owner: Option<LocalDefId>,
}

impl<'tcx> DependencyCollector<'tcx> {
    pub(super) fn new(tcx: TyCtxt<'tcx>) -> Self {
        Self {
            tcx,
            definitions: HashSet::new(),
            body_owner: None,
        }
    }

    pub(super) fn finish(self) -> HashSet<LocalDefId> {
        self.definitions
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
