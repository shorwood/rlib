extern crate rustc_lint;
extern crate rustc_span;

use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;

use super::declaration_node::DeclarationNode;

/// Builds an atomic permutation suggestion when every declaration has plainly owned source.
pub(crate) fn reorder_declarations(
    cx: &LateContext<'_>,
    nodes: &[DeclarationNode],
    order: &[usize],
) -> Option<Vec<(Span, String)>> {
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
        if gap.contains("//") || gap.contains("/*") || gap.contains("macro_rules!") {
            return None;
        }
    }

    let snippets = order
        .iter()
        .map(|index| source_map.span_to_snippet(nodes[*index].span).ok())
        .collect::<Option<Vec<_>>>()?;
    Some(
        by_source
            .iter()
            .zip(snippets)
            .map(|(target, replacement)| (nodes[*target].span, replacement))
            .collect(),
    )
}
