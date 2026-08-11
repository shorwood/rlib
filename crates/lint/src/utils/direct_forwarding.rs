extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_hir::def::Res;
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::{Body, Expr, ExprKind, FnHeader, HirId, MatchSource, PatKind};
use rustc_lint::LateContext;
use rustc_span::symbol::sym;

// -----------------------------------------------------------------------------
// DirectForwarding: Shared forwarding syntax
// -----------------------------------------------------------------------------

/// A direct call and its semantic argument order.
pub struct Call<'hir> {
    /// Resolved function or method target.
    pub(crate) target: DefId,
    /// Receiver followed by explicit arguments for methods, or ordinary arguments for functions.
    pub(crate) arguments: Vec<&'hir Expr<'hir>>,
    /// Whether method-call syntax supplied the first argument as a receiver.
    pub(crate) is_method: bool,
}

/// The one call expression and parameter bindings found in a forwarding body.
pub struct Expression<'hir> {
    /// Direct call remaining after transparent syntax is removed.
    pub(crate) forwarded: &'hir Expr<'hir>,
    /// Plain parameter binding identities in declaration order.
    pub(crate) bindings: Vec<HirId>,
    /// Body owner whose type-checking results apply to the expression.
    pub(crate) typeck_owner: LocalDefId,
}

/// Stateless exact-forwarding syntax queries shared by wrapper policies.
pub struct DirectForwarding;

impl DirectForwarding {
    /// Allows documentation and lint controls while rejecting behavior-bearing attributes.
    pub(crate) fn has_only_nonsemantic_attributes(cx: &LateContext<'_>, hir_id: HirId) -> bool {
        cx.tcx.hir_attrs(hir_id).iter().all(|attribute| {
            attribute.is_doc_comment().is_some()
                || attribute.has_any_name(&[
                    sym::doc,
                    sym::allow,
                    sym::warn,
                    sym::deny,
                    sym::forbid,
                    sym::expect,
                ])
        })
    }

    /// Resolves plain parameter bindings and the function's one forwarding expression.
    pub(crate) fn expression<'hir>(
        cx: &LateContext<'hir>,
        def_id: LocalDefId,
        header: FnHeader,
        body: &'hir Body<'hir>,
    ) -> Option<Expression<'hir>> {
        // Reject destructured parameters before matching their uses by identity.
        let mut parameter_bindings = Vec::with_capacity(body.params.len());
        for parameter in body.params {
            let PatKind::Binding(_, binding, _, None) = parameter.pat.kind else {
                return None;
            };
            parameter_bindings.push(binding);
        }

        // Account for the compiler-generated coroutine bindings of async functions.
        if header.is_async() {
            return Self::async_expression(cx, body, &parameter_bindings);
        }
        Some(Expression {
            forwarded: Self::single_body_expression(body.value)?,
            bindings: parameter_bindings,
            typeck_owner: def_id,
        })
    }

    /// Recognizes one direct path or method call and preserves semantic argument order.
    pub(crate) fn call<'hir>(
        cx: &LateContext<'_>,
        owner: LocalDefId,
        expression: &'hir Expr<'hir>,
    ) -> Option<Call<'hir>> {
        match expression.kind {
            ExprKind::Call(callee, arguments) => Self::function_call(cx, callee, arguments),
            ExprKind::MethodCall(_, receiver, arguments, _) => {
                Self::method_call(cx, owner, expression, receiver, arguments)
            }
            _ => None,
        }
    }

    /// Returns whether an expression is one plain local binding.
    pub(crate) fn is_binding(cx: &LateContext<'_>, expression: &Expr<'_>, binding: HirId) -> bool {
        let ExprKind::Path(path) = expression.kind else {
            return false;
        };
        matches!(cx.qpath_res(&path, expression.hir_id), Res::Local(id) if id == binding)
    }

    /// Removes syntax that does not alter the returned value.
    pub(super) fn single_body_expression<'hir>(
        mut expression: &'hir Expr<'hir>,
    ) -> Option<&'hir Expr<'hir>> {
        loop {
            expression = match expression.kind {
                ExprKind::Block(block, None) if block.stmts.is_empty() => block.expr?,
                ExprKind::Block(block, None) if block.expr.is_none() && block.stmts.len() == 1 => {
                    Self::returned_expression(&block.stmts[0])?
                }
                ExprKind::DropTemps(inner) | ExprKind::Ret(Some(inner)) => inner,
                _ => return Some(expression),
            };
        }
    }

    /// Resolves one direct path call.
    fn function_call<'hir>(
        cx: &LateContext<'_>,
        callee: &Expr<'_>,
        arguments: &'hir [Expr<'hir>],
    ) -> Option<Call<'hir>> {
        // Resolve the authored path before packaging its ordinary argument order.
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };
        let target = cx.qpath_res(&path, callee.hir_id).opt_def_id()?;

        // Preserve ordinary authored argument order for exact-forwarding comparison.
        Some(Call {
            target,
            arguments: arguments.iter().collect(),
            is_method: false,
        })
    }

    /// Resolves one method call and prepends its receiver.
    fn method_call<'hir>(
        cx: &LateContext<'_>,
        owner: LocalDefId,
        expression: &'hir Expr<'hir>,
        receiver: &'hir Expr<'hir>,
        arguments: &'hir [Expr<'hir>],
    ) -> Option<Call<'hir>> {
        // Resolve the selected method before rebuilding semantic argument order.
        let target = cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)?;
        let mut forwarded = Vec::with_capacity(arguments.len() + 1);
        forwarded.push(receiver);
        forwarded.extend(arguments);

        // Preserve method syntax as evidence for receiver-specific policies.
        Some(Call {
            target,
            arguments: forwarded,
            is_method: true,
        })
    }

    /// Finds the call inside the compiler lowering of `target(arguments).await`.
    fn async_expression<'hir>(
        cx: &LateContext<'hir>,
        body: &'hir Body<'hir>,
        outer_bindings: &[HirId],
    ) -> Option<Expression<'hir>> {
        // Resolve the compiler-generated coroutine block and binding remap.
        let ExprKind::Closure(closure) = body.value.kind else {
            return None;
        };
        let coroutine = cx.tcx.hir_body(closure.body);
        let ExprKind::Block(block, None) = coroutine.value.kind else {
            return None;
        };
        if block.stmts.len() != outer_bindings.len() {
            return None;
        }
        let mut inner_bindings = Vec::with_capacity(outer_bindings.len());
        for (statement, outer_binding) in block.stmts.iter().zip(outer_bindings) {
            inner_bindings.push(Self::async_inner_binding(cx, statement, *outer_binding)?);
        }

        // Unwrap the compiler's await lowering to the one forwarded future.
        let awaited = Self::single_body_expression(block.expr?)?;
        let ExprKind::Match(scrutinee, _, MatchSource::AwaitDesugar) = awaited.kind else {
            return None;
        };
        let ExprKind::Call(_, futures) = scrutinee.kind else {
            return None;
        };
        let [forwarded] = futures else {
            return None;
        };

        // Reject await adapters that rely on a semantic type adjustment.
        let owner = cx.tcx.hir_body_owner_def_id(closure.body);
        let typeck = cx.tcx.typeck(owner);
        if typeck.expr_ty(awaited) != typeck.expr_ty_adjusted(awaited) {
            return None;
        }

        // Package the forwarded expression after semantic await validation.
        Some(Expression {
            forwarded,
            bindings: inner_bindings,
            typeck_owner: owner,
        })
    }

    /// Maps one compiler-generated coroutine binding back to its authored parameter.
    fn async_inner_binding(
        cx: &LateContext<'_>,
        statement: &rustc_hir::Stmt<'_>,
        outer_binding: HirId,
    ) -> Option<HirId> {
        // Resolve the generated local and require a plain binding pattern.
        let rustc_hir::StmtKind::Let(local) = statement.kind else {
            return None;
        };
        let PatKind::Binding(_, inner_binding, _, None) = local.pat.kind else {
            return None;
        };

        // Join the generated initializer back to the authored outer binding.
        let ExprKind::Path(path) = local.init?.kind else {
            return None;
        };
        matches!(cx.qpath_res(&path, local.init?.hir_id), Res::Local(id) if id == outer_binding)
            .then_some(inner_binding)
    }

    /// Extracts the value from a single explicit return statement.
    const fn returned_expression<'hir>(
        statement: &'hir rustc_hir::Stmt<'hir>,
    ) -> Option<&'hir Expr<'hir>> {
        let rustc_hir::StmtKind::Semi(expression) = statement.kind else {
            return None;
        };
        let ExprKind::Ret(Some(expression)) = expression.kind else {
            return None;
        };
        Some(expression)
    }
}
