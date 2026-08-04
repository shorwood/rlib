extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{AmbigArg, Node, Ty, TyKind};
use rustc_lint::LateContext;
use rustc_span::Span;

// -----------------------------------------------------------------------------
// ExplicitTuple: Tuple classification for authored type annotations
// -----------------------------------------------------------------------------

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum ExplicitTupleKind {
    Bare,
    Nested,
}

pub(crate) struct ExplicitTupleType {
    pub(crate) root_span: Span,
    pub(crate) tuple_span: Span,
    pub(crate) kind: ExplicitTupleKind,
}

impl ExplicitTupleType {
    pub(crate) fn classify(cx: &LateContext<'_>, ty: &Ty<'_, AmbigArg>) -> Option<Self> {
        if ty.span.from_expansion() || matches!(cx.tcx.parent_hir_node(ty.hir_id), Node::Ty(_)) {
            return None;
        }
        if Self::is_non_unit_tuple(ty) {
            return Some(Self {
                root_span: ty.span,
                tuple_span: ty.span,
                kind: ExplicitTupleKind::Bare,
            });
        }

        let mut finder = NestedTupleFinder::default();
        intravisit::walk_ty(&mut finder, ty);
        finder.tuple_span.map(|tuple_span| Self {
            root_span: ty.span,
            tuple_span,
            kind: ExplicitTupleKind::Nested,
        })
    }

    fn is_non_unit_tuple(ty: &Ty<'_, AmbigArg>) -> bool {
        matches!(ty.kind, TyKind::Tup(elements) if !elements.is_empty())
    }
}

// -----------------------------------------------------------------------------
// NestedTupleFinder: First tuple beneath an explicit root type
// -----------------------------------------------------------------------------

#[derive(Default)]
struct NestedTupleFinder {
    tuple_span: Option<Span>,
}

impl<'hir> Visitor<'hir> for NestedTupleFinder {
    fn visit_ty(&mut self, ty: &'hir Ty<'hir, AmbigArg>) {
        if self.tuple_span.is_none() && ExplicitTupleType::is_non_unit_tuple(ty) {
            self.tuple_span = Some(ty.span);
            return;
        }
        intravisit::walk_ty(self, ty);
    }
}
