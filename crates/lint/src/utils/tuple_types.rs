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
        // Reject generated, nested, and compiler-encoded roots before tuple discovery.
        let nested = matches!(cx.tcx.parent_hir_node(ty.hir_id), Node::Ty(_));
        if ty.span.from_expansion() || nested || Self::has_callable_trait_syntax(cx, ty.span) {
            return None;
        }

        // Classify an explicit root tuple without descending into its component types.
        if Self::is_non_unit_tuple(ty) {
            if !Self::has_tuple_syntax(cx, ty.span) {
                return None;
            }
            return Some(Self {
                root_span: ty.span,
                tuple_span: ty.span,
                kind: ExplicitTupleKind::Bare,
            });
        }

        // Find the first nested tuple and confirm that its source uses actual tuple syntax.
        let mut finder = NestedTupleFinder::default();
        intravisit::walk_ty(&mut finder, ty);
        finder.tuple_span.and_then(|tuple_span| {
            let is_authored_tuple = Self::has_tuple_syntax(cx, tuple_span);
            is_authored_tuple.then_some(Self {
                root_span: ty.span,
                tuple_span,
                kind: ExplicitTupleKind::Nested,
            })
        })
    }

    /// Returns whether a source span begins with authored tuple syntax.
    fn has_tuple_syntax(cx: &LateContext<'_>, span: Span) -> bool {
        let source_map = cx.sess().source_map();
        source_map
            .span_to_snippet(span)
            .is_ok_and(|source| source.trim_start().starts_with('('))
    }

    /// Returns whether callable-trait syntax produced the compiler's internal tuple encoding.
    fn has_callable_trait_syntax(cx: &LateContext<'_>, span: Span) -> bool {
        let source_map = cx.sess().source_map();
        source_map.span_to_snippet(span).is_ok_and(|source| {
            source.contains("Fn(") || source.contains("FnMut(") || source.contains("FnOnce(")
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
