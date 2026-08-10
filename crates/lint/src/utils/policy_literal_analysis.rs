extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_ast::LitKind;
use rustc_hir::def::Res;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{
    BinOpKind, BindingMode, Body, ByRef, Expr, ExprKind, HirId, MatchSource, Mutability, PatKind,
    Stmt, StmtKind,
};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::Span;

use super::policy_api_catalog::PolicyApi;
use super::policy_literal_kind::PolicyCategory;
use super::policy_name_suggestion::{
    ConfigurationNameContext, for_authored_name, is_configuration_type_name,
};

// -----------------------------------------------------------------------------
// PolicyLiteralFinding: Complete diagnostic evidence
// -----------------------------------------------------------------------------

/// One literal-derived expression proven to control operational behavior.
pub struct PolicyLiteralFinding {
    /// Authored literal or maximal literal-derived expression.
    pub(crate) span: Span,
    /// Behavioral policy controlled by the expression.
    pub(crate) category: PolicyCategory,
    /// Precise semantic or structural evidence supporting the finding.
    pub(crate) context: String,
    /// Confidence-gated domain-specific constant name.
    pub(crate) suggested_name: Option<String>,
    /// Confidence of the evidence that classified this source expression.
    evidence_strength: u8,
}

impl PolicyLiteralFinding {
    /// Replaces weaker overlapping evidence while preserving one diagnostic per source expression.
    fn merge(&mut self, candidate: Self) {
        let is_stronger = candidate.evidence_strength > self.evidence_strength
            || (candidate.evidence_strength == self.evidence_strength
                && candidate.category.evidence_rank() > self.category.evidence_rank());
        if is_stronger {
            *self = candidate;
            return;
        }
        if self.suggested_name.is_some() {
            return;
        }
        self.suggested_name = candidate.suggested_name;
    }
}

// -----------------------------------------------------------------------------
// PolicyLiteral: Authored numeric policy material
// -----------------------------------------------------------------------------

/// Span and optional authored name of one literal-derived value.
#[derive(Clone)]
struct PolicyLiteralOrigin {
    /// Maximal source span that can be extracted into a constant.
    span: Span,
    /// Binding name available when the origin is reached through a local alias.
    authored_name: Option<String>,
}

/// Immutable local bindings mapped to their authored numeric origins.
type PolicyLiteralOrigins = HashMap<HirId, Vec<PolicyLiteralOrigin>>;

/// Collects maximal literal-derived subexpressions while resolving immutable aliases.
struct PolicyLiteralOriginCollector<'analysis, 'tcx> {
    /// Compiler context used for semantic path and type queries.
    cx: &'analysis LateContext<'tcx>,
    /// Previously collected immutable local definitions.
    locals: &'analysis PolicyLiteralOrigins,
    /// Origins accumulated from the selected policy expression.
    origins: Vec<PolicyLiteralOrigin>,
}

impl<'analysis, 'tcx> PolicyLiteralOriginCollector<'analysis, 'tcx> {
    /// Resolves all literal material contributing to one selected expression.
    fn collect(
        cx: &'analysis LateContext<'tcx>,
        locals: &'analysis PolicyLiteralOrigins,
        expression: &'tcx Expr<'tcx>,
    ) -> Vec<PolicyLiteralOrigin> {
        let mut collector = Self {
            cx,
            locals,
            origins: Vec::new(),
        };
        collector.visit_expr(expression);
        collector.origins
    }

    /// Returns whether an expression is entirely composed of numeric literals and operators.
    fn is_literal_derived(expression: &Expr<'_>) -> bool {
        // Accept numeric leaves as the base of a constant expression.
        if let ExprKind::Lit(literal) = expression.kind {
            return matches!(literal.node, LitKind::Int(..) | LitKind::Float(..));
        }

        // Peel compile-time wrappers without broadening the accepted leaves.
        if let ExprKind::Unary(_, inner)
        | ExprKind::Cast(inner, _)
        | ExprKind::Type(inner, _)
        | ExprKind::DropTemps(inner)
        | ExprKind::Use(inner, _) = expression.kind
        {
            return Self::is_literal_derived(inner);
        }

        // Require both operands of arithmetic expressions to remain literal-derived.
        let ExprKind::Binary(operator, left, right) = expression.kind else {
            return false;
        };
        Self::is_arithmetic(operator.node)
            && Self::is_literal_derived(left)
            && Self::is_literal_derived(right)
    }

    /// Returns whether a binary operator can participate in a numeric constant expression.
    const fn is_arithmetic(operator: BinOpKind) -> bool {
        matches!(
            operator,
            BinOpKind::Add
                | BinOpKind::Sub
                | BinOpKind::Mul
                | BinOpKind::Div
                | BinOpKind::Rem
                | BinOpKind::BitXor
                | BinOpKind::BitAnd
                | BinOpKind::BitOr
                | BinOpKind::Shl
                | BinOpKind::Shr
        )
    }

    /// Exempts only direct conventional identity literals.
    fn is_identity(expression: &Expr<'_>) -> bool {
        // Recognize the conventional numeric identities exactly as authored.
        if let ExprKind::Lit(literal) = expression.kind {
            return match literal.node {
                LitKind::Int(value, _) => matches!(value.0, 0 | 1),
                LitKind::Float(symbol, _) => {
                    matches!(symbol.as_str(), "0" | "0.0" | "1" | "1.0")
                }
                _ => false,
            };
        }

        // Peel harmless type wrappers while retaining unary signs as meaningful policy.
        let (ExprKind::Cast(inner, _) | ExprKind::Type(inner, _) | ExprKind::Use(inner, _)) =
            expression.kind
        else {
            return false;
        };
        Self::is_identity(inner)
    }

    /// Resolves an authored path back to an immutable literal origin.
    fn local_origins(&self, expression: &Expr<'_>) -> Option<Vec<PolicyLiteralOrigin>> {
        let ExprKind::Path(path) = expression.kind else {
            return None;
        };
        let Res::Local(binding) = self.cx.qpath_res(&path, expression.hir_id) else {
            return None;
        };
        self.locals.get(&binding).cloned()
    }
}

impl<'tcx> Visitor<'tcx> for PolicyLiteralOriginCollector<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if matches!(expression.kind, ExprKind::ConstBlock(_)) {
            return;
        }
        if let Some(origins) = self.local_origins(expression) {
            self.origins.extend(origins);
            return;
        }
        if Self::is_literal_derived(expression) {
            if !expression.span.from_expansion() && !Self::is_identity(expression) {
                self.origins.push(PolicyLiteralOrigin {
                    span: expression.span,
                    authored_name: None,
                });
            }
            return;
        }

        // Index literals select positions rather than define behavioral policy.
        if let ExprKind::Index(container, _, _) = expression.kind {
            self.visit_expr(container);
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// LocalLiteralCollector: Immutable literal aliases
// -----------------------------------------------------------------------------

/// First traversal that resolves immutable local aliases before policy classification.
struct LocalLiteralCollector<'analysis, 'tcx> {
    /// Compiler context used by literal-origin collection.
    cx: &'analysis LateContext<'tcx>,
    /// Literal origins indexed by local binding HIR identifier.
    locals: PolicyLiteralOrigins,
    /// Policy-named locals that require constants without additional use evidence.
    named_findings: Vec<PolicyLiteralFinding>,
}

impl<'analysis, 'tcx> LocalLiteralCollector<'analysis, 'tcx> {
    /// Starts an empty local-definition traversal.
    fn for_context(cx: &'analysis LateContext<'tcx>) -> Self {
        Self {
            cx,
            locals: HashMap::new(),
            named_findings: Vec::new(),
        }
    }

    /// Records one simple immutable binding and its literal origins.
    fn record_local(&mut self, statement: &'tcx Stmt<'tcx>) {
        // Restrict propagation to simple immutable initialized bindings.
        let StmtKind::Let(local) = statement.kind else {
            return;
        };
        let PatKind::Binding(BindingMode(ByRef::No, Mutability::Not), binding, identifier, None) =
            local.pat.kind
        else {
            return;
        };

        // Exclude declarations without a value before attempting origin resolution.
        let Some(initializer) = local.init else {
            return;
        };

        // Resolve origins before attaching the authored alias vocabulary.
        let mut origins = PolicyLiteralOriginCollector::collect(self.cx, &self.locals, initializer);
        for origin in &mut origins {
            origin.authored_name = Some(identifier.name.as_str().to_owned());
        }
        if origins.is_empty() {
            return;
        }

        // A policy-bearing local name is itself sufficient evidence that a constant is required.
        if !identifier.name.as_str().starts_with('_')
            && let Some(category) = PolicyCategory::from_name(identifier.name.as_str())
        {
            let suggested_name = for_authored_name(identifier.name.as_str(), category);
            for origin in &origins {
                // Retain name evidence at low confidence for later semantic merging.
                let context = format!(
                    "immutable local `{identifier}` names this value as a {}",
                    category.description()
                );

                // Store name evidence for later merging with stronger semantic evidence.
                let finding = PolicyLiteralFinding {
                    span: origin.span,
                    category,
                    context,
                    suggested_name: suggested_name.clone(),
                    evidence_strength: 1,
                };

                // Keep all aliased origins because one expression may contain several literals.
                self.named_findings.push(finding);
            }
        }
        self.locals.insert(binding, origins);
    }
}

impl<'tcx> Visitor<'tcx> for LocalLiteralCollector<'_, 'tcx> {
    fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
        self.record_local(statement);
        intravisit::walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if matches!(
            expression.kind,
            ExprKind::Closure(_) | ExprKind::ConstBlock(_)
        ) {
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// ControlFlowEvidence: Fallibility and exits
// -----------------------------------------------------------------------------

/// Semantic proof retained while examining one controlled body.
#[derive(Default)]
struct ControlFlowEvidence {
    /// Body contains `?` or an expression producing the standard `Result` type.
    has_fallible_operation: bool,
    /// Body returns, breaks, continues, or diverges.
    has_exit: bool,
}

impl ControlFlowEvidence {
    /// Collects evidence without descending into separately checked closures.
    fn for_expression<'tcx>(cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) -> Self {
        let mut visitor = ControlFlowEvidenceVisitor {
            cx,
            evidence: Self::default(),
        };
        visitor.visit_expr(expression);
        visitor.evidence
    }
}

/// HIR visitor that identifies fallible operations and control-flow exits.
struct ControlFlowEvidenceVisitor<'analysis, 'tcx> {
    /// Compiler context used for expression-type queries.
    cx: &'analysis LateContext<'tcx>,
    /// Evidence accumulated from the selected branch or loop body.
    evidence: ControlFlowEvidence,
}

impl<'tcx> Visitor<'tcx> for ControlFlowEvidenceVisitor<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Keep separately checked bodies and anonymous constants outside this evidence scope.
        if matches!(
            expression.kind,
            ExprKind::Closure(_) | ExprKind::ConstBlock(_)
        ) {
            return;
        }

        // Record explicit try desugaring and authored exits independently.
        if matches!(
            expression.kind,
            ExprKind::Match(_, _, MatchSource::TryDesugar(_))
        ) {
            self.evidence.has_fallible_operation = true;
        }

        // Treat direct control-flow transfer as a policy-relevant branch effect.
        if matches!(
            expression.kind,
            ExprKind::Ret(_) | ExprKind::Break(..) | ExprKind::Continue(_)
        ) {
            self.evidence.has_exit = true;
        }

        // Treat every standard Result-producing expression as fallible evidence.
        let expression_type = self.cx.typeck_results().expr_ty(expression);
        if let ty::Adt(definition, _) = expression_type.kind()
            && self
                .cx
                .tcx
                .is_diagnostic_item(rustc_span::sym::Result, definition.did())
        {
            self.evidence.has_fallible_operation = true;
        }
        if expression_type.is_never() {
            self.evidence.has_exit = true;
        }

        // Continue through transparent expressions in the selected body.
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// EvidenceCollector: Policy context classification
// -----------------------------------------------------------------------------

/// Second traversal that attaches policy meaning to previously resolved literal origins.
struct EvidenceCollector<'analysis, 'tcx> {
    /// Compiler context used for semantic API and type resolution.
    cx: &'analysis LateContext<'tcx>,
    /// Immutable local aliases collected during the first traversal.
    locals: &'analysis PolicyLiteralOrigins,
    /// Deduplicated findings accumulated for the current body.
    findings: Vec<PolicyLiteralFinding>,
}

impl<'analysis, 'tcx> EvidenceCollector<'analysis, 'tcx> {
    /// Returns the authored argument list for direct and method calls.
    const fn call_arguments(expression: &'tcx Expr<'tcx>) -> Option<&'tcx [Expr<'tcx>]> {
        match expression.kind {
            ExprKind::Call(_, arguments) | ExprKind::MethodCall(_, _, arguments, _) => {
                Some(arguments)
            }
            _ => None,
        }
    }

    /// Deduplicates a finding by exact authored source span.
    fn record(&mut self, candidate: PolicyLiteralFinding) {
        if let Some(existing) = self.findings.iter_mut().find(|finding| {
            finding.span.lo() == candidate.span.lo() && finding.span.hi() == candidate.span.hi()
        }) {
            existing.merge(candidate);
            return;
        }
        self.findings.push(candidate);
    }

    /// Starts policy classification with any name-derived local findings.
    fn new(
        cx: &'analysis LateContext<'tcx>,
        locals: &'analysis PolicyLiteralOrigins,
        named_findings: Vec<PolicyLiteralFinding>,
    ) -> Self {
        let mut collector = Self {
            cx,
            locals,
            findings: Vec::new(),
        };
        for finding in named_findings {
            collector.record(finding);
        }
        collector
    }

    /// Records every literal origin contributing to one proven policy expression.
    fn record_origins(
        &mut self,
        expression: &'tcx Expr<'tcx>,
        category: PolicyCategory,
        context: &str,
        suggested_name: Option<&str>,
        evidence_strength: u8,
    ) {
        let origins = PolicyLiteralOriginCollector::collect(self.cx, self.locals, expression);
        for origin in origins {
            // Prefer alias vocabulary before falling back to contextual inference.
            let suggestion = origin
                .authored_name
                .as_deref()
                .and_then(|name| for_authored_name(name, category))
                .or_else(|| suggested_name.map(str::to_owned));

            // Materialize the complete remediation context at the evidence boundary.
            self.record(PolicyLiteralFinding {
                span: origin.span,
                category,
                context: context.to_owned(),
                suggested_name: suggestion,
                evidence_strength,
            });
        }
    }

    /// Records literals passed into a semantically resolved policy API.
    fn record_api(&mut self, expression: &'tcx Expr<'tcx>) {
        let Some(api) = PolicyApi::for_expression(self.cx, expression) else {
            return;
        };

        // Resolve authored positions only after the semantic call has matched.
        let Some(arguments) = Self::call_arguments(expression) else {
            return;
        };
        for position in api.argument_positions {
            let Some(argument) = arguments.get(*position) else {
                continue;
            };

            // Retain the exact API and argument role for remediation context.
            let context = format!(
                "argument {} to `{}` controls its {}",
                position + 1,
                api.display,
                api.category.description()
            );
            self.record_origins(argument, api.category, &context, None, 3);
        }
    }

    /// Records policy-bearing fields on explicit configuration values.
    fn record_configuration(&mut self, expression: &'tcx Expr<'tcx>) {
        // Resolve only struct expressions whose type advertises configuration ownership.
        let ExprKind::Struct(path, fields, _) = expression.kind else {
            return;
        };

        // Resolve the defining type so aliases cannot forge configuration identity.
        let Some(definition) = self.cx.qpath_res(path, expression.hir_id).opt_def_id() else {
            return;
        };

        // Restrict generic field vocabulary to explicit configuration containers.
        let type_symbol = self.cx.tcx.item_name(definition);
        let type_name = type_symbol.as_str();
        if !is_configuration_type_name(type_name) {
            return;
        }

        // Combine every policy-bearing field role with the configuration domain.
        for field in fields {
            // Ignore fields whose names do not carry policy vocabulary.
            let field_name = field.ident.name.as_str();
            let Some(category) = PolicyCategory::from_name(field_name) else {
                continue;
            };

            // Combine the field role with its owning configuration domain.
            let name_context = ConfigurationNameContext {
                type_name,
                field_name,
            };
            let suggestion = name_context.infer(category);

            // Explain both the owning configuration and the field's policy role.
            let context = format!(
                "field `{type_name}::{field_name}` stores a literal-backed {}",
                category.description()
            );
            self.record_origins(field.expr, category, &context, suggestion.as_deref(), 4);
        }
    }

    /// Records comparisons whose controlled branch is fallible or exits its current flow.
    fn record_branch_thresholds(
        &mut self,
        condition: &'tcx Expr<'tcx>,
        then_expression: &'tcx Expr<'tcx>,
        else_expression: Option<&'tcx Expr<'tcx>>,
    ) {
        // Require a fallible operation or exit before treating comparisons as policy.
        let then_evidence = ControlFlowEvidence::for_expression(self.cx, then_expression);
        let else_evidence = else_expression
            .map(|expression| ControlFlowEvidence::for_expression(self.cx, expression))
            .unwrap_or_default();

        // Require either branch to make the comparison operationally consequential.
        let has_policy_effect = then_evidence.has_fallible_operation
            || then_evidence.has_exit
            || else_evidence.has_fallible_operation
            || else_evidence.has_exit;
        if !has_policy_effect {
            return;
        }

        let mut comparisons = PolicyComparisonCollector::default();
        comparisons.visit_expr(condition);
        for comparison in comparisons.comparisons {
            // Infer names only from direct comparison subjects.
            let suggested_name = comparison
                .subject_name
                .as_deref()
                .and_then(|name| for_authored_name(name, PolicyCategory::Threshold));

            // Describe the exact flow consequence instead of merely naming a threshold.
            let context = "this comparison controls a branch that performs a fallible operation or exits the current control flow";

            // Attach the complete threshold evidence to every literal-derived side.
            self.record_origins(
                comparison.literal_side,
                PolicyCategory::Threshold,
                context,
                suggested_name.as_deref(),
                2,
            );
        }
    }

    /// Records range bounds only when the desugared `for` body is fallible.
    fn record_fallible_for_loop(
        &mut self,
        scrutinee: &'tcx Expr<'tcx>,
        arms: &'tcx [rustc_hir::Arm<'tcx>],
    ) {
        // Reject ordinary iteration before resolving authored range bounds.
        let has_fallible_body = arms.iter().any(|arm| {
            ControlFlowEvidence::for_expression(self.cx, arm.body).has_fallible_operation
        });
        if !has_fallible_body {
            return;
        }

        // Traverse only range constructors within the desugared iterator expression.
        let mut ranges = RangeOriginCollector {
            cx: self.cx,
            locals: self.locals,
            origins: Vec::new(),
        };
        ranges.visit_expr(scrutinee);
        for origin in ranges.origins {
            // Promote alias vocabulary when the bound already names its retry contract.
            let suggested_name = origin
                .authored_name
                .as_deref()
                .and_then(|name| for_authored_name(name, PolicyCategory::Retry));

            // Preserve the fallible repetition evidence in the final diagnostic.
            self.record(PolicyLiteralFinding {
                span: origin.span,
                category: PolicyCategory::Retry,
                context: "this range bounds repeated execution of a fallible operation".to_owned(),
                suggested_name,
                evidence_strength: 4,
            });
        }
    }
}

impl<'tcx> Visitor<'tcx> for EvidenceCollector<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Leave separately checked bodies and const blocks to their own ownership boundary.
        if matches!(
            expression.kind,
            ExprKind::Closure(_) | ExprKind::ConstBlock(_)
        ) {
            return;
        }

        // Attach direct semantic evidence before broader control-flow evidence.
        self.record_api(expression);
        self.record_configuration(expression);

        // Classify authored branches and desugared loops by structural role.
        match expression.kind {
            ExprKind::If(condition, then_expression, else_expression) => {
                self.record_branch_thresholds(condition, then_expression, else_expression);
            }
            ExprKind::Match(scrutinee, arms, MatchSource::ForLoopDesugar) => {
                self.record_fallible_for_loop(scrutinee, arms);
            }
            _ => {}
        }

        // Continue through remaining transparent expression structure.
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// PolicyComparison: Authored branch thresholds
// -----------------------------------------------------------------------------

/// One comparison side carrying literals and its opposite authored subject.
struct PolicyComparison<'tcx> {
    /// Expression from which literal origins are extracted.
    literal_side: &'tcx Expr<'tcx>,
    /// Simple opposite-side identifier available for constant-name inference.
    subject_name: Option<String>,
}

/// Finds numeric comparison expressions without entering nested closures.
#[derive(Default)]
struct PolicyComparisonCollector<'tcx> {
    /// Comparisons retained from a compound condition.
    comparisons: Vec<PolicyComparison<'tcx>>,
}

impl PolicyComparisonCollector<'_> {
    /// Returns whether one operator establishes a branch threshold.
    const fn is_comparison(operator: BinOpKind) -> bool {
        matches!(
            operator,
            BinOpKind::Eq
                | BinOpKind::Ne
                | BinOpKind::Lt
                | BinOpKind::Le
                | BinOpKind::Ge
                | BinOpKind::Gt
        )
    }

    /// Retains a simple source identifier for confidence-gated name inference.
    fn simple_name(expression: &Expr<'_>) -> Option<String> {
        // Ignore compound subjects whose domain cannot be named without guessing.
        let ExprKind::Path(path) = expression.kind else {
            return None;
        };

        // Preserve only the final identifier for direct and type-relative paths.
        match path {
            rustc_hir::QPath::Resolved(_, path) => path
                .segments
                .last()
                .map(|segment| segment.ident.name.as_str().to_owned()),
            rustc_hir::QPath::TypeRelative(_, segment) => {
                Some(segment.ident.name.as_str().to_owned())
            }
        }
    }
}

impl<'tcx> Visitor<'tcx> for PolicyComparisonCollector<'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if matches!(
            expression.kind,
            ExprKind::Closure(_) | ExprKind::ConstBlock(_)
        ) {
            return;
        }
        if let ExprKind::Binary(operator, left, right) = expression.kind
            && Self::is_comparison(operator.node)
        {
            // Retain both orientations; literal collection naturally rejects the nonliteral side.
            self.comparisons.push(PolicyComparison {
                literal_side: right,
                subject_name: Self::simple_name(left),
            });

            // Reverse the sides so origin collection can determine which is literal-backed.
            self.comparisons.push(PolicyComparison {
                literal_side: left,
                subject_name: Self::simple_name(right),
            });
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// RangeOriginCollector: Fallible `for` bounds
// -----------------------------------------------------------------------------

/// Finds range expressions inside a desugared `for` iterator expression.
struct RangeOriginCollector<'analysis, 'tcx> {
    /// Compiler context used to resolve range constructors.
    cx: &'analysis LateContext<'tcx>,
    /// Immutable aliases available to range bounds.
    locals: &'analysis PolicyLiteralOrigins,
    /// Literal origins collected only from actual range expressions.
    origins: Vec<PolicyLiteralOrigin>,
}

impl RangeOriginCollector<'_, '_> {
    /// Returns whether a struct expression constructs a standard range.
    fn is_range(&self, expression: &Expr<'_>) -> bool {
        let ExprKind::Struct(path, _, _) = expression.kind else {
            return false;
        };
        let Some(definition) = self.cx.qpath_res(path, expression.hir_id).opt_def_id() else {
            return false;
        };
        let path = self.cx.tcx.def_path_str(definition);
        path.contains("::ops::range::Range") || path.ends_with("::ops::Range")
    }
}

impl<'tcx> Visitor<'tcx> for RangeOriginCollector<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if self.is_range(expression) {
            self.origins.extend(PolicyLiteralOriginCollector::collect(
                self.cx,
                self.locals,
                expression,
            ));
            return;
        }
        if matches!(
            expression.kind,
            ExprKind::Closure(_) | ExprKind::ConstBlock(_)
        ) {
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// PolicyLiteralAnalyzer: Body level orchestration
// -----------------------------------------------------------------------------

/// Performs alias resolution and policy classification for one executable body.
pub struct PolicyLiteralAnalyzer;

impl PolicyLiteralAnalyzer {
    /// Returns every deduplicated unnamed policy literal in one function or closure body.
    pub(crate) fn analyze<'tcx>(
        cx: &LateContext<'tcx>,
        body: &'tcx Body<'tcx>,
    ) -> Vec<PolicyLiteralFinding> {
        let mut locals = LocalLiteralCollector::for_context(cx);
        locals.visit_expr(body.value);

        let mut evidence = EvidenceCollector::new(cx, &locals.locals, locals.named_findings);
        evidence.visit_expr(body.value);
        evidence.findings.sort_by_key(|finding| finding.span.lo());
        evidence.findings
    }
}
