extern crate rustc_abi;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_abi::ExternAbi;
use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::{
    Constness, ExprKind, ImplItem, ImplItemKind, ItemKind, Node, PatKind, StructTailExpr,
};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    type_name: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual structural constructor for `{}` is derivable",
            self.type_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "this public constant constructor only assigns its parameters to the corresponding fields",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "replace this implementation with `#[derive(derive_more::Constructor)]` on the struct",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            DERIVE_MORE_MANUAL_CONSTRUCTORS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this `new` method is exact field assembly");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct DeriveMoreManualConstructors;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_CONSTRUCTORS,
    Warn,
    "finds structural constructors reproducible by derive_more",
    DeriveMoreManualConstructors
}

impl LateLintPass<'_> for DeriveMoreManualConstructors {
    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return;
        };
        if item.ident.name.as_str() != "new"
            || item.span.from_expansion()
            || signature.decl.implicit_self.has_implicit_self()
            || signature.header.constness != Constness::Const
            || signature.header.is_unsafe()
            || signature.header.abi != ExternAbi::Rust
            || !item.generics.params.is_empty()
            || !cx.tcx.visibility(item.owner_id.def_id).is_public()
            || !cx.tcx.hir_attrs(item.hir_id()).is_empty()
        {
            return;
        }
        let implementation = cx.tcx.local_parent(item.owner_id.def_id);
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
            return;
        };
        let ItemKind::Impl(implementation_item) = parent.kind else {
            return;
        };
        if implementation_item.of_trait.is_some() {
            return;
        }
        let Some(definition) = cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .ty_adt_def()
        else {
            return;
        };
        if !definition.is_struct() {
            return;
        }
        let body = cx.tcx.hir_body(body_id);
        let Some(bindings) = parameter_bindings(body) else {
            return;
        };
        let Some(expression) = DirectForwarding::single_body_expression(body.value) else {
            return;
        };
        if !exact_field_assembly(
            cx,
            definition.did(),
            item.owner_id.def_id,
            &bindings,
            expression,
        ) {
            return;
        }
        Violation {
            owner: item.hir_id(),
            span: item.span,
            type_name: cx.tcx.item_name(definition.did()),
        }
        .emit(cx);
    }
}

fn parameter_bindings(body: &rustc_hir::Body<'_>) -> Option<Vec<(rustc_hir::HirId, Symbol)>> {
    body.params
        .iter()
        .map(|parameter| {
            let PatKind::Binding(_, binding, ident, None) = parameter.pat.kind else {
                return None;
            };
            Some((binding, ident.name))
        })
        .collect()
}

fn exact_field_assembly(
    cx: &LateContext<'_>,
    definition: rustc_hir::def_id::DefId,
    owner: rustc_hir::def_id::LocalDefId,
    bindings: &[(rustc_hir::HirId, Symbol)],
    expression: &rustc_hir::Expr<'_>,
) -> bool {
    match expression.kind {
        ExprKind::Struct(path, fields, StructTailExpr::None) => {
            path_targets(cx, cx.qpath_res(&path, expression.hir_id), definition)
                && fields.len() == bindings.len()
                && fields.iter().all(|field| {
                    bindings.iter().any(|(binding, name)| {
                        field.ident.name == *name
                            && DirectForwarding::is_binding(cx, field.expr, *binding)
                    })
                })
        }
        ExprKind::Call(_, arguments) => {
            DirectForwarding::call(cx, owner, expression).is_some_and(|call| {
                call.target == definition
                    && arguments.len() == bindings.len()
                    && arguments
                        .iter()
                        .zip(bindings)
                        .all(|(argument, (binding, _))| {
                            DirectForwarding::is_binding(cx, argument, *binding)
                        })
            })
        }
        _ => false,
    }
}

fn path_targets(
    cx: &LateContext<'_>,
    resolution: Res,
    definition: rustc_hir::def_id::DefId,
) -> bool {
    match resolution {
        Res::Def(_, target) => target == definition,
        Res::SelfTyAlias { alias_to, .. } => cx
            .tcx
            .type_of(alias_to)
            .instantiate_identity()
            .ty_adt_def()
            .is_some_and(|target| target.did() == definition),
        _ => false,
    }
}
