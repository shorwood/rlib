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

/// Resolved `SQLx` API operation.
pub struct SqlxOperation<'hir> {
    pub definition: DefId,
    pub name: String,
    pub receiver: Option<&'hir Expr<'hir>>,
    pub arguments: &'hir [Expr<'hir>],
}

/// Returns whether a definition belongs to `SQLx`'s public or core implementation crate.
pub fn is_sqlx_definition(cx: &LateContext<'_>, definition: DefId) -> bool {
    matches!(
        cx.tcx.crate_name(definition.krate).as_str(),
        "sqlx" | "sqlx_core"
    )
}

/// Resolves an authored function, tuple-struct constructor, or method call into `SQLx`.
pub fn operation<'hir>(
    cx: &LateContext<'_>,
    expression: &'hir Expr<'hir>,
) -> Option<SqlxOperation<'hir>> {
    match expression.kind {
        ExprKind::MethodCall(segment, receiver, arguments, _) => {
            let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
            let definition = cx
                .tcx
                .typeck(owner)
                .type_dependent_def_id(expression.hir_id)?;
            is_sqlx_definition(cx, definition).then_some(SqlxOperation {
                definition,
                name: segment.ident.name.to_string(),
                receiver: Some(receiver),
                arguments,
            })
        }
        ExprKind::Call(callee, arguments) => {
            let ExprKind::Path(path) = callee.kind else {
                return None;
            };
            let Res::Def(_, definition) = cx.qpath_res(&path, callee.hir_id) else {
                return None;
            };
            is_sqlx_definition(cx, definition).then_some(SqlxOperation {
                definition,
                name: cx.tcx.item_name(definition).to_string(),
                receiver: None,
                arguments,
            })
        }
        _ => None,
    }
}

/// Resolves an expression to a local binding.
pub const fn local_binding(expression: &Expr<'_>) -> Option<HirId> {
    let ExprKind::Path(QPath::Resolved(_, path)) = expression.kind else {
        return None;
    };
    match path.res {
        Res::Local(binding) => Some(binding),
        _ => None,
    }
}

/// Resolves the local binding at the root of a receiver chain.
pub fn root_local(expression: &Expr<'_>) -> Option<HirId> {
    if let Some(binding) = local_binding(expression) {
        return Some(binding);
    }
    match expression.kind {
        ExprKind::MethodCall(_, receiver, _, _)
        | ExprKind::DropTemps(receiver)
        | ExprKind::AddrOf(_, _, receiver)
        | ExprKind::Unary(_, receiver) => root_local(receiver),
        _ => None,
    }
}

/// Decodes a directly authored or trivially wrapped static string.
pub fn static_string(expression: &Expr<'_>) -> Option<String> {
    match expression.kind {
        ExprKind::Lit(literal) => match literal.node {
            LitKind::Str(value, _) => Some(value.to_string()),
            _ => None,
        },
        ExprKind::DropTemps(inner)
        | ExprKind::AddrOf(_, _, inner)
        | ExprKind::Cast(inner, _)
        | ExprKind::Type(inner, _) => static_string(inner),
        ExprKind::Binary(operator, left, right) if operator.node == rustc_hir::BinOpKind::Add => {
            let mut value = static_string(left)?;
            value.push_str(&static_string(right)?);
            Some(value)
        }
        _ => None,
    }
}

/// Returns whether a type is a primitive/string carrier rather than a typed SQL fragment.
pub fn is_unstructured_value(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
    let ty = cx.typeck_results().expr_ty_adjusted(expression).peel_refs();
    ty.is_primitive()
        || ty.is_str()
        || matches!(ty.kind(), ty::Adt(definition, _) if {
            let crate_name = cx.tcx.crate_name(definition.did().krate);
            let name = cx.tcx.item_name(definition.did());
            crate_name.as_str() == "alloc"
                && matches!(name.as_str(), "String" | "Cow" | "Box" | "Arc")
        })
}

/// Finds an `SQLx` macro expansion and returns its public name and authored call site.
pub fn sqlx_macro(cx: &LateContext<'_>, span: Span) -> Option<(String, Span)> {
    span.macro_backtrace().find_map(|expansion| {
        let definition = expansion.macro_def_id?;
        (cx.tcx.crate_name(definition.krate).as_str() == "sqlx").then(|| {
            (
                cx.tcx.item_name(definition).to_string(),
                expansion.call_site,
            )
        })
    })
}

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
