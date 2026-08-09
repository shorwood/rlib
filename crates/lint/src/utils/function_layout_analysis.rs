extern crate rustc_hir;
extern crate rustc_lexer;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Block, Expr, ExprKind, MatchSource, Stmt, StmtKind};
use rustc_lexer::{FrontmatterAllowed, TokenKind, tokenize};
use rustc_lint::{LateContext, LintContext};
use rustc_span::{BytePos, Span};

use super::config::FunctionStructureConfig;
use super::function_layout_comments::{FunctionLayoutEntryGap, FunctionLayoutFinding};

// -----------------------------------------------------------------------------
// FunctionLayout: Analyze direct code phases
// -----------------------------------------------------------------------------

/// Phase-comment findings separated by public lint identity.
#[derive(Default)]
pub struct FunctionLayoutAnalysis {
    /// Oversized direct phases lacking sufficient semantic decomposition.
    pub(crate) missing: Vec<FunctionLayoutFinding>,
    /// Authored phase comments that violate syntax or placement rules.
    pub(crate) malformed: Vec<FunctionLayoutFinding>,
}

/// Whether a measured code phase already has an explanatory header.
#[derive(Clone, Copy)]
enum FunctionLayoutPhaseHeader {
    /// A canonical phase comment precedes the code.
    Present,
    /// The phase is currently unnamed.
    Missing,
}

impl FunctionLayoutPhaseHeader {
    /// Classifies whether an analyzed entry gap supplies a valid phase header.
    const fn from_entry_gap(gap: &FunctionLayoutEntryGap) -> Self {
        if gap.has_valid_header {
            Self::Present
        } else {
            Self::Missing
        }
    }
}

// -----------------------------------------------------------------------------
// FunctionLayoutNestedSpanCollector: Nested body collection
// -----------------------------------------------------------------------------

/// Finds immediate nested bodies whose code belongs to a child layout scope.
struct FunctionLayoutNestedSpanCollector<'analysis, 'tcx> {
    /// Compiler context used to resolve closure bodies.
    cx: &'analysis LateContext<'tcx>,
    /// Outermost nested spans found inside one direct entry.
    spans: Vec<Span>,
}

impl<'analysis, 'tcx> FunctionLayoutNestedSpanCollector<'analysis, 'tcx> {
    /// Collects nested bodies owned by one statement.
    fn collect_statement(
        cx: &'analysis LateContext<'tcx>,
        statement: &'tcx Stmt<'tcx>,
    ) -> Vec<Span> {
        let mut collector = Self {
            cx,
            spans: Vec::new(),
        };
        intravisit::walk_stmt(&mut collector, statement);
        collector.spans
    }

    /// Collects nested bodies owned by one expression.
    fn collect_expression(
        cx: &'analysis LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
    ) -> Vec<Span> {
        let mut collector = Self {
            cx,
            spans: Vec::new(),
        };
        intravisit::walk_expr(&mut collector, expression);
        collector.spans
    }

    /// Returns whether a lexer token contributes authored code to a physical line.
    const fn is_code_token(kind: TokenKind) -> bool {
        !matches!(
            kind,
            TokenKind::Whitespace | TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
        )
    }

    /// Counts source lines after masking code owned by independently analyzed child bodies.
    fn filtered_line_count(cx: &LateContext<'_>, span: Span, nested_spans: &[Span]) -> usize {
        // Load the authored source and initialize its per-line token mask.
        let span = span.source_callsite();
        let Ok(source) = cx.sess().source_map().span_to_snippet(span) else {
            return 0;
        };
        let mut code_lines = vec![false; source.lines().count().max(1)];

        // Traverse the source tokens in byte order.
        let mut offset = 0;
        for token in tokenize(&source, FrontmatterAllowed::No) {
            // Classify the token by its absolute source range and owning layout scope.
            let length = usize::try_from(token.len).expect("token length should fit usize");
            let end = offset + length;
            let token_lo = span.lo() + BytePos(u32::try_from(offset).expect("offset should fit"));
            let token_hi = span.lo() + BytePos(u32::try_from(end).expect("offset should fit"));

            // Determine whether an independently analyzed child body owns this token.
            let is_nested = nested_spans
                .iter()
                .map(|nested| nested.source_callsite())
                .any(|nested| nested.lo() <= token_lo && token_hi <= nested.hi());

            // Mark physical lines touched by direct, non-comment source tokens.
            if !is_nested && Self::is_code_token(token.kind) {
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
}

impl<'tcx> Visitor<'tcx> for FunctionLayoutNestedSpanCollector<'_, 'tcx> {
    fn visit_block(&mut self, block: &'tcx Block<'tcx>) {
        self.spans.push(block.span.source_callsite());
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Closure(closure) = expression.kind {
            let body = self.cx.tcx.hir_body(closure.body);
            self.spans.push(body.value.span.source_callsite());
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// FunctionLayoutEntry: Direct phase entry
// -----------------------------------------------------------------------------

/// One statement or tail expression participating in direct-phase analysis.
struct FunctionLayoutEntry {
    /// Authored source range used for line counting and gap construction.
    span: Span,
    /// Nested authored bodies excluded from the containing block's line count.
    nested_spans: Vec<Span>,
    /// Whether this entry is a declarative literal mapping counted as one operation.
    is_declarative_mapping: bool,
}

impl FunctionLayoutEntry {
    /// Collects nested bodies from a statement without descending into those bodies.
    fn from_statement<'tcx>(cx: &LateContext<'tcx>, statement: &'tcx Stmt<'tcx>) -> Self {
        // Treat nested items as entirely independent; otherwise collect child bodies.
        let nested_spans = match statement.kind {
            StmtKind::Item(_) => vec![statement.span.source_callsite()],
            _ => FunctionLayoutNestedSpanCollector::collect_statement(cx, statement),
        };

        // Classify a direct expression independently from its nested source bodies.
        let is_declarative_mapping = match statement.kind {
            StmtKind::Expr(expression) | StmtKind::Semi(expression) => {
                Self::is_declarative_mapping(expression)
            }
            StmtKind::Let(_) | StmtKind::Item(_) => false,
        };

        // Retain the complete entry span for comments and surface line counting.
        Self {
            span: statement.span,
            nested_spans,
            is_declarative_mapping,
        }
    }

    /// Collects nested bodies from a block tail expression.
    fn from_expression<'tcx>(cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) -> Self {
        Self {
            span: expression.span,
            nested_spans: FunctionLayoutNestedSpanCollector::collect_expression(cx, expression),
            is_declarative_mapping: Self::is_declarative_mapping(expression),
        }
    }

    /// Returns whether an expression contains only a literal value and transparent blocks.
    fn is_literal_value(expression: &Expr<'_>) -> bool {
        match expression.kind {
            ExprKind::Lit(_) => true,
            ExprKind::Block(block, _) => {
                block.stmts.is_empty() && block.expr.is_some_and(Self::is_literal_value)
            }
            _ => false,
        }
    }

    /// Recognizes authored matches that only map patterns to literal values.
    fn is_declarative_mapping(expression: &Expr<'_>) -> bool {
        let ExprKind::Match(_, arms, MatchSource::Normal | MatchSource::Postfix) = expression.kind
        else {
            return false;
        };
        arms.iter()
            .all(|arm| arm.guard.is_none() && Self::is_literal_value(arm.body))
    }

    /// Counts authored code on the containing block's surface.
    fn direct_line_count(&self, cx: &LateContext<'_>) -> usize {
        if self.is_declarative_mapping {
            return 1;
        }
        FunctionLayoutNestedSpanCollector::filtered_line_count(cx, self.span, &self.nested_spans)
    }
}

// -----------------------------------------------------------------------------
// FunctionLayoutAnalyzer: Phase analysis
// -----------------------------------------------------------------------------

/// HIR visitor that validates phase comments and continuous linear code runs.
pub(super) struct FunctionLayoutAnalyzer<'analysis, 'tcx> {
    /// Compiler context used for source snippets and physical line mapping.
    cx: &'analysis LateContext<'tcx>,
    /// Validated phase-comment syntax and size limits.
    config: &'analysis FunctionStructureConfig,
    /// Findings accumulated during traversal.
    analysis: FunctionLayoutAnalysis,
}

impl<'analysis, 'tcx> FunctionLayoutAnalyzer<'analysis, 'tcx> {
    /// Starts an empty layout analysis for one named function.
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

    /// Traverses the function expression and returns findings by lint identity.
    pub(super) fn analyze(mut self, expression: &'tcx Expr<'tcx>) -> FunctionLayoutAnalysis {
        self.visit_expr(expression);
        self.analysis
    }

    /// Rejects phase comments stranded after the final entry in a block.
    fn analyze_trailing_gap(&mut self, block: &Block<'_>, entries: &[FunctionLayoutEntry]) {
        let last = entries.last().expect("entries are nonempty").span;
        let gap = block.span.with_lo(last.hi());
        let trailing = FunctionLayoutEntryGap::analyze(self.cx, self.config, gap, Some(last), None);
        self.analysis.malformed.extend(trailing.findings);
    }

    /// Records one direct phase that lacks a header or exceeds the configured line limit.
    fn record_oversized_phase(
        &mut self,
        phase: &[FunctionLayoutEntry],
        lines: usize,
        header: FunctionLayoutPhaseHeader,
    ) {
        // Locate the complete source phase receiving the diagnostic.
        let first = phase[0].span.source_callsite();
        let last = phase[phase.len() - 1].span.source_callsite();
        let span = Span::with_root_ctxt(first.lo(), last.hi());

        // Distinguish missing names from named phases that still contain too much work.
        let message = if matches!(header, FunctionLayoutPhaseHeader::Present) {
            format!(
                "this code phase contains {lines} lines, exceeding the configured maximum of {}",
                self.config.max_phase_lines
            )
        } else {
            format!("this {lines}-line code phase has no explanatory comment")
        };

        // Select guidance that matches the phase's missing name or excessive size.
        let help = if matches!(header, FunctionLayoutPhaseHeader::Present) {
            format!(
                "split this phase with `{}` explanatory comments or extract named operations",
                self.config.phase_comment_prefix
            )
        } else {
            format!(
                "name this phase with a `{}` explanatory comment or extract a named operation",
                self.config.phase_comment_prefix
            )
        };

        // Record the phase with its tailored remedy.
        self.analysis.missing.push(FunctionLayoutFinding {
            span,
            message,
            help,
            replacement: None,
        });
    }

    /// Splits direct code at canonical headers and measures every semantic phase.
    fn analyze_phases(
        &mut self,
        entries: &[FunctionLayoutEntry],
        headers: &[FunctionLayoutPhaseHeader],
    ) {
        let line_counts = entries
            .iter()
            .map(|entry| entry.direct_line_count(self.cx))
            .collect::<Vec<_>>();
        if line_counts.iter().sum::<usize>() <= self.config.max_phase_lines {
            return;
        }

        // Treat every valid explanation as the start of a distinct semantic phase.
        let mut phase_start = 0;
        for index in 1..=entries.len() {
            if index < entries.len() && matches!(headers[index], FunctionLayoutPhaseHeader::Missing)
            {
                continue;
            }
            let lines = line_counts[phase_start..index].iter().sum::<usize>();
            let header = headers[phase_start];
            let phase = &entries[phase_start..index];
            if matches!(header, FunctionLayoutPhaseHeader::Missing)
                || lines > self.config.max_phase_lines
            {
                self.record_oversized_phase(phase, lines, header);
            }
            phase_start = index;
        }
    }

    /// Parses the source gap preceding one entry and merges malformed-comment findings.
    fn analyze_entry_gap(
        &mut self,
        block: &Block<'_>,
        entries: &[FunctionLayoutEntry],
        index: usize,
        next: Span,
    ) -> FunctionLayoutPhaseHeader {
        // Resolve the source gap between this entry and its predecessor.
        let previous = index.checked_sub(1).map(|index| entries[index].span);
        let lo = previous.map_or_else(|| block.span.lo(), Span::hi);
        let gap = block.span.with_lo(lo).with_hi(next.lo());

        // Preserve the comment parser's semantic boundary decision.
        let analyzed =
            FunctionLayoutEntryGap::analyze(self.cx, self.config, gap, previous, Some(next));
        let header = FunctionLayoutPhaseHeader::from_entry_gap(&analyzed);

        // Merge malformed comments before returning the lightweight boundary state.
        self.analysis.malformed.extend(analyzed.findings);
        header
    }

    /// Classifies and analyzes all authored entries in one block.
    fn analyze_block(&mut self, block: &'tcx Block<'tcx>) {
        // Collect authored direct entries while excluding generated blocks.
        if block.span.from_expansion() {
            return;
        }

        // Convert statements and any tail expression into one direct entry sequence.
        let mut entries = block
            .stmts
            .iter()
            .map(|statement| FunctionLayoutEntry::from_statement(self.cx, statement))
            .collect::<Vec<_>>();
        if let Some(expression) = block.expr {
            entries.push(FunctionLayoutEntry::from_expression(self.cx, expression));
        }
        if entries.is_empty() {
            return;
        }

        // Validate each entry's leading gap while recording valid phase boundaries.
        let mut headers = Vec::with_capacity(entries.len());
        for (index, entry) in entries.iter().enumerate() {
            headers.push(self.analyze_entry_gap(block, &entries, index, entry.span));
        }
        self.analyze_trailing_gap(block, &entries);

        // Nested bodies are excluded here and visited as independent layout scopes.
        self.analyze_phases(&entries, &headers);
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

/// Function-layout measurements colocated with compiler spans.
pub(super) trait FunctionLayoutAnalyzerSpanExt {
    /// Counts physical lines containing non-comment source tokens within this span.
    fn code_line_count(self, cx: &LateContext<'_>) -> usize;
}

impl FunctionLayoutAnalyzerSpanExt for Span {
    fn code_line_count(self, cx: &LateContext<'_>) -> usize {
        FunctionLayoutNestedSpanCollector::filtered_line_count(cx, self, &[])
    }
}
