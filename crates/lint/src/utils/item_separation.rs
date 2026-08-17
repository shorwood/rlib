extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::HirId;
use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;

// -----------------------------------------------------------------------------
// Source: Authored declaration identity
// -----------------------------------------------------------------------------

/// Source identity needed to compare one authored item with its successor.
pub struct Source {
    /// HIR node used to respect lint levels on the following item.
    hir_id: HirId,
    /// Declaration span used to locate the authored boundary.
    span: Span,
    /// Human-readable item name used in remediation guidance.
    name: String,
}

impl Source {
    /// Captures one item participating in separation analysis.
    pub const fn new(hir_id: HirId, span: Span, name: String) -> Self {
        Self { hir_id, span, name }
    }
}

// -----------------------------------------------------------------------------
// Finding: Dense authored boundary
// -----------------------------------------------------------------------------

/// Adjacent authored items lacking a visually empty line between them.
pub struct Finding {
    /// Following item used to respect its local lint level.
    pub hir_id: HirId,
    /// Following declaration highlighted as the dense boundary.
    pub span: Span,
    /// Name of the item immediately before the boundary.
    pub previous_name: String,
    /// Name of the item immediately after the boundary.
    pub following_name: String,
    /// Zero-width insertion point when leading context can retain its ownership.
    pub insertion: Option<Span>,
}

// -----------------------------------------------------------------------------
// Analyzer: Physical source-gap inspection
// -----------------------------------------------------------------------------

/// Shared source analyzer for authored item boundaries.
pub struct Analyzer;

impl Analyzer {
    /// Finds every adjacent authored pair without an intervening empty line.
    pub fn findings(cx: &LateContext<'_>, items: &[Source]) -> Vec<Finding> {
        let source_map = cx.sess().source_map();
        let mut findings = Vec::new();
        for pair in items.windows(2) {
            let [previous, following] = pair else {
                continue;
            };
            if previous.span.from_expansion() || following.span.from_expansion() {
                continue;
            }
            let gap = Span::with_root_ctxt(previous.span.hi(), following.span.lo());
            let Ok(source) = source_map.span_to_snippet(gap) else {
                continue;
            };
            if Self::has_blank_line(&source) {
                continue;
            }
            findings.push(Finding {
                hir_id: following.hir_id,
                span: following.span,
                previous_name: previous.name.clone(),
                following_name: following.name.clone(),
                insertion: Self::has_safe_insertion(&source).then(|| previous.span.shrink_to_hi()),
            });
        }
        findings
    }

    /// Returns whether the source gap contains a complete visually empty line.
    fn has_blank_line(source: &str) -> bool {
        let mut saw_line_break = false;
        let mut line_is_empty = true;
        for byte in source.bytes() {
            if byte != b'\n' {
                line_is_empty &= !saw_line_break || matches!(byte, b' ' | b'\t' | b'\r');
                continue;
            }
            // A second empty physical line establishes the required visual boundary.
            if saw_line_break && line_is_empty {
                return true;
            }
            saw_line_break = true;
            line_is_empty = true;
        }
        false
    }

    /// Returns whether inserting after the previous item preserves comment ownership.
    fn has_safe_insertion(source: &str) -> bool {
        // Same-line boundaries cannot accept a standalone blank line safely.
        let Some((same_line, following_lines)) = source.split_once('\n') else {
            return false;
        };
        // Trailing tokens may own the following line and prohibit a blind insertion.
        if !same_line.trim().is_empty() {
            return false;
        }
        !following_lines.lines().any(|line| {
            let line = line.trim_start();
            (line.starts_with("//") && !line.starts_with("///"))
                || (line.starts_with("/*") && !line.starts_with("/**"))
        })
    }
}
