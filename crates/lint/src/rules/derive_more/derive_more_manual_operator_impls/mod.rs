extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::Span;

use super::contracts::authored_item_source;
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    name: String,
    derive: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual `{}` implementation for `{}` is derivable",
            self.derive, self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the implementation applies the corresponding operator to the sole field and reconstructs the wrapper without additional policy",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "replace this implementation with `#[derive(derive_more::{})]`",
            self.derive
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            DERIVE_MORE_MANUAL_OPERATOR_IMPLS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this operator is exact newtype forwarding");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct DeriveMoreManualOperatorImpls;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_OPERATOR_IMPLS,
    Warn,
    "finds newtype operator implementations reproducible by derive_more",
    DeriveMoreManualOperatorImpls
}

impl LateLintPass<'_> for DeriveMoreManualOperatorImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };
        if item.span.from_expansion() || !cx.tcx.hir_attrs(item.hir_id()).is_empty() {
            return;
        }
        let Some(trait_id) = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())
        else {
            return;
        };
        let derive_name = cx.tcx.item_name(trait_id);
        let derive = derive_name.as_str();
        if cx.tcx.crate_name(trait_id.krate).as_str() != "core"
            || operator(derive).is_none()
        {
            return;
        }
        let trait_ref = cx
            .tcx
            .impl_trait_ref(item.owner_id.def_id)
            .instantiate_identity();
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return;
        };
        if !definition.is_struct()
            || definition.non_enum_variant().fields.len() != 1
            || !cx.tcx.generics_of(definition.did()).own_params.is_empty()
            || !exact_operator_source(cx, item, derive)
        {
            return;
        }
        Violation {
            owner: item.hir_id(),
            span: item.span,
            name: cx.tcx.item_name(definition.did()).to_string(),
            derive: derive.to_owned(),
        }
        .emit(cx);
    }
}

fn exact_operator_source(cx: &LateContext<'_>, item: &Item<'_>, derive: &str) -> bool {
    let Some(source) = authored_item_source(cx, item) else {
        return false;
    };
    let Ok(implementation) = syn::parse_str::<syn::ItemImpl>(&source) else {
        return false;
    };
    let Some(operator) = operator(derive) else {
        return false;
    };
    match operator {
        Operator::Binary(expected) => {
            let Some(method) = output_method(&implementation.items, derive) else {
                return false;
            };
            let Some(argument) = constructed_argument(method) else {
                return false;
            };
            matches!(argument, syn::Expr::Binary(operation)
                if expected(&operation.op)
                    && field_of(&operation.left, "self")
                    && field_of(&operation.right, "rhs"))
        }
        Operator::Unary(expected) => {
            let Some(method) = output_method(&implementation.items, derive) else {
                return false;
            };
            let Some(argument) = constructed_argument(method) else {
                return false;
            };
            matches!(argument, syn::Expr::Unary(operation)
                if expected(&operation.op) && field_of(&operation.expr, "self"))
        }
        Operator::Assignment(expected) => {
            let [syn::ImplItem::Fn(method)] = implementation.items.as_slice() else {
                return false;
            };
            if method.sig.ident != operator_method(derive) {
                return false;
            }
            let [syn::Stmt::Expr(syn::Expr::Binary(operation), _)] = method.block.stmts.as_slice()
            else {
                return false;
            };
            expected(&operation.op)
                && field_of(&operation.left, "self")
                && field_of(&operation.right, "rhs")
        }
    }
}

fn output_method<'a>(items: &'a [syn::ImplItem], derive: &str) -> Option<&'a syn::ImplItemFn> {
    let [syn::ImplItem::Type(output), syn::ImplItem::Fn(method)] = items else {
        return None;
    };
    (output.ident == "Output"
        && matches!(&output.ty, syn::Type::Path(path) if path.path.is_ident("Self"))
        && method.sig.ident == operator_method(derive))
    .then_some(method)
}

fn constructed_argument(method: &syn::ImplItemFn) -> Option<&syn::Expr> {
    let [syn::Stmt::Expr(syn::Expr::Call(construction), _)] = method.block.stmts.as_slice() else {
        return None;
    };
    if !matches!(construction.func.as_ref(), syn::Expr::Path(path) if path.path.is_ident("Self"))
        || construction.args.len() != 1
    {
        return None;
    }
    construction.args.first()
}

enum Operator {
    Binary(fn(&syn::BinOp) -> bool),
    Unary(fn(&syn::UnOp) -> bool),
    Assignment(fn(&syn::BinOp) -> bool),
}

fn operator(derive: &str) -> Option<Operator> {
    Some(match derive {
        "Add" => Operator::Binary(|value| matches!(value, syn::BinOp::Add(_))),
        "Sub" => Operator::Binary(|value| matches!(value, syn::BinOp::Sub(_))),
        "Mul" => Operator::Binary(|value| matches!(value, syn::BinOp::Mul(_))),
        "Div" => Operator::Binary(|value| matches!(value, syn::BinOp::Div(_))),
        "Rem" => Operator::Binary(|value| matches!(value, syn::BinOp::Rem(_))),
        "BitAnd" => Operator::Binary(|value| matches!(value, syn::BinOp::BitAnd(_))),
        "BitOr" => Operator::Binary(|value| matches!(value, syn::BinOp::BitOr(_))),
        "BitXor" => Operator::Binary(|value| matches!(value, syn::BinOp::BitXor(_))),
        "Shl" => Operator::Binary(|value| matches!(value, syn::BinOp::Shl(_))),
        "Shr" => Operator::Binary(|value| matches!(value, syn::BinOp::Shr(_))),
        "Neg" => Operator::Unary(|value| matches!(value, syn::UnOp::Neg(_))),
        "Not" => Operator::Unary(|value| matches!(value, syn::UnOp::Not(_))),
        "AddAssign" => Operator::Assignment(|value| matches!(value, syn::BinOp::AddAssign(_))),
        "SubAssign" => Operator::Assignment(|value| matches!(value, syn::BinOp::SubAssign(_))),
        "MulAssign" => Operator::Assignment(|value| matches!(value, syn::BinOp::MulAssign(_))),
        "DivAssign" => Operator::Assignment(|value| matches!(value, syn::BinOp::DivAssign(_))),
        "RemAssign" => Operator::Assignment(|value| matches!(value, syn::BinOp::RemAssign(_))),
        "BitAndAssign" => Operator::Assignment(|value| matches!(value, syn::BinOp::BitAndAssign(_))),
        "BitOrAssign" => Operator::Assignment(|value| matches!(value, syn::BinOp::BitOrAssign(_))),
        "BitXorAssign" => Operator::Assignment(|value| matches!(value, syn::BinOp::BitXorAssign(_))),
        "ShlAssign" => Operator::Assignment(|value| matches!(value, syn::BinOp::ShlAssign(_))),
        "ShrAssign" => Operator::Assignment(|value| matches!(value, syn::BinOp::ShrAssign(_))),
        _ => return None,
    })
}

fn operator_method(derive: &str) -> &str {
    match derive {
        "Add" => "add",
        "Sub" => "sub",
        "Mul" => "mul",
        "Div" => "div",
        "Rem" => "rem",
        "BitAnd" => "bitand",
        "BitOr" => "bitor",
        "BitXor" => "bitxor",
        "Shl" => "shl",
        "Shr" => "shr",
        "Neg" => "neg",
        "Not" => "not",
        "AddAssign" => "add_assign",
        "SubAssign" => "sub_assign",
        "MulAssign" => "mul_assign",
        "DivAssign" => "div_assign",
        "RemAssign" => "rem_assign",
        "BitAndAssign" => "bitand_assign",
        "BitOrAssign" => "bitor_assign",
        "BitXorAssign" => "bitxor_assign",
        "ShlAssign" => "shl_assign",
        "ShrAssign" => "shr_assign",
        _ => "",
    }
}

fn field_of(expression: &syn::Expr, receiver: &str) -> bool {
    matches!(
        expression,
        syn::Expr::Field(field)
            if matches!(field.base.as_ref(), syn::Expr::Path(path) if path.path.is_ident(receiver))
                && matches!(&field.member, syn::Member::Unnamed(index) if index.index == 0)
    )
}
