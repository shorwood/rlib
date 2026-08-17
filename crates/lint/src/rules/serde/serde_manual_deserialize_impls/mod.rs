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
// Violation: Derivable manual Deserialize implementation
// -----------------------------------------------------------------------------

/// Transparent manual deserializer reproducible by a Serde derive.
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
            "manual transparent deserializer for `{}` is derivable",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the complete implementation decodes the sole field unchanged and immediately constructs the wrapper",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "replace this implementation with `#[derive(serde::Deserialize)]` and `#[serde(transparent)]`",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SERDE_MANUAL_DESERIALIZE_IMPLS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this deserializer is exact transparent forwarding",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// SerdeManualDeserializeImpls: Declarative deserialization policy
// -----------------------------------------------------------------------------

/// Finds transparent manual deserializers reproducible by Serde derives.
struct SerdeManualDeserializeImpls;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_MANUAL_DESERIALIZE_IMPLS,
    Warn,
    "finds transparent Deserialize implementations reproducible by Serde derive",
    SerdeManualDeserializeImpls
}

impl SerdeManualDeserializeImpls {
    /// Proves that deserialization delegates to one field and wraps the result unchanged.
    fn exact_transparent_deserializer(cx: &LateContext<'_>, item: &Item<'_>) -> bool {
        // Missing authored source cannot prove an exact manual implementation.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return false;
        };

        // Unparseable implementation text cannot be compared structurally.
        let Ok(implementation) = syn::parse_str::<syn::ItemImpl>(&source) else {
            return false;
        };

        // A transparent derive replaces only a solitary deserialization method.
        let [syn::ImplItem::Fn(method)] = implementation.items.as_slice() else {
            return false;
        };

        // Other methods do not implement the deserialization entry point.
        if method.sig.ident != "deserialize" {
            return false;
        }

        // Transparent deserialization accepts exactly one typed deserializer argument.
        let [syn::FnArg::Typed(deserializer)] =
            method.sig.inputs.iter().collect::<Vec<_>>().as_slice()
        else {
            return false;
        };

        // The deserializer must have a stable binding that can be tracked into the call.
        let syn::Pat::Ident(deserializer) = deserializer.pat.as_ref() else {
            return false;
        };

        // Match a successful single-field construction around one direct deserialize call.
        // Additional statements encode behavior beyond transparent forwarding.
        let [syn::Stmt::Expr(syn::Expr::Call(ok), _)] = method.block.stmts.as_slice() else {
            return false;
        };

        // The body must return exactly one successful wrapper value.
        if !matches!(ok.func.as_ref(), syn::Expr::Path(path)
            if path.path.segments.last().is_some_and(|segment| segment.ident == "Ok"))
            || ok.args.len() != 1
        {
            return false;
        }

        // Extract the single-field construction nested beneath the successful Result.
        let decoded = match ok.args.first() {
            Some(syn::Expr::Call(construction))
                if matches!(construction.func.as_ref(), syn::Expr::Path(path)
                    if path.path.is_ident("Self"))
                    && construction.args.len() == 1 =>
            {
                construction.args.first()
            }
            Some(syn::Expr::Struct(construction))
                if construction.path.is_ident("Self")
                    && construction.fields.len() == 1
                    && construction.rest.is_none() =>
            {
                construction.fields.first().map(|field| &field.expr)
            }
            _ => None,
        };

        // The wrapped value must propagate failure from its decode operation directly.
        let Some(syn::Expr::Try(decoded)) = decoded else {
            return false;
        };

        // Require direct forwarding to the standard deserialize operation.
        // Noncall expressions perform work a transparent derive cannot reproduce.
        let syn::Expr::Call(decode) = decoded.expr.as_ref() else {
            return false;
        };

        decode.args.len() == 1
            && matches!(decode.func.as_ref(), syn::Expr::Path(path)
            if path.path.segments.last().is_some_and(|segment| segment.ident == "deserialize"))
            && matches!(decode.args.first(), Some(syn::Expr::Path(path))
            if path.path.is_ident(&deserializer.ident))
    }
}
impl LateLintPass<'_> for SerdeManualDeserializeImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Only implementation items can define a manual deserializer.
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };

        // Generated implementations are outside authored derive policy.
        if item.span.from_expansion() {
            return;
        }

        // Inherent implementations do not implement the Serde contract.
        let Some(trait_id) = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())
        else {
            return;
        };

        // Exclude implementations of unrelated traits with similar method shapes.
        if cx.tcx.item_name(trait_id).as_str() != "Deserialize"
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

        // Transparent deserialization applies only to algebraic wrapper types.
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return;
        };

        // Require a single-field struct and exact forwarding behavior.
        if !definition.is_struct()
            || definition.non_enum_variant().fields.len() != 1
            || !Self::exact_transparent_deserializer(cx, item)
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
