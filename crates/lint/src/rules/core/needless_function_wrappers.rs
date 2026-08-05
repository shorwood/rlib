extern crate rustc_abi;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_abi::ExternAbi;
use rustc_errors::DiagDecorator;
use rustc_hir::{
    Body, Expr, ExprKind, FnHeader, HirId, ImplItem, ImplItemKind, Item, ItemKind, MatchSource,
    Node, PatKind,
    def::{DefKind, Res},
    def_id::LocalDefId,
};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::{Span, Symbol, sym};

// -----------------------------------------------------------------------------
// RedundantWrapper: Forwarding wrapper model
// -----------------------------------------------------------------------------

/// The forwarding expression and parameter bindings found inside a wrapper.
struct RedundantWrapperForwarding<'hir> {
    /// Direct call expression remaining after transparent syntax is removed.
    expression: &'hir Expr<'hir>,
    /// Parameter binding identities in their declared order.
    bindings: Vec<HirId>,
    /// Body owner whose type-checking results apply to `expression`.
    typeck_owner: LocalDefId,
}

/// The local call targeted by a possible forwarding wrapper.
struct RedundantWrapperCall<'hir> {
    /// Local function or method definition being forwarded to.
    target: LocalDefId,
    /// Receiver and arguments in signature order.
    arguments: Vec<&'hir Expr<'hir>>,
    /// Whether the target is invoked with method-call syntax.
    is_method: bool,
}

/// A function whose body only forwards its parameters to another local function.
struct RedundantWrapper {
    /// Wrapper HIR node used for diagnostic ownership.
    hir_id: HirId,
    /// Wrapper name shown in remediation guidance.
    name: Symbol,
    /// Identifier span used as the primary diagnostic site.
    name_span: Span,
    /// Local implementation to which every argument is forwarded.
    target: LocalDefId,
}

impl RedundantWrapper {
    /// Recognizes a safe wrapper that forwards every parameter unchanged to one local target.
    fn discover<'tcx>(
        cx: &LateContext<'tcx>,
        def_id: LocalDefId,
        hir_id: HirId,
        name: Symbol,
        name_span: Span,
        header: FnHeader,
        body: &'tcx Body<'tcx>,
    ) -> Option<Self> {
        if header.abi != ExternAbi::Rust
            || header.is_unsafe()
            || name_span.from_expansion()
            || !Self::has_only_nonsemantic_attributes(cx, hir_id)
        {
            return None;
        }

        let parameter_bindings = body
            .params
            .iter()
            .map(|parameter| match parameter.pat.kind {
                PatKind::Binding(_, binding, _, None) => Some(binding),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let forwarding = if header.is_async() {
            Self::async_forwarding_expression(cx, body, &parameter_bindings)?
        } else {
            RedundantWrapperForwarding {
                expression: Self::single_body_expression(body.value)?,
                bindings: parameter_bindings,
                typeck_owner: def_id,
            }
        };
        let call = Self::direct_call(cx, forwarding.typeck_owner, forwarding.expression)?;
        if call.target == def_id
            || call.arguments.len() != forwarding.bindings.len()
            || (header.is_async() && !Self::is_async_function(cx, call.target))
            || (call.is_method && !Self::shares_inherent_type(cx, def_id, call.target))
        {
            return None;
        }

        let typeck = cx.tcx.typeck(forwarding.typeck_owner);
        let signature = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();
        if signature.inputs().len() != call.arguments.len()
            || (!header.is_async() && signature.output() != typeck.expr_ty(forwarding.expression))
        {
            return None;
        }
        if call.is_method {
            let target_signature = cx
                .tcx
                .fn_sig(call.target)
                .instantiate(cx.tcx, typeck.node_args(forwarding.expression.hir_id))
                .skip_binder();
            if target_signature.inputs().len() != signature.inputs().len()
                || !Self::same_receiver_type(target_signature.inputs()[0], signature.inputs()[0])
            {
                return None;
            }
        }

        let bound_arguments = forwarding.bindings.iter().zip(call.arguments);
        let typed_arguments = bound_arguments.zip(signature.inputs());
        for (index, ((binding, argument), input)) in typed_arguments.enumerate() {
            let ExprKind::Path(path) = argument.kind else {
                return None;
            };
            let has_incompatible_type =
                !(call.is_method && index == 0) && typeck.expr_ty_adjusted(argument) != *input;
            let has_matching_binding =
                matches!(cx.qpath_res(&path, argument.hir_id), Res::Local(id) if id == *binding);
            if has_matching_binding && !has_incompatible_type {
                continue;
            }
            return None;
        }

        Some(Self {
            hir_id,
            name,
            name_span,
            target: call.target,
        })
    }

    /// Finds the call inside the compiler lowering of `target(arguments).await`.
    ///
    /// Async functions first move their source parameters into the generated coroutine. Keeping
    /// that explicit binding map lets the same exact-order check used for synchronous functions
    /// apply without relying on parameter names.
    fn async_forwarding_expression<'hir>(
        cx: &LateContext<'hir>,
        body: &'hir Body<'hir>,
        outer_bindings: &[HirId],
    ) -> Option<RedundantWrapperForwarding<'hir>> {
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

        // Require exactly the future passed through by the wrapper.
        let [forwarded] = futures else {
            return None;
        };

        // Reject await adapters that rely on a semantic type adjustment.
        let owner = cx.tcx.hir_body_owner_def_id(closure.body);
        let typeck = cx.tcx.typeck(owner);
        if typeck.expr_ty(awaited) != typeck.expr_ty_adjusted(awaited) {
            return None;
        }
        Some(RedundantWrapperForwarding {
            expression: forwarded,
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
        // Recover the compiler-generated local binding.
        let rustc_hir::StmtKind::Let(local) = statement.kind else {
            return None;
        };

        // Require a simple binding initialized from the authored parameter.
        let PatKind::Binding(_, inner_binding, _, None) = local.pat.kind else {
            return None;
        };
        let ExprKind::Path(path) = local.init?.kind else {
            return None;
        };

        // Preserve the binding only when resolution confirms the forwarding relationship.
        matches!(cx.qpath_res(&path, local.init?.hir_id), Res::Local(id) if id == outer_binding)
            .then_some(inner_binding)
    }

    /// Returns whether a local free function or inherent method is asynchronous.
    fn is_async_function(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
        match cx.tcx.hir_node_by_def_id(def_id) {
            Node::Item(item) => {
                matches!(item.kind, ItemKind::Fn { sig, .. } if sig.header.is_async())
            }
            Node::ImplItem(item) => {
                matches!(item.kind, ImplItemKind::Fn(sig, _) if sig.header.is_async())
            }
            _ => false,
        }
    }

    /// Prevents receiver auto-deref from turning a meaningful adapter into a candidate while still
    /// allowing wrappers that call a method from another impl block for the same type.
    fn shares_inherent_type(cx: &LateContext<'_>, wrapper: LocalDefId, target: LocalDefId) -> bool {
        let wrapper_impl = cx.tcx.local_parent(wrapper);
        let target_impl = cx.tcx.local_parent(target);
        if !matches!(
            cx.tcx.def_kind(wrapper_impl),
            DefKind::Impl { of_trait: false }
        ) || !matches!(
            cx.tcx.def_kind(target_impl),
            DefKind::Impl { of_trait: false }
        ) {
            return false;
        }
        cx.tcx
            .type_of(wrapper_impl)
            .instantiate_identity()
            .ty_adt_def()
            == cx
                .tcx
                .type_of(target_impl)
                .instantiate_identity()
                .ty_adt_def()
    }

    /// Compares receiver types while requiring reference mutability to match exactly.
    fn same_receiver_type<'tcx>(left: Ty<'tcx>, right: Ty<'tcx>) -> bool {
        match (left.kind(), right.kind()) {
            (ty::Ref(_, left, left_mutability), ty::Ref(_, right, right_mutability)) => {
                left == right && left_mutability == right_mutability
            }
            _ => left == right,
        }
    }

    /// Allows only documentation and lint-level attributes that do not change execution.
    fn has_only_nonsemantic_attributes(cx: &LateContext<'_>, hir_id: HirId) -> bool {
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

    /// Removes only syntax that does not alter the returned value.
    fn single_body_expression<'hir>(mut expression: &'hir Expr<'hir>) -> Option<&'hir Expr<'hir>> {
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

    /// Extracts the value from a single explicit `return` statement.
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

    /// Resolves a direct path call to a local function and its authored arguments.
    fn direct_function_call<'hir>(
        cx: &LateContext<'_>,
        callee: &Expr<'_>,
        arguments: &'hir [Expr<'hir>],
    ) -> Option<RedundantWrapperCall<'hir>> {
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };
        let target = cx
            .qpath_res(&path, callee.hir_id)
            .opt_def_id()?
            .as_local()?;
        Some(RedundantWrapperCall {
            target,
            arguments: arguments.iter().collect(),
            is_method: false,
        })
    }

    /// Resolves a method call and prepends its receiver to the forwarded arguments.
    fn direct_method_call<'hir>(
        cx: &LateContext<'_>,
        owner: LocalDefId,
        expression: &Expr<'_>,
        receiver: &'hir Expr<'hir>,
        arguments: &'hir [Expr<'hir>],
    ) -> Option<RedundantWrapperCall<'hir>> {
        let target = cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)?
            .as_local()?;
        let mut forwarded = Vec::with_capacity(arguments.len() + 1);
        forwarded.push(receiver);
        forwarded.extend(arguments);

        // Return the receiver followed by every explicit method argument.
        Some(RedundantWrapperCall {
            target,
            arguments: forwarded,
            is_method: true,
        })
    }

    /// Recognizes a direct local function or associated-function call.
    fn direct_call<'hir>(
        cx: &LateContext<'_>,
        owner: LocalDefId,
        expression: &'hir Expr<'hir>,
    ) -> Option<RedundantWrapperCall<'hir>> {
        let call = match expression.kind {
            ExprKind::Call(callee, arguments) => Self::direct_function_call(cx, callee, arguments)?,
            ExprKind::MethodCall(_, receiver, arguments, _) => {
                Self::direct_method_call(cx, owner, expression, receiver, arguments)?
            }
            _ => return None,
        };
        matches!(cx.tcx.def_kind(call.target), DefKind::Fn | DefKind::AssocFn).then_some(call)
    }

    /// Emits removal guidance relating the wrapper to its real implementation.
    fn emit(&self, cx: &LateContext<'_>) {
        let target_name = cx.tcx.def_path_str(self.target.to_def_id());
        let target_span = cx.tcx.def_span(self.target);

        // Relate the redundant wrapper to the implementation callers should use directly.
        cx.tcx.emit_node_span_lint(
            NEEDLESS_FUNCTION_WRAPPERS,
            self.hir_id,
            self.name_span,
            DiagDecorator(|diag| {
                diag.primary_message(format!(
                    "function `{}` is a redundant forwarding wrapper",
                    self.name
                ));
                diag.span_label(self.name_span, "delete this useless indirection");
                diag.span_label(target_span, format!("forwarded implementation `{target_name}`"));
                diag.help(format!(
                    "remove `{}` and update its callers to invoke `{target_name}` directly; move its documentation or adjust the target's name and visibility if needed",
                    self.name
                ));
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// NeedlessFunctionWrappers: Lint pass
// -----------------------------------------------------------------------------

/// Late lint pass that detects semantically empty forwarding functions.
struct NeedlessFunctionWrappers;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Rejects safe Rust functions and inherent methods that do nothing except pass every argument
    /// unchanged to another function in the same crate.
    ///
    /// ### Why is this bad?
    ///
    /// Pure forwarding wrappers obscure the real implementation, scatter documentation and
    /// visibility decisions, and force readers to follow an indirection with no semantic value.
    ///
    /// For example, `parse` only repeats `parse_document`'s signature and call:
    ///
    /// ```rust
    /// fn parse_document(input: &str) -> usize { input.len() }
    /// fn parse(input: &str) -> usize { parse_document(input) }
    /// ```
    ///
    /// Remove the wrapper and have callers use the implementation directly:
    ///
    /// ```rust
    /// fn parse_document(input: &str) -> usize { input.len() }
    ///
    /// fn main() {
    ///     let length = parse_document("example");
    /// }
    /// ```
    pub NEEDLESS_FUNCTION_WRAPPERS,
    Warn,
    "forbids redundant function forwarding wrappers",
    NeedlessFunctionWrappers
}

impl<'tcx> LateLintPass<'tcx> for NeedlessFunctionWrappers {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let ItemKind::Fn {
            sig,
            ident,
            body,
            has_body: true,
            ..
        } = item.kind
        else {
            return;
        };

        // Analyze the authored body and emit only complete forwarding wrappers.
        let body = cx.tcx.hir_body(body);

        // Discover one direct forwarding target without guessing through adapters.
        let Some(wrapper) = RedundantWrapper::discover(
            cx,
            item.owner_id.def_id,
            item.hir_id(),
            ident.name,
            ident.span,
            sig.header,
            body,
        ) else {
            return;
        };

        // Report the complete redundant wrapper after semantic validation.
        wrapper.emit(cx);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        let ImplItemKind::Fn(signature, body) = item.kind else {
            return;
        };

        // Resolve the enclosing implementation before classifying the method.
        let parent = cx.tcx.local_parent(item.owner_id.def_id);
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(parent) else {
            return;
        };

        // Only analyze inherent methods that are not trait implementations.
        let ItemKind::Impl(implementation) = parent.kind else {
            return;
        };

        // Trait methods intentionally adapt an external contract and are not redundant wrappers.
        if implementation.of_trait.is_some() {
            return;
        }

        // Discover one direct forwarding target without guessing through adapters.
        let Some(wrapper) = RedundantWrapper::discover(
            cx,
            item.owner_id.def_id,
            item.hir_id(),
            item.ident.name,
            item.ident.span,
            signature.header,
            cx.tcx.hir_body(body),
        ) else {
            return;
        };

        // Report the complete redundant wrapper after semantic validation.
        wrapper.emit(cx);
    }
}
