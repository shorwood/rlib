extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Arm, Block, Expr, ExprKind, HirId, MatchSource, StmtKind};
use rustc_lint::LateContext;
use rustc_span::Span;

use super::config::FunctionStructureConfig;
use super::control_flow_loop::LoopBodyExt;
use super::control_flow_metrics::{ControlFlowArmExt, ControlFlowExpressionExt};

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

/// More specific remediation available for an excessive-depth finding.
#[derive(Clone, Copy)]
enum ControlFlowDepthRemedy {
    /// Guard-clause guidance supersedes generic depth guidance.
    GuardClause,
    /// Only generic extraction or flattening guidance is available.
    General,
}

/// Return behavior of the outer function under analysis.
#[derive(Clone, Copy)]
pub(super) enum ControlFlowFunctionReturn {
    /// A bare return can exit the outer function.
    Unit,
    /// An early return must provide a value.
    Value,
}

/// Mutable traversal state shared by the control-flow readability analyses.
#[derive(Default)]
struct ControlFlowState {
    /// Current semantic nesting depth.
    depth: usize,
    /// Whether an ancestor has already been reported for excessive depth.
    is_inside_excessive_depth: bool,
    /// Spans already recognized as having a guard-clause alternative.
    guardable_spans: Vec<Span>,
}

/// HIR visitor that performs all control-flow readability analyses in one traversal.
pub(super) struct ControlFlowAnalyzer<'analysis, 'tcx> {
    /// Compiler context for parent queries, types, and source mapping.
    cx: &'analysis LateContext<'tcx>,
    /// Validated limits shared by the function-structure lint family.
    config: &'analysis FunctionStructureConfig,
    /// Findings accumulated during traversal.
    analysis: ControlFlowAnalysis,
    /// Depth and guard-clause state maintained during traversal.
    state: ControlFlowState,
    /// Whether the outer function permits a bare early `return`.
    function_return: ControlFlowFunctionReturn,
}

impl<'analysis, 'tcx> ControlFlowAnalyzer<'analysis, 'tcx> {
    /// Starts an empty analysis for one named function body.
    pub(super) fn new(
        cx: &'analysis LateContext<'tcx>,
        config: &'analysis FunctionStructureConfig,
        function_return: ControlFlowFunctionReturn,
    ) -> Self {
        Self {
            cx,
            config,
            analysis: ControlFlowAnalysis::default(),
            state: ControlFlowState::default(),
            function_return,
        }
    }

    /// Returns the trailing expression of a block, including semicolon statements.
    fn block_last_expression<'hir>(block: &'hir Block<'hir>) -> Option<&'hir Expr<'hir>> {
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

    /// Builds an excessive-depth finding for the first construct crossing the limit.
    fn deep_finding(
        config: &FunctionStructureConfig,
        expression: &Expr<'_>,
        kind: &str,
        depth: usize,
        remedy: ControlFlowDepthRemedy,
    ) -> ControlFlowDeepFinding {
        // Describe the exact construct and configured depth excess.
        let message = format!(
            "this {kind} reaches control-flow depth {}, exceeding the configured maximum of {}",
            depth, config.max_control_flow_depth
        );
        let help =
            "extract a named helper or flatten guardable branches before adding more nesting"
                .to_owned();

        // Preserve the semantic facts consumed by both depth-related diagnostics.
        ControlFlowDeepFinding {
            span: expression.span,
            hir_id: expression.hir_id,
            has_guard_clause_alternative: matches!(remedy, ControlFlowDepthRemedy::GuardClause),
            message,
            help,
        }
    }

    /// Returns whether `span` has already received guard-clause guidance.
    fn is_guardable(&self, span: Span) -> bool {
        self.state
            .guardable_spans
            .iter()
            .any(|guardable| guardable.lo() == span.lo() && guardable.hi() == span.hi())
    }

    /// Records only the outermost construct that crosses the nesting limit.
    fn record_excessive_depth(&mut self, expression: &Expr<'_>, kind: &str) {
        // Ignore permitted depth and descendants of an already reported construct.
        if self.state.depth <= self.config.max_control_flow_depth
            || self.state.is_inside_excessive_depth
        {
            return;
        }
        self.state.is_inside_excessive_depth = true;

        // Preserve whether more specific guard-clause guidance can supersede this finding.
        let remedy = if self.is_guardable(expression.span) {
            ControlFlowDepthRemedy::GuardClause
        } else {
            ControlFlowDepthRemedy::General
        };
        let finding = Self::deep_finding(self.config, expression, kind, self.state.depth, remedy);
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
        let previous_depth = self.state.depth;
        let previous_excessive = self.state.is_inside_excessive_depth;
        self.state.depth += 1;
        self.record_excessive_depth(expression, "match expression");

        // Measure each authored arm while traversing its nested expressions.
        for arm in arms {
            let lines = arm.code_lines(self.cx);
            if lines > self.config.max_match_arm_lines && !arm.span.from_expansion() {
                // Describe the oversized arm using its measured and configured limits.
                let message = format!(
                    "this match arm contains {lines} lines, exceeding the configured maximum of {}",
                    self.config.max_match_arm_lines
                );
                let help =
                    "extract the arm's work into a named helper or meaningful intermediary values"
                        .to_owned();

                // Record one actionable finding for the authored arm body.
                self.analysis.oversized_match_arms.push(ControlFlowFinding {
                    span: arm.body.span,
                    message,
                    help,
                });
            }
            intravisit::walk_arm(self, arm);
        }
        self.state.depth = previous_depth;
        self.state.is_inside_excessive_depth = previous_excessive;
    }

    /// Records one unique guard-clause opportunity.
    fn push_needless(&mut self, finding: ControlFlowFinding) {
        if self.is_guardable(finding.span) {
            return;
        }
        self.state.guardable_spans.push(finding.span);
        self.analysis.needless_nesting.push(finding);
    }

    /// Checks a block's trailing conditional before walking its contents.
    fn visit_block_with_exit(&mut self, block: &'tcx Block<'tcx>, exit: ControlFlowGuardExit) {
        if !block.span.from_expansion()
            && let Some(expression) = Self::block_last_expression(block)
            && matches!(expression.kind, ExprKind::If(_, _, None))
            && !matches!(exit, ControlFlowGuardExit::None)
        {
            // Name the guard exit that can replace the trailing branch.
            let exit_name = match exit {
                ControlFlowGuardExit::Return => "an early `return`",
                ControlFlowGuardExit::Continue => "an early `continue`",
                ControlFlowGuardExit::None => unreachable!(),
            };

            // Record the branch inversion with its concrete early-exit form.
            self.push_needless(ControlFlowFinding {
                span: expression.span,
                message: "this trailing condition needlessly wraps the remaining work".to_owned(),
                help: format!("invert the condition and use {exit_name} before the unwrapped body"),
            });
        }
        intravisit::walk_block(self, block);
    }

    /// Traverses the function expression and returns findings grouped by lint identity.
    pub(super) fn analyze(mut self, expression: &'tcx Expr<'tcx>) -> ControlFlowAnalysis {
        if let ExprKind::Block(block, _) = expression.kind {
            let exit = if matches!(self.function_return, ControlFlowFunctionReturn::Unit) {
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
        let previous_depth = self.state.depth;
        let previous_excessive = self.state.is_inside_excessive_depth;
        self.state.depth += 1;
        self.record_excessive_depth(expression, "loop");
        self.visit_block_with_exit(block, ControlFlowGuardExit::Continue);
        self.state.depth = previous_depth;
        self.state.is_inside_excessive_depth = previous_excessive;
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
                self.push_needless(ControlFlowFinding {
                    span: expression.span,
                    message: "this conditional keeps useful work inside a needless branch".to_owned(),
                    help: "use the diverging branch as a guard and move the useful work into the surrounding block".to_owned(),
                });
            }
        }

        // Traverse both branches at the same conditional depth.
        let previous_depth = self.state.depth;
        let previous_excessive = self.state.is_inside_excessive_depth;
        self.state.depth = depth;
        self.record_excessive_depth(expression, "conditional");

        // Traverse each branch while preserving one shared conditional depth.
        self.visit_expr(then);
        if let Some(otherwise) = otherwise {
            if let ExprKind::If(condition, then, nested_otherwise) = otherwise.kind {
                self.state.depth = previous_depth;
                self.state.is_inside_excessive_depth = previous_excessive;
                self.visit_if_at_depth(otherwise, condition, then, nested_otherwise, depth);
            } else {
                self.visit_expr(otherwise);
            }
        }
        self.state.depth = previous_depth;
        self.state.is_inside_excessive_depth = previous_excessive;
    }

    /// Reports an outermost method chain when its call count exceeds the limit.
    fn record_method_chain(&mut self, expression: &Expr<'_>) {
        // Ignore generated and nested receiver calls before measuring the chain.
        if expression.span.from_expansion() || expression.is_parent_method_receiver(self.cx) {
            return;
        }
        let calls = expression.method_chain_length();
        if calls <= self.config.max_method_chain_calls {
            return;
        }

        // Report only the outermost call so one chain produces one diagnostic.
        let message = format!(
            "this expression chains {calls} method calls, exceeding the configured maximum of {}",
            self.config.max_method_chain_calls
        );
        let help =
            "introduce meaningful intermediary bindings or extract a named operation".to_owned();

        // Record the measured outermost chain and its extraction guidance.
        self.analysis.long_method_chains.push(ControlFlowFinding {
            span: expression.span,
            message,
            help,
        });
    }
}

impl<'tcx> Visitor<'tcx> for ControlFlowAnalyzer<'_, 'tcx> {
    fn visit_block(&mut self, block: &'tcx Block<'tcx>) {
        let exit = if block.is_direct_loop_body(self.cx) {
            ControlFlowGuardExit::Continue
        } else {
            ControlFlowGuardExit::None
        };
        self.visit_block_with_exit(block, exit);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Traverse authored desugarings while ignoring opaque macro expansions.
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

        // Dispatch closures and conditionals before the remaining control-flow forms.
        if matches!(expression.kind, ExprKind::Closure(..)) {
            return;
        }
        if let ExprKind::If(condition, then, otherwise) = expression.kind {
            self.visit_if_at_depth(expression, condition, then, otherwise, self.state.depth + 1);
            return;
        }

        // Dispatch remaining authored control flow through its specialized handling.
        match expression.kind {
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
