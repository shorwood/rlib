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

use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Derivable manual Serialize implementation
// -----------------------------------------------------------------------------

/// Transparent manual serializer reproducible by a Serde derive.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Authored type or member name involved in the wire contract.
    name: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual transparent serializer for `{}` is derivable",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the complete implementation forwards the sole field unchanged to the provided serializer",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "replace this implementation with `#[derive(serde::Serialize)]` and `#[serde(transparent)]`",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SERDE_MANUAL_SERIALIZE_IMPLS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this serializer is exact transparent forwarding");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// SerdeManualSerializeImpls: Declarative serialization policy
// -----------------------------------------------------------------------------

/// Finds transparent manual serializers reproducible by Serde derives.
struct SerdeManualSerializeImpls;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_MANUAL_SERIALIZE_IMPLS,
    Warn,
    "finds transparent Serialize implementations reproducible by Serde derive",
    SerdeManualSerializeImpls
}

impl SerdeManualSerializeImpls {
    /// Proves that serialization delegates unchanged to a newtype's sole field.
    fn exact_transparent_serializer(
        cx: &LateContext<'_>,
        item: &Item<'_>,
        field_name: &str,
    ) -> bool {
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return false;
        };
        let Ok(implementation) = syn::parse_str::<syn::ItemImpl>(&source) else {
            return false;
        };

        let [syn::ImplItem::Fn(method)] = implementation.items.as_slice() else {
            return false;
        };
        if method.sig.ident != "serialize" {
            return false;
        }

        let [_, syn::FnArg::Typed(serializer)] =
            method.sig.inputs.iter().collect::<Vec<_>>().as_slice()
        else {
            return false;
        };

        let syn::Pat::Ident(serializer) = serializer.pat.as_ref() else {
            return false;
        };
        let [syn::Stmt::Expr(expression, _)] = method.block.stmts.as_slice() else {
            return false;
        };

        let is_field = |expression: &syn::Expr| {
            let expression = match expression {
                syn::Expr::Reference(reference) => reference.expr.as_ref(),
                expression => expression,
            };
            matches!(expression, syn::Expr::Field(field)
            if matches!(field.base.as_ref(), syn::Expr::Path(path) if path.path.is_ident("self"))
                && match &field.member {
                    syn::Member::Unnamed(index) => index.index == 0,
                    syn::Member::Named(name) => name == field_name,
                })
        };
        let is_serializer = |expression: Option<&syn::Expr>| {
            matches!(expression, Some(syn::Expr::Path(path))
                if path.path.is_ident(&serializer.ident))
        };

        match expression {
            syn::Expr::MethodCall(call) => {
                call.method == "serialize"
                    && call.args.len() == 1
                    && is_field(&call.receiver)
                    && is_serializer(call.args.first())
            }
            syn::Expr::Call(call) => {
                matches!(call.func.as_ref(), syn::Expr::Path(path)
                    if path.qself.is_some()
                        && path.path.segments.last().is_some_and(|segment| segment.ident == "serialize"))
                    && call.args.len() == 2
                    && call.args.first().is_some_and(is_field)
                    && is_serializer(call.args.iter().nth(1))
            }
            _ => false,
        }
    }
}
impl LateLintPass<'_> for SerdeManualSerializeImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };
        if item.span.from_expansion() {
            return;
        }

        let Some(trait_id) = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())
        else {
            return;
        };

        if cx.tcx.item_name(trait_id).as_str() != "Serialize"
            || !matches!(
                cx.tcx.crate_name(trait_id.krate).as_str(),
                "serde" | "serde_core"
            )
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
            || !Self::exact_transparent_serializer(
                cx,
                item,
                definition
                    .non_enum_variant()
                    .fields
                    .iter()
                    .next()
                    .expect("single-field shape was checked")
                    .name
                    .as_str(),
            )
        {
            return;
        }

        Violation {
            owner: item.hir_id(),
            span: item.span,
            name: cx.tcx.item_name(definition.did()).to_string(),
        }
        .emit(cx);
    }
}
