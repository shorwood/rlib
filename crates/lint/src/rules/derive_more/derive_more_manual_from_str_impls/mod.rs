extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::def_id::DefId;
use rustc_hir::{Expr, ExprKind, ImplItem, ImplItemKind, ItemKind, Node, PatKind, StructTailExpr};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::symbol::sym;

use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

// -----------------------------------------------------------------------------
// Violation: Derivable string parser implementation
// -----------------------------------------------------------------------------

/// Transparent parsing implementation reproducible by `derive_more`.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Local type name used to identify the affected derive contract.
    name: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual transparent parser for `{}` is derivable",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the implementation forwards the complete input to the sole field's parser and maps its unchanged error directly into the wrapper",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "replace this implementation with `#[derive(derive_more::FromStr)]` on the newtype",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            DERIVE_MORE_MANUAL_FROM_STR_IMPLS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this parser is exact newtype forwarding");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// DeriveMoreManualFromStrImpls: Declarative parsing policy
// -----------------------------------------------------------------------------

/// Finds transparent parsing implementations reproducible by `derive_more`.
struct DeriveMoreManualFromStrImpls;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_FROM_STR_IMPLS,
    Warn,
    "finds transparent FromStr implementations reproducible by derive_more",
    DeriveMoreManualFromStrImpls
}

impl DeriveMoreManualFromStrImpls {
    /// Returns whether a resolved path names the wrapper definition.
    fn resolution_targets(cx: &LateContext<'_>, resolution: Res, definition: DefId) -> bool {
        match resolution {
            Res::Def(_, target) => {
                target == definition || cx.tcx.opt_parent(target) == Some(definition)
            }
            Res::SelfCtor(implementation)
            | Res::SelfTyAlias {
                alias_to: implementation,
                ..
            } => cx
                .tcx
                .type_of(implementation)
                .instantiate_identity()
                .ty_adt_def()
                .is_some_and(|target| target.did() == definition),
            _ => false,
        }
    }

    /// Returns whether a construction path names the wrapper definition.
    fn path_targets(cx: &LateContext<'_>, expression: &Expr<'_>, definition: DefId) -> bool {
        let ExprKind::Path(path) = expression.kind else {
            return false;
        };
        Self::resolution_targets(cx, cx.qpath_res(&path, expression.hir_id), definition)
    }

    /// Recognizes a direct tuple constructor or one-field named constructor mapper.
    fn exact_mapper(
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
        definition: ty::AdtDef<'_>,
    ) -> bool {
        if Self::path_targets(cx, expression, definition.did()) {
            return true;
        }
        let ExprKind::Closure(closure) = expression.kind else {
            return false;
        };
        let body = cx.tcx.hir_body(closure.body);
        let [parameter] = body.params else {
            return false;
        };
        let PatKind::Binding(_, binding, _, None) = parameter.pat.kind else {
            return false;
        };
        let Some(value) = DirectForwarding::single_body_expression(body.value) else {
            return false;
        };
        let Some(sole_field) = definition.non_enum_variant().fields.iter().next() else {
            return false;
        };
        match value.kind {
            ExprKind::Struct(path, [field], StructTailExpr::None) => {
                Self::resolution_targets(cx, cx.qpath_res(path, value.hir_id), definition.did())
                    && sole_field.name == field.ident.name
                    && DirectForwarding::is_binding(cx, field.expr, binding)
            }
            ExprKind::Call(constructor, [argument]) => {
                Self::path_targets(cx, constructor, definition.did())
                    && DirectForwarding::is_binding(cx, argument, binding)
            }
            _ => false,
        }
    }

    /// Proves that `FromStr` only parses and wraps a single field.
    fn exact_forwarding<'tcx>(
        cx: &LateContext<'tcx>,
        item: &ImplItem<'_>,
        body_id: rustc_hir::BodyId,
        definition: ty::AdtDef<'tcx>,
        arguments: ty::GenericArgsRef<'tcx>,
        trait_id: DefId,
    ) -> bool {
        let body = cx.tcx.hir_body(body_id);
        let [parameter] = body.params else {
            return false;
        };
        let PatKind::Binding(_, binding, _, None) = parameter.pat.kind else {
            return false;
        };
        let Some(expression) = DirectForwarding::single_body_expression(body.value) else {
            return false;
        };
        let ExprKind::MethodCall(_, parsed, [mapper], _) = expression.kind else {
            return false;
        };
        let typeck = cx.tcx.typeck(item.owner_id.def_id);
        let Some(map) = typeck.type_dependent_def_id(expression.hir_id) else {
            return false;
        };
        let ty::Adt(result, result_arguments) = typeck.expr_ty(parsed).kind() else {
            return false;
        };
        let Some(sole_field) = definition.non_enum_variant().fields.iter().next() else {
            return false;
        };
        if cx.tcx.item_name(map).as_str() != "map"
            || !cx.tcx.is_diagnostic_item(sym::Result, result.did())
            || result_arguments.type_at(0) != sole_field.ty(cx.tcx, arguments)
            || !Self::exact_mapper(cx, mapper, definition)
        {
            return false;
        }

        match parsed.kind {
            ExprKind::MethodCall(_, receiver, [], _) => {
                typeck
                    .type_dependent_def_id(parsed.hir_id)
                    .is_some_and(|target| cx.tcx.item_name(target).as_str() == "parse")
                    && DirectForwarding::is_binding(cx, receiver, binding)
            }
            ExprKind::Call(callee, [input]) => {
                let ExprKind::Path(path) = callee.kind else {
                    return false;
                };
                cx.qpath_res(&path, callee.hir_id)
                    .opt_def_id()
                    .is_some_and(|target| cx.tcx.trait_of_assoc(target) == Some(trait_id))
                    && DirectForwarding::is_binding(cx, input, binding)
            }
            _ => false,
        }
    }
}
impl LateLintPass<'_> for DeriveMoreManualFromStrImpls {
    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        let ImplItemKind::Fn(_, body_id) = item.kind else {
            return;
        };
        if item.ident.name.as_str() != "from_str"
            || item.span.from_expansion()
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
        if !cx.tcx.hir_attrs(parent.hir_id()).is_empty() {
            return;
        }
        let Some(trait_id) = implementation_item
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())
        else {
            return;
        };
        if cx.tcx.crate_name(trait_id.krate).as_str() != "core"
            || cx.tcx.item_name(trait_id).as_str() != "FromStr"
        {
            return;
        }
        let trait_ref = cx.tcx.impl_trait_ref(implementation).instantiate_identity();
        let ty::Adt(definition, arguments) = trait_ref.self_ty().kind() else {
            return;
        };

        if !definition.is_struct()
            || definition.non_enum_variant().fields.len() != 1
            || !Self::exact_forwarding(cx, item, body_id, *definition, arguments, trait_id)
        {
            return;
        }

        Violation {
            owner: item.hir_id(),
            span: parent.span,
            name: cx.tcx.item_name(definition.did()).to_string(),
        }
        .emit(cx);
    }
}
