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
/// Relationship between a reported tuple and its explicit root type.
#[derive(Clone, Copy, Eq, PartialEq)]
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

        // Only authored root annotations represent tuple API choices owned by this analysis.
        if ty.span.from_expansion() || nested || Self::is_callable_argument_tuple(cx, ty) {
            return None;
        }

        // Classify an explicit root tuple without descending into its component types.
        if Self::is_non_unit_tuple(ty) {
            // Compiler tuple encodings without authored parentheses are not explicit tuple syntax.
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
        let mut finder = NestedTupleFinder {
            cx,
            tuple_span: None,
        };
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

    /// Returns whether this tuple is the compiler encoding of callable-trait arguments.
    fn is_callable_argument_tuple(cx: &LateContext<'_>, ty: &Ty<'_, AmbigArg>) -> bool {
        // Non-tuple types cannot be callable-trait argument tuple encodings.
        if !Self::is_non_unit_tuple(ty) {
            return false;
        }
        let owner = cx.tcx.hir_get_parent_item(ty.hir_id).def_id;
        let owner_span = cx.tcx.def_span(owner);

        // A tuple preceding its owner cannot have a callable-trait prefix in that owner source.
        if owner_span.lo() > ty.span.lo() {
            return false;
        }
        let prefix = ty.span.with_lo(owner_span.lo()).with_hi(ty.span.lo());
        cx.sess()
            .source_map()
            .span_to_snippet(prefix)
            .is_ok_and(|source| {
                let source = source.trim_end();
                source.ends_with("Fn") || source.ends_with("FnMut") || source.ends_with("FnOnce")
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
/// HIR visitor that records the first tuple beneath an explicit root type.
struct NestedTupleFinder<'cx, 'tcx> {
    /// Compiler context used to distinguish callable argument encoding from tuple syntax.
    cx: &'cx LateContext<'tcx>,
    /// Exact span of the first nested non-unit tuple.
    tuple_span: Option<Span>,
}

impl<'hir> Visitor<'hir> for NestedTupleFinder<'_, '_> {
    fn visit_ty(&mut self, ty: &'hir Ty<'hir, AmbigArg>) {
        // The first authored nested tuple completes discovery for this root annotation.
        if self.tuple_span.is_none()
            && ExplicitTupleType::is_non_unit_tuple(ty)
            && !ExplicitTupleType::is_callable_argument_tuple(self.cx, ty)
        {
            self.tuple_span = Some(ty.span);
            return;
        }
        intravisit::walk_ty(self, ty);
    }
}
