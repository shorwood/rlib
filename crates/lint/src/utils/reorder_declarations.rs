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
        if nodes.len() != order.len() || nodes.iter().any(|node| node.span.from_expansion()) {
            return None;
        }
        let source_map = cx.sess().source_map();
        let file = source_map.span_to_filename(nodes.first()?.span);
        if nodes
            .iter()
            .any(|node| source_map.span_to_filename(node.span) != file)
        {
            return None;
        }

        let mut by_source = (0..nodes.len()).collect::<Vec<_>>();
        by_source.sort_by_key(|index| nodes[*index].span.lo());
        for pair in by_source.windows(2) {
            let gap = source_map
                .span_to_snippet(Span::with_root_ctxt(
                    nodes[pair[0]].span.hi(),
                    nodes[pair[1]].span.lo(),
                ))
                .ok()?;
            if !gap.contains("//") && !gap.contains("/*") && !gap.contains("macro_rules!") {
                continue;
            }
            return None;
        }

        let snippets = order
            .iter()
            .map(|index| source_map.span_to_snippet(nodes[*index].span).ok())
            .collect::<Option<Vec<_>>>()?;
        let targets = by_source.iter().zip(snippets);
        let edits = targets.map(|(target, replacement)| DeclarationOrderEdit {
            span: nodes[*target].span,
            replacement,
        });
        Some(edits.collect())
    }
}
