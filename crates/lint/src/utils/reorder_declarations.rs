extern crate rustc_lint;
extern crate rustc_span;

use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;

use super::declaration_node::DeclarationNode;

// -----------------------------------------------------------------------------
// DeclarationOrder: Atomic declaration reordering
// -----------------------------------------------------------------------------

/// One source replacement in an atomic declaration permutation.
pub struct DeclarationOrderEdit {
    /// Existing declaration span that receives reordered source text.
    pub(crate) span: Span,
    /// Complete declaration snippet moved into `span`.
    pub(crate) replacement: String,
}

/// Builder for conservative whole-declaration reorder suggestions.
pub struct DeclarationOrder;

impl DeclarationOrder {
    /// Builds an atomic permutation suggestion when every declaration has plainly owned source.
    pub(crate) fn edits(
        cx: &LateContext<'_>,
        nodes: &[DeclarationNode],
        order: &[usize],
    ) -> Option<Vec<DeclarationOrderEdit>> {
        // Reject incomplete, expanded, or cross-file declaration groups.
        if nodes.len() != order.len() || nodes.iter().any(|node| node.source.span.from_expansion())
        {
            return None;
        }
        let source_map = cx.sess().source_map();
        let file = source_map.span_to_filename(nodes.first()?.source.span);

        // Cross-file declaration groups cannot be permuted by one atomic source suggestion.
        if nodes
            .iter()
            .any(|node| source_map.span_to_filename(node.source.span) != file)
        {
            return None;
        }

        // Preserve comments and macro definitions that sit between declarations.
        let mut by_source = (0..nodes.len()).collect::<Vec<_>>();
        by_source.sort_by_key(|index| nodes[*index].source.span.lo());
        for pair in by_source.windows(2) {
            // Inspect the exact authored gap between adjacent source declarations.
            let gap_span = Span::with_root_ctxt(
                nodes[pair[0]].source.span.hi(),
                nodes[pair[1]].source.span.lo(),
            );

            // An unavailable inter-declaration gap cannot prove that reordering preserves source.
            let Ok(gap) = source_map.span_to_snippet(gap_span) else {
                return None;
            };

            // Authored comments or macros in a gap have ambiguous ownership under reordering.
            if gap.contains("//") || gap.contains("/*") || gap.contains("macro_rules!") {
                return None;
            }
        }

        // Pair reordered snippets with the original source-ordered target spans.
        let snippets = order
            .iter()
            .map(|index| {
                source_map
                    .span_to_snippet(nodes[*index].source.span)
                    .map_err(|_| ())
            })
            .collect::<Result<Vec<_>, _>>();

        // Every declaration snippet must be available before constructing an atomic permutation.
        let Ok(snippets) = snippets else {
            return None;
        };

        // Construct one replacement for each original source position.
        let targets = by_source.iter().zip(snippets);
        let edits = targets.map(|(target, replacement)| DeclarationOrderEdit {
            span: nodes[*target].source.span,
            replacement,
        });
        Some(edits.collect())
    }
}
