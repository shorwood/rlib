extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Arm, Block, Expr, ExprKind, HirId, MatchSource, Node, StmtKind};
use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;

use super::config::FunctionStructureConfig;
use super::function_layout_analysis::function_layout_code_line_count;

// -----------------------------------------------------------------------------
// ControlFlow: Analyze semantic function structure
// -----------------------------------------------------------------------------

/// One structural problem found in a named function body.
pub struct ControlFlowFinding {
    /// Source range containing the structural problem.
    pub(crate) span: Span,
    /// Primary explanation of the violated readability limit.
    pub(crate) message: String,
    /// Refactoring direction appropriate to the finding.
    pub(crate) help: String,
}

/// One excessive-depth problem, including whether a guard-clause lint supersedes it.
pub struct ControlFlowDeepFinding {
    /// Outermost source range that first exceeds the configured depth.
    pub(crate) span: Span,
    /// HIR node used to coordinate overlapping control-flow lints.
    pub(crate) hir_id: HirId,
    /// Whether the needless-nesting lint offers a more specific remedy.
    pub(crate) has_guard_clause_alternative: bool,
    /// Primary explanation of the excessive nesting depth.
    pub(crate) message: String,
    /// Refactoring guidance for reducing nesting.
    pub(crate) help: String,
}

/// Findings split by public lint identity.
#[derive(Default)]
pub struct ControlFlowAnalysis {
    /// Branches that can be inverted into early exits.
    pub(crate) needless_nesting: Vec<ControlFlowFinding>,
    /// Outermost constructs exceeding the configured nesting depth.
    pub(crate) deep_nesting: Vec<ControlFlowDeepFinding>,
    /// Match arms exceeding the configured source-line limit.
    pub(crate) oversized_match_arms: Vec<ControlFlowFinding>,
    /// Outermost method chains exceeding the configured call limit.
    pub(crate) long_method_chains: Vec<ControlFlowFinding>,
}

#[derive(Clone, Copy)]
/// Early-exit form available when flattening a trailing conditional.
enum ControlFlowGuardExit {
    /// The surrounding block cannot be flattened with a direct early exit.
    None,
    /// A function body can invert the condition and return early.
    Return,
    /// A direct loop body can invert the condition and continue early.
    Continue,
}

/// Returns the trailing expression of a block, including semicolon statements.
fn control_flow_block_last_expression<'hir>(block: &'hir Block<'hir>) -> Option<&'hir Expr<'hir>> {
    block.expr.or_else(|| {
        block
            .stmts
            .last()
            .and_then(|statement| match statement.kind {
                StmtKind::Expr(expression) | StmtKind::Semi(expression) => Some(expression),
                StmtKind::Let(_) | StmtKind::Item(_) => None,
            })
    })
}

/// Counts authored code lines in one match arm body without counting its braces.
fn control_flow_match_arm_line_count(cx: &LateContext<'_>, arm: &Arm<'_>) -> usize {
    if let ExprKind::Block(block, _) = arm.body.kind {
        block
            .stmts
            .iter()
            .map(|statement| function_layout_code_line_count(cx, statement.span))
            .sum::<usize>()
            + block.expr.map_or(0, |expression| {
                function_layout_code_line_count(cx, expression.span)
            })
    } else {
        function_layout_code_line_count(cx, arm.body.span)
    }
}

/// Counts consecutive method calls by following receiver expressions inward.
const fn control_flow_method_chain_length(mut expression: &Expr<'_>) -> usize {
    let mut calls = 0;
    while let ExprKind::MethodCall(_, receiver, _, _) = expression.kind {
        calls += 1;
        expression = receiver;
    }
    calls
}

/// Returns whether an expression is the receiver of a surrounding method call.
fn control_flow_method_receiver_of_parent(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
    matches!(
        cx.tcx.parent_hir_node(expression.hir_id),
        Node::Expr(Expr {
            kind: ExprKind::MethodCall(_, receiver, _, _),
            ..
        }) if receiver.hir_id == expression.hir_id
    )
}

/// Distinguishes a loop's direct body block from blocks nested inside its tail.
fn control_flow_loop_ends_with_block(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    block: &Block<'_>,
) -> bool {
    let source_map = cx.sess().source_map();
    let block_end = source_map.lookup_char_pos(block.span.hi()).line;
    let loop_end = source_map.lookup_char_pos(expression.span.hi()).line;
    block_end == loop_end
}

/// Classifies whether a parent proves or disproves that `block` is a direct loop body.
fn control_flow_parent_loop_relation(
    cx: &LateContext<'_>,
    parent: HirId,
    block: &Block<'_>,
) -> Option<bool> {
    match cx.tcx.hir_node(parent) {
        Node::Expr(expression) => match expression.kind {
            ExprKind::Loop(..) => Some(control_flow_loop_ends_with_block(cx, expression, block)),
            ExprKind::Closure(..)
            | ExprKind::If(..)
            | ExprKind::Match(_, _, MatchSource::Normal | MatchSource::Postfix) => Some(false),
            _ => None,
        },
        Node::Item(_) | Node::TraitItem(_) | Node::ImplItem(_) | Node::Crate(_) => Some(false),
        _ => None,
    }
}

/// Builds an excessive-depth finding for the first construct crossing the limit.
fn control_flow_deep_finding(
    config: &FunctionStructureConfig,
    expression: &Expr<'_>,
    kind: &str,
    depth: usize,
    has_guard_clause_alternative: bool,
) -> ControlFlowDeepFinding {
    ControlFlowDeepFinding {
        span: expression.span,
        hir_id: expression.hir_id,
        has_guard_clause_alternative,
        message: format!(
            "this {kind} reaches control-flow depth {}, exceeding the configured maximum of {}",
            depth, config.max_control_flow_depth
        ),
        help: "extract a named helper or flatten guardable branches before adding more nesting"
            .to_owned(),
    }
}

/// Walks transparent HIR parents to determine whether `block` is a loop body.
fn control_flow_block_is_direct_loop_body(cx: &LateContext<'_>, block: &Block<'_>) -> bool {
    let mut current = block.hir_id;
    loop {
        let parent = cx.tcx.parent_hir_id(current);
        if parent == current {
            return false;
        }
        if let Some(is_direct) = control_flow_parent_loop_relation(cx, parent, block) {
            return is_direct;
        }
        current = parent;
    }
}

/// HIR visitor that performs all control-flow readability analyses in one traversal.
pub(super) struct ControlFlowAnalyzer<'analysis, 'tcx> {
    /// Compiler context for parent queries, types, and source mapping.
    cx: &'analysis LateContext<'tcx>,
    /// Validated limits shared by the function-structure lint family.
    config: &'analysis FunctionStructureConfig,
    /// Findings accumulated during traversal.
    analysis: ControlFlowAnalysis,
    /// Current semantic nesting depth.
    control_depth: usize,
    /// Whether an ancestor has already been reported for excessive depth.
    is_inside_excessive_depth: bool,
    /// Spans already recognized as having a guard-clause alternative.
    guardable_spans: Vec<Span>,
    /// Whether the outer function permits a bare early `return`.
    is_function_returning_unit: bool,
}

impl<'analysis, 'tcx> ControlFlowAnalyzer<'analysis, 'tcx> {
    /// Starts an empty analysis for one named function body.
    pub(super) fn new(
        cx: &'analysis LateContext<'tcx>,
        config: &'analysis FunctionStructureConfig,
        is_function_returning_unit: bool,
    ) -> Self {
        Self {
            cx,
            config,
            analysis: ControlFlowAnalysis::default(),
            control_depth: 0,
            is_inside_excessive_depth: false,
            guardable_spans: Vec::new(),
            is_function_returning_unit,
        }
    }

    /// Returns whether `span` has already received guard-clause guidance.
    fn is_guardable(&self, span: Span) -> bool {
        self.guardable_spans
            .iter()
            .any(|guardable| guardable.lo() == span.lo() && guardable.hi() == span.hi())
    }

    /// Records only the outermost construct that crosses the nesting limit.
    fn record_excessive_depth(&mut self, expression: &Expr<'_>, kind: &str) {
        if self.control_depth <= self.config.max_control_flow_depth
            || self.is_inside_excessive_depth
        {
            return;
        }
        self.is_inside_excessive_depth = true;
        let has_guard = self.is_guardable(expression.span);
        let finding =
            control_flow_deep_finding(self.config, expression, kind, self.control_depth, has_guard);
        self.analysis.deep_nesting.push(finding);
    }

    /// Measures and traverses a normal match at one additional control-flow level.
    fn visit_match(
        &mut self,
        expression: &'tcx Expr<'tcx>,
        scrutinee: &'tcx Expr<'tcx>,
        arms: &'tcx [Arm<'tcx>],
    ) {
        // Visit the scrutinee before entering the match's control-flow level.
        self.visit_expr(scrutinee);
        let previous_depth = self.control_depth;
        let previous_excessive = self.is_inside_excessive_depth;
        self.control_depth += 1;
        self.record_excessive_depth(expression, "match expression");

        // Measure each authored arm while traversing its nested expressions.
        for arm in arms {
            let lines = control_flow_match_arm_line_count(self.cx, arm);
            if lines > self.config.max_match_arm_lines && !arm.span.from_expansion() {
                self.analysis
                    .oversized_match_arms
                    .push(ControlFlowFinding {
                        span: arm.body.span,
                        message: format!(
                            "this match arm contains {lines} lines, exceeding the configured maximum of {}",
                            self.config.max_match_arm_lines
                        ),
                        help: "extract the arm's work into a named helper or meaningful intermediary values"
                            .to_owned(),
                    });
            }
            intravisit::walk_arm(self, arm);
        }
        self.control_depth = previous_depth;
        self.is_inside_excessive_depth = previous_excessive;
    }

    /// Records one unique guard-clause opportunity.
    fn push_needless(&mut self, span: Span, message: &str, help: String) {
        if self.is_guardable(span) {
            return;
        }
        self.guardable_spans.push(span);
        self.analysis.needless_nesting.push(ControlFlowFinding {
            span,
            message: message.to_owned(),
            help,
        });
    }

    /// Checks a block's trailing conditional before walking its contents.
    fn visit_block_with_exit(&mut self, block: &'tcx Block<'tcx>, exit: ControlFlowGuardExit) {
        if !block.span.from_expansion()
            && let Some(expression) = control_flow_block_last_expression(block)
            && matches!(expression.kind, ExprKind::If(_, _, None))
            && !matches!(exit, ControlFlowGuardExit::None)
        {
            let exit_name = match exit {
                ControlFlowGuardExit::Return => "an early `return`",
                ControlFlowGuardExit::Continue => "an early `continue`",
                ControlFlowGuardExit::None => unreachable!(),
            };
            self.push_needless(
                expression.span,
                "this trailing condition needlessly wraps the remaining work",
                format!("invert the condition and use {exit_name} before the unwrapped body"),
            );
        }
        intravisit::walk_block(self, block);
    }

    /// Traverses the function expression and returns findings grouped by lint identity.
    pub(super) fn analyze(mut self, expression: &'tcx Expr<'tcx>) -> ControlFlowAnalysis {
        if let ExprKind::Block(block, _) = expression.kind {
            let exit = if self.is_function_returning_unit {
                ControlFlowGuardExit::Return
            } else {
                ControlFlowGuardExit::None
            };
            self.visit_block_with_exit(block, exit);
        } else {
            self.visit_expr(expression);
        }
        self.analysis
    }

    /// Traverses a loop while preserving depth state for its siblings.
    fn visit_loop(&mut self, expression: &'tcx Expr<'tcx>, block: &'tcx Block<'tcx>) {
        let previous_depth = self.control_depth;
        let previous_excessive = self.is_inside_excessive_depth;
        self.control_depth += 1;
        self.record_excessive_depth(expression, "loop");
        self.visit_block_with_exit(block, ControlFlowGuardExit::Continue);
        self.control_depth = previous_depth;
        self.is_inside_excessive_depth = previous_excessive;
    }

    /// Returns whether an expression exits its current control-flow path.
    fn expression_diverges(&self, expression: &Expr<'_>) -> bool {
        if matches!(
            expression.kind,
            ExprKind::Break(..) | ExprKind::Continue(..) | ExprKind::Ret(..) | ExprKind::Become(..)
        ) {
            return true;
        }
        let owner = self.cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        self.cx.tcx.typeck(owner).expr_ty(expression).is_never()
    }

    /// Traverses an if/else-if chain at one shared conditional depth.
    fn visit_if_at_depth(
        &mut self,
        expression: &'tcx Expr<'tcx>,
        condition: &'tcx Expr<'tcx>,
        then: &'tcx Expr<'tcx>,
        otherwise: Option<&'tcx Expr<'tcx>>,
        depth: usize,
    ) {
        // Find guard-clause opportunities before increasing the recorded depth.
        self.visit_expr(condition);
        if let Some(otherwise) = otherwise {
            let then_diverges = self.expression_diverges(then);
            let otherwise_diverges = self.expression_diverges(otherwise);
            if then_diverges ^ otherwise_diverges {
                self.push_needless(
                    expression.span,
                    "this conditional keeps useful work inside a needless branch",
                    "use the diverging branch as a guard and move the useful work into the surrounding block".to_owned(),
                );
            }
        }

        // Traverse both branches at the same conditional depth.
        let previous_depth = self.control_depth;
        let previous_excessive = self.is_inside_excessive_depth;
        self.control_depth = depth;
        self.record_excessive_depth(expression, "conditional");
        self.visit_expr(then);
        if let Some(otherwise) = otherwise {
            if let ExprKind::If(condition, then, nested_otherwise) = otherwise.kind {
                self.control_depth = previous_depth;
                self.is_inside_excessive_depth = previous_excessive;
                self.visit_if_at_depth(otherwise, condition, then, nested_otherwise, depth);
            } else {
                self.visit_expr(otherwise);
            }
        }
        self.control_depth = previous_depth;
        self.is_inside_excessive_depth = previous_excessive;
    }

    /// Reports an outermost method chain when its call count exceeds the limit.
    fn record_method_chain(&mut self, expression: &Expr<'_>) {
        if expression.span.from_expansion()
            || control_flow_method_receiver_of_parent(self.cx, expression)
        {
            return;
        }
        let calls = control_flow_method_chain_length(expression);
        if calls <= self.config.max_method_chain_calls {
            return;
        }

        // Report only the outermost call so one chain produces one diagnostic.
        self.analysis
            .long_method_chains
            .push(ControlFlowFinding {
                span: expression.span,
                message: format!(
                    "this expression chains {calls} method calls, exceeding the configured maximum of {}",
                    self.config.max_method_chain_calls
                ),
                help: "introduce meaningful intermediary bindings or extract a named operation"
                    .to_owned(),
            });
    }
}

impl<'tcx> Visitor<'tcx> for ControlFlowAnalyzer<'_, 'tcx> {
    fn visit_block(&mut self, block: &'tcx Block<'tcx>) {
        let exit = if control_flow_block_is_direct_loop_body(self.cx, block) {
            ControlFlowGuardExit::Continue
        } else {
            ControlFlowGuardExit::None
        };
        self.visit_block_with_exit(block, exit);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if expression.span.from_expansion() {
            if expression.span.desugaring_kind().is_none() {
                return;
            }
            if let ExprKind::Loop(block, ..) = expression.kind {
                self.visit_loop(expression, block);
            } else {
                intravisit::walk_expr(self, expression);
            }
            return;
        }
        match expression.kind {
            ExprKind::Closure(..) => {}
            ExprKind::If(condition, then, otherwise) => self.visit_if_at_depth(
                expression,
                condition,
                then,
                otherwise,
                self.control_depth + 1,
            ),
            ExprKind::Loop(block, ..) => self.visit_loop(expression, block),
            ExprKind::Match(scrutinee, arms, MatchSource::Normal | MatchSource::Postfix) => {
                self.visit_match(expression, scrutinee, arms);
            }
            ExprKind::MethodCall(..) => {
                self.record_method_chain(expression);
                intravisit::walk_expr(self, expression);
            }
            _ => intravisit::walk_expr(self, expression),
        }
    }
}
