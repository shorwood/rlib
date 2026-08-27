extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_ast::LitKind;
use rustc_hir::def::Res;
use rustc_hir::{Expr, ExprKind, HirId, QPath};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::DefId;

/// Returns whether a definition belongs to `SQLx`'s public or core implementation crate.
fn is_sqlx_definition(cx: &LateContext<'_>, definition: DefId) -> bool {
    matches!(
        cx.tcx.crate_name(definition.krate).as_str(),
        "sqlx" | "sqlx_core"
    )
}

// -----------------------------------------------------------------------------
// SqlxOperation: Resolved SQLx call
// -----------------------------------------------------------------------------

/// Resolved `SQLx` API operation.
pub struct SqlxOperation<'hir> {
    /// Compiler definition identifying the called API.
    pub definition: DefId,
    /// Public function or method name.
    pub name: String,
    /// Method receiver, absent for free functions and constructors.
    pub receiver: Option<&'hir Expr<'hir>>,
    /// Authored call arguments excluding the method receiver.
    pub arguments: &'hir [Expr<'hir>],
}

impl<'hir> SqlxOperation<'hir> {
    /// Resolves an authored function, tuple-struct constructor, or method call into `SQLx`.
    fn resolve(cx: &LateContext<'_>, expression: &'hir Expr<'hir>) -> Option<Self> {
        match expression.kind {
            ExprKind::MethodCall(segment, receiver, arguments, _) => {
                let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
                let definition = cx
                    .tcx
                    .typeck(owner)
                    .type_dependent_def_id(expression.hir_id)?;
                is_sqlx_definition(cx, definition).then_some(Self {
                    definition,
                    name: segment.ident.name.to_string(),
                    receiver: Some(receiver),
                    arguments,
                })
            }
            ExprKind::Call(callee, arguments) => {
                // Indirect callees cannot be resolved to one SQLx definition.
                let ExprKind::Path(path) = callee.kind else {
                    return None;
                };

                // Paths without a function or constructor definition are not SQLx calls.
                let Res::Def(_, definition) = cx.qpath_res(&path, callee.hir_id) else {
                    return None;
                };
                is_sqlx_definition(cx, definition).then_some(Self {
                    definition,
                    name: cx.tcx.item_name(definition).to_string(),
                    receiver: None,
                    arguments,
                })
            }
            _ => None,
        }
    }
}

// -----------------------------------------------------------------------------
// SqlxExprExt: SQLx-aware expression analysis
// -----------------------------------------------------------------------------

/// SQLx analysis operations attached to their Rust expression subject.
pub trait SqlxExprExt<'hir> {
    /// Resolves this expression into a SQLx operation.
    fn sqlx_operation(&'hir self, cx: &LateContext<'_>) -> Option<SqlxOperation<'hir>>;

    /// Resolves this expression to a local binding.
    fn local_binding(&self) -> Option<HirId>;

    /// Resolves the local binding at the root of this receiver chain.
    fn root_local(&self) -> Option<HirId>;

    /// Decodes a directly authored or trivially wrapped static string.
    fn static_string(&self) -> Option<String>;

    /// Returns whether this expression is a primitive carrier rather than a typed SQL fragment.
    fn is_unstructured_value(&self, cx: &LateContext<'_>) -> bool;
}

impl<'hir> SqlxExprExt<'hir> for Expr<'hir> {
    fn sqlx_operation(&'hir self, cx: &LateContext<'_>) -> Option<SqlxOperation<'hir>> {
        SqlxOperation::resolve(cx, self)
    }

    fn local_binding(&self) -> Option<HirId> {
        // Only a resolved path can identify one local binding.
        let ExprKind::Path(QPath::Resolved(_, path)) = self.kind else {
            return None;
        };
        match path.res {
            Res::Local(binding) => Some(binding),
            _ => None,
        }
    }

    fn root_local(&self) -> Option<HirId> {
        // A direct local path completes receiver-root discovery.
        if let Some(binding) = self.local_binding() {
            return Some(binding);
        }
        match self.kind {
            ExprKind::MethodCall(_, receiver, _, _)
            | ExprKind::DropTemps(receiver)
            | ExprKind::AddrOf(_, _, receiver)
            | ExprKind::Unary(_, receiver) => receiver.root_local(),
            _ => None,
        }
    }

    fn static_string(&self) -> Option<String> {
        match self.kind {
            ExprKind::Lit(literal) => match literal.node {
                LitKind::Str(value, _) => Some(value.to_string()),
                _ => None,
            },
            ExprKind::DropTemps(inner)
            | ExprKind::AddrOf(_, _, inner)
            | ExprKind::Cast(inner, _)
            | ExprKind::Type(inner, _) => inner.static_string(),
            ExprKind::Binary(operator, left, right)
                if operator.node == rustc_hir::BinOpKind::Add =>
            {
                let mut value = left.static_string()?;
                value.push_str(&right.static_string()?);
                Some(value)
            }
            _ => None,
        }
    }

    fn is_unstructured_value(&self, cx: &LateContext<'_>) -> bool {
        let ty = cx.typeck_results().expr_ty_adjusted(self).peel_refs();
        ty.is_primitive()
            || ty.is_str()
            || matches!(ty.kind(), ty::Adt(definition, _) if {
                let crate_name = cx.tcx.crate_name(definition.did().krate);
                let name = cx.tcx.item_name(definition.did());
                crate_name.as_str() == "alloc"
                    && matches!(name.as_str(), "String" | "Cow" | "Box" | "Arc")
            })
    }
}

// -----------------------------------------------------------------------------
// SqlxMacro: Authored macro identity
// -----------------------------------------------------------------------------

/// Public SQLx macro name paired with its authored call site.
pub struct SqlxMacroCall {
    /// Public macro name.
    pub name: String,
    /// Span written by the user before macro expansion.
    pub call_site: Span,
}

/// SQLx macro lookup behavior attached to the expansion span being inspected.
pub trait SqlxMacroSpanExt {
    /// Finds the nearest SQLx macro expansion containing this span.
    fn sqlx_macro(self, cx: &LateContext<'_>) -> Option<SqlxMacroCall>;
}

impl SqlxMacroSpanExt for Span {
    fn sqlx_macro(self, cx: &LateContext<'_>) -> Option<SqlxMacroCall> {
        self.macro_backtrace().find_map(|expansion| {
            let definition = expansion.macro_def_id?;
            (cx.tcx.crate_name(definition.krate).as_str() == "sqlx").then(|| SqlxMacroCall {
                name: cx.tcx.item_name(definition).to_string(),
                call_site: expansion.call_site,
            })
        })
    }
}

// -----------------------------------------------------------------------------
// QueryExecution: Executor method vocabulary
// -----------------------------------------------------------------------------

/// Recognizes methods that send a query to a database executor.
pub fn is_query_execution(name: &str) -> bool {
    matches!(
        name,
        "execute"
            | "execute_many"
            | "fetch"
            | "fetch_all"
            | "fetch_many"
            | "fetch_one"
            | "fetch_optional"
    )
}
