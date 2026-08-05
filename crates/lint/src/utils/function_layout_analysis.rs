extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Block, Expr, ExprKind, Stmt, StmtKind};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;

use super::config::FunctionStructureConfig;
use super::function_layout_comments::{FunctionLayoutEntryGap, FunctionLayoutFinding};

// -----------------------------------------------------------------------------
// FunctionLayout: Analyze linear code phases
// -----------------------------------------------------------------------------

/// Phase-comment findings separated by public lint identity.
#[derive(Default)]
pub(crate) struct FunctionLayoutAnalysis {
    pub(crate) missing: Vec<FunctionLayoutFinding>,
    pub(crate) malformed: Vec<FunctionLayoutFinding>,
}

struct FunctionLayoutBoundary {
    has_header: bool,
    has_run_boundary: bool,
}

pub(super) fn function_layout_code_line_count(cx: &LateContext<'_>, span: Span) -> usize {
    let span = span.source_callsite();
    let Ok(source) = cx.sess().source_map().span_to_snippet(span) else {
        return 0;
    };
    let mut code_lines = vec![false; source.lines().count().max(1)];

    // Mark every physical line touched by a non-comment source token.
    let mut offset = 0;
    for token in tokenize(&source, FrontmatterAllowed::No) {
        let length = usize::try_from(token.len).expect("token length should fit usize");
        let end = offset + length;
        if !matches!(
            token.kind,
            TokenKind::Whitespace | TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
        ) {
            let start_line = source[..offset]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count();
            let end_line = source[..end].bytes().filter(|byte| *byte == b'\n').count();
            let end_line = end_line.min(code_lines.len() - 1);
            code_lines[start_line..=end_line].fill(true);
        }
        offset = end;
    }
    code_lines.into_iter().filter(|has_code| *has_code).count()
}

fn function_layout_is_control_boundary(expression: &Expr<'_>) -> bool {
    expression.span.desugaring_kind().is_some()
        || matches!(
            expression.kind,
            ExprKind::If(..)
                | ExprKind::Loop(..)
                | ExprKind::Match(..)
                | ExprKind::Block(..)
                | ExprKind::Closure(..)
        )
}

#[derive(Clone, Copy)]
struct FunctionLayoutEntry {
    span: Span,
    is_linear: bool,
}

impl FunctionLayoutEntry {
    fn from_statement(statement: &Stmt<'_>) -> Self {
        let is_linear = match statement.kind {
            StmtKind::Let(local) => local
                .init
                .is_none_or(|expression| !function_layout_is_control_boundary(expression)),
            StmtKind::Expr(expression) | StmtKind::Semi(expression) => {
                !function_layout_is_control_boundary(expression)
            }
            StmtKind::Item(_) => false,
        };
        Self {
            span: statement.span,
            is_linear,
        }
    }

    fn from_expression(expression: &Expr<'_>) -> Self {
        Self {
            span: expression.span,
            is_linear: !function_layout_is_control_boundary(expression),
        }
    }
}

pub(super) struct FunctionLayoutAnalyzer<'analysis, 'tcx> {
    cx: &'analysis LateContext<'tcx>,
    config: &'analysis FunctionStructureConfig,
    analysis: FunctionLayoutAnalysis,
}

impl<'analysis, 'tcx> FunctionLayoutAnalyzer<'analysis, 'tcx> {
    pub(super) fn new(
        cx: &'analysis LateContext<'tcx>,
        config: &'analysis FunctionStructureConfig,
    ) -> Self {
        Self {
            cx,
            config,
            analysis: FunctionLayoutAnalysis::default(),
        }
    }

    pub(super) fn analyze(mut self, expression: &'tcx Expr<'tcx>) -> FunctionLayoutAnalysis {
        self.visit_expr(expression);
        self.analysis
    }

    fn analyze_trailing_gap(&mut self, block: &Block<'_>, entries: &[FunctionLayoutEntry]) {
        let last = entries.last().expect("entries are nonempty").span;
        let gap = block.span.with_lo(last.hi());
        let trailing = FunctionLayoutEntryGap::analyze(self.cx, self.config, gap, Some(last), None);
        self.analysis.malformed.extend(trailing.findings);
    }

    fn record_oversized_phase(
        &mut self,
        phase: &[FunctionLayoutEntry],
        lines: usize,
        has_header: bool,
    ) {
        let first = phase[0].span.source_callsite();
        let last = phase[phase.len() - 1].span.source_callsite();
        let span = Span::with_root_ctxt(first.lo(), last.hi());
        let message = if has_header {
            format!(
                "this code phase contains {lines} lines, exceeding the configured maximum of {}",
                self.config.max_phase_lines
            )
        } else {
            format!("this {lines}-line code phase has no explanatory comment")
        };
        self.analysis.missing.push(FunctionLayoutFinding {
            span,
            message,
            help: format!(
                "divide the work with `{}` explanatory comments or extract named operations",
                self.config.phase_comment_prefix
            ),
            replacement: None,
        });
    }

    fn analyze_linear_run(
        &mut self,
        entries: &[FunctionLayoutEntry],
        headers: &[bool],
        boundaries: &[bool],
    ) {
        let line_counts = entries
            .iter()
            .map(|entry| function_layout_code_line_count(self.cx, entry.span))
            .collect::<Vec<_>>();
        if line_counts.iter().sum::<usize>() <= self.config.max_phase_lines {
            return;
        }

        // Treat every valid explanation as the start of a distinct semantic phase.
        let mut phase_start = 0;
        for index in 1..=entries.len() {
            if index < entries.len() && !boundaries[index] {
                continue;
            }
            let lines = line_counts[phase_start..index].iter().sum::<usize>();
            let has_header = headers[phase_start];
            let phase = &entries[phase_start..index];
            if phase.len() > 1 && lines > self.config.max_phase_lines {
                self.record_oversized_phase(phase, lines, has_header);
            }
            phase_start = index;
        }
    }

    fn analyze_entry_gap(
        &mut self,
        block: &Block<'_>,
        entries: &[FunctionLayoutEntry],
        index: usize,
        next: Span,
    ) -> FunctionLayoutBoundary {
        // Resolve the source gap between this entry and its predecessor.
        let previous = index.checked_sub(1).map(|index| entries[index].span);
        let lo = previous.map_or(block.span.lo(), Span::hi);
        let gap = block.span.with_lo(lo).with_hi(next.lo());

        // Preserve the comment parser's semantic and physical boundary decisions.
        let analyzed =
            FunctionLayoutEntryGap::analyze(self.cx, self.config, gap, previous, Some(next));
        let boundary = FunctionLayoutBoundary {
            has_header: analyzed.has_valid_header,
            has_run_boundary: analyzed.has_run_boundary,
        };

        // Merge malformed comments before returning the lightweight boundary state.
        self.analysis.malformed.extend(analyzed.findings);
        boundary
    }

    fn analyze_block(&mut self, block: &'tcx Block<'tcx>) {
        if block.span.from_expansion() {
            return;
        }
        let mut entries = block
            .stmts
            .iter()
            .map(FunctionLayoutEntry::from_statement)
            .collect::<Vec<_>>();
        if let Some(expression) = block.expr {
            entries.push(FunctionLayoutEntry::from_expression(expression));
        }
        if entries.is_empty() {
            return;
        }

        // Validate each entry's leading gap while recording valid phase boundaries.
        let mut headers = Vec::with_capacity(entries.len());
        let mut boundaries = Vec::with_capacity(entries.len());
        for (index, entry) in entries.iter().enumerate() {
            let boundary = self.analyze_entry_gap(block, &entries, index, entry.span);
            headers.push(boundary.has_header);
            boundaries.push(boundary.has_run_boundary);
        }
        self.analyze_trailing_gap(block, &entries);

        // Analyze only uninterrupted linear statements; control flow carries its own structure.
        let mut start = 0;
        while start < entries.len() {
            if !entries[start].is_linear {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < entries.len() && entries[end].is_linear {
                end += 1;
            }
            self.analyze_linear_run(
                &entries[start..end],
                &headers[start..end],
                &boundaries[start..end],
            );
            start = end;
        }
    }
}

impl<'tcx> Visitor<'tcx> for FunctionLayoutAnalyzer<'_, 'tcx> {
    fn visit_block(&mut self, block: &'tcx Block<'tcx>) {
        self.analyze_block(block);
        intravisit::walk_block(self, block);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if matches!(expression.kind, ExprKind::Closure(..)) {
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}
