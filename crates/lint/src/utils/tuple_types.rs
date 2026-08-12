extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{AmbigArg, Node, Ty, TyKind};
use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;

// -----------------------------------------------------------------------------
// ExplicitTuple: Tuple classification for authored type annotations
// -----------------------------------------------------------------------------

#[derive(Clone, Copy, Eq, PartialEq)]
/// Relationship between a reported tuple and its explicit root type.
pub enum ExplicitTupleKind {
    /// The explicit root annotation is itself a non-unit tuple.
    Bare,
    /// A non-unit tuple appears beneath another explicit type constructor.
    Nested,
}

/// First reportable tuple found in one authored root type annotation.
pub struct ExplicitTupleType {
    /// Complete explicit root type used for diagnostic context.
    pub(crate) root_span: Span,
    /// Exact non-unit tuple span that violates the selected rule.
    pub(crate) tuple_span: Span,
    /// Whether the tuple is the root or nested beneath it.
    pub(crate) kind: ExplicitTupleKind,
}

impl ExplicitTupleType {
    /// Classifies an authored root type and locates its first non-unit tuple.
    pub(crate) fn classify(cx: &LateContext<'_>, ty: &Ty<'_, AmbigArg>) -> Option<Self> {
        if ty.span.from_expansion() || matches!(cx.tcx.parent_hir_node(ty.hir_id), Node::Ty(_)) {
            return None;
        }
        // Rust represents callable-trait arguments as a tuple internally even when the author
        // wrote ordinary `Fn(A, B)` syntax. That compiler encoding is not an authored tuple API.
        if cx
            .sess()
            .source_map()
            .span_to_snippet(ty.span)
            .is_ok_and(|source| {
                source.contains("Fn(") || source.contains("FnMut(") || source.contains("FnOnce(")
            })
        {
            return None;
        }
        if Self::is_non_unit_tuple(ty) {
            if cx
                .sess()
                .source_map()
                .span_to_snippet(ty.span)
                .is_ok_and(|source| !source.trim_start().starts_with('('))
            {
                return None;
            }
            return Some(Self {
                root_span: ty.span,
                tuple_span: ty.span,
                kind: ExplicitTupleKind::Bare,
            });
        }

        let mut finder = NestedTupleFinder::default();
        intravisit::walk_ty(&mut finder, ty);
        finder.tuple_span.and_then(|tuple_span| {
            let is_authored_tuple = cx
                .sess()
                .source_map()
                .span_to_snippet(tuple_span)
                .is_ok_and(|source| source.trim_start().starts_with('('));
            is_authored_tuple.then_some(Self {
                root_span: ty.span,
                tuple_span,
                kind: ExplicitTupleKind::Nested,
            })
        })
    }

    /// Returns whether a HIR type is a tuple with at least one element.
    const fn is_non_unit_tuple(ty: &Ty<'_, AmbigArg>) -> bool {
        matches!(ty.kind, TyKind::Tup(elements) if !elements.is_empty())
    }
}

// -----------------------------------------------------------------------------
// NestedTupleFinder: First tuple beneath an explicit root type
// -----------------------------------------------------------------------------

#[derive(Default)]
/// HIR visitor that records the first tuple beneath an explicit root type.
struct NestedTupleFinder {
    /// Exact span of the first nested non-unit tuple.
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
