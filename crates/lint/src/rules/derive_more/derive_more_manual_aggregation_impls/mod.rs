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
// Violation: Derivable aggregation implementation
// -----------------------------------------------------------------------------

/// Transparent `Sum` or `Product` implementation reproducible by `derive_more`.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Local type name used to identify the affected derive contract.
    name: String,
    /// `derive_more` macro capable of replacing the implementation.
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
            "the implementation maps the sole field and delegates directly to the same aggregation without changing identity or failure behavior",
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
            DERIVE_MORE_MANUAL_AGGREGATION_IMPLS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this aggregation is exact newtype forwarding");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// AggregationNames: Compared aggregation identity
// -----------------------------------------------------------------------------

/// Derive and wrapper names used to compare an authored aggregation implementation.
struct AggregationNames<'name> {
    /// Standard aggregation trait name.
    derive: &'name str,
    /// Authored wrapper type name.
    type_name: &'name str,
}

// -----------------------------------------------------------------------------
// DeriveMoreManualAggregationImpls: Declarative aggregation policy
// -----------------------------------------------------------------------------

/// Finds `Sum` and `Product` implementations reproducible by `derive_more`.
struct DeriveMoreManualAggregationImpls;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_AGGREGATION_IMPLS,
    Warn,
    "finds Sum and Product implementations reproducible by derive_more",
    DeriveMoreManualAggregationImpls
}

impl DeriveMoreManualAggregationImpls {
    /// Proves that aggregation maps inputs to the sole field and wraps the result unchanged.
    fn is_exact_aggregation_source(
        cx: &LateContext<'_>,
        item: &Item<'_>,
        names: &AggregationNames<'_>,
        field_name: Option<&str>,
    ) -> bool {
        let AggregationNames { derive, type_name } = names;

        // Implementations without authored source cannot prove exact aggregation forwarding.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return false;
        };

        // Unparseable source cannot establish the authored aggregation shape.
        let Ok(implementation) = syn::parse_str::<syn::ItemImpl>(&source) else {
            return false;
        };

        // Derivable aggregation implementations contain exactly one trait method.
        let [syn::ImplItem::Fn(method)] = implementation.items.as_slice() else {
            return false;
        };
        let expected_method = derive.to_ascii_lowercase();

        // A differently named method does not implement the selected aggregation operation.
        if method.sig.ident != expected_method {
            return false;
        }

        // Aggregation methods with other arities cannot forward one iterator unchanged.
        let [syn::FnArg::Typed(parameter)] =
            method.sig.inputs.iter().collect::<Vec<_>>().as_slice()
        else {
            return false;
        };

        // Only a simple input binding can be tracked through the iterator pipeline.
        let syn::Pat::Ident(parameter) = parameter.pat.as_ref() else {
            return false;
        };

        // Match a single-field construction that forwards the aggregation input unchanged.
        let [syn::Stmt::Expr(construction, _)] = method.block.stmts.as_slice() else {
            return false;
        };
        let aggregation = match construction {
            syn::Expr::Call(construction)
                if matches!(construction.func.as_ref(), syn::Expr::Path(path) if path.path.is_ident("Self") || path.path.is_ident(type_name))
                    && construction.args.len() == 1 => construction.args.first(),
            syn::Expr::Struct(construction)
                if (construction.path.is_ident("Self") || construction.path.is_ident(type_name))
                    && construction.fields.len() == 1
                    && construction.rest.is_none()
                    && construction.fields.first().is_some_and(|field| {
                        matches!((&field.member, field_name), (syn::Member::Named(actual), Some(expected)) if actual == expected)
                    }) => {
                construction.fields.first().map(|field| &field.expr)
            }
            _ => None,
        };

        // Require the aggregation call to select the expected iterator operation.
        let Some(syn::Expr::MethodCall(aggregation)) = aggregation else {
            return false;
        };

        // The outer call must be the argument-free selected aggregation method.
        if aggregation.method != expected_method || !aggregation.args.is_empty() {
            return false;
        }

        // The aggregation receiver must be the field-projection map call.
        let syn::Expr::MethodCall(map) = aggregation.receiver.as_ref() else {
            return false;
        };

        // Only a one-closure map over the original input proves transparent projection.
        if map.method != "map"
            || !matches!(map.receiver.as_ref(), syn::Expr::Path(path) if path.path.is_ident(&parameter.ident))
            || map.args.len() != 1
        {
            return false;
        }

        // Non-closure map arguments cannot prove direct field projection.
        let Some(syn::Expr::Closure(projection)) = map.args.first() else {
            return false;
        };

        // The projection must bind exactly one aggregate input value.
        let [syn::Pat::Ident(value)] = projection.inputs.iter().collect::<Vec<_>>().as_slice()
        else {
            return false;
        };

        matches!(
            projection.body.as_ref(),
            syn::Expr::Field(field)
                if matches!(field.base.as_ref(), syn::Expr::Path(path) if path.path.is_ident(&value.ident))
                    && (matches!(&field.member, syn::Member::Unnamed(index) if index.index == 0 && field_name.is_none())
                        || matches!((&field.member, field_name), (syn::Member::Named(actual), Some(expected)) if actual == expected))
        )
    }
}

impl LateLintPass<'_> for DeriveMoreManualAggregationImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Only implementation items can define an aggregation trait contract.
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };

        // Generated implementations do not represent authored aggregation boilerplate.
        if item.span.from_expansion() {
            return;
        }

        // Inherent implementations do not implement a derivable aggregation trait.
        let Some(trait_id) = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())
        else {
            return;
        };
        let derive_name = cx.tcx.item_name(trait_id);
        let derive = derive_name.as_str();

        // Only core aggregation traits have this derive_more replacement.
        if cx.tcx.crate_name(trait_id.krate).as_str() != "core"
            || !matches!(derive, "Sum" | "Product")
        {
            return;
        }

        // Require an aggregation implementation over the same single-field wrapper type.
        let trait_ref = cx
            .tcx
            .impl_trait_ref(item.owner_id.def_id)
            .instantiate_identity();
        let self_ty = trait_ref.self_ty();

        // Only nominal wrapper self types can derive aggregation.
        let ty::Adt(definition, _) = self_ty.kind() else {
            return;
        };

        // Aggregation over another item type is not same-wrapper forwarding.
        if trait_ref.args.type_at(1) != self_ty {
            return;
        }

        // Resolve the wrapper and its sole field for authored source comparison.
        let name = cx.tcx.item_name(definition.did()).to_string();

        // Only one-field structs match derive_more's transparent aggregation contract.
        if !definition.is_struct() || definition.non_enum_variant().fields.len() != 1 {
            return;
        }
        let field = definition
            .non_enum_variant()
            .fields
            .iter()
            .next()
            .expect("the single field was checked above");
        let field_name = (!field
            .name
            .as_str()
            .chars()
            .all(|character| character.is_ascii_digit()))
        .then(|| field.name.as_str());

        // Source syntax must exactly map the field, aggregate it, and reconstruct the wrapper.
        if !Self::is_exact_aggregation_source(
            cx,
            item,
            &AggregationNames {
                derive,
                type_name: &name,
            },
            field_name,
        ) {
            return;
        }

        Violation {
            owner: item.hir_id(),
            span: item.span,
            name,
            derive: derive.to_owned(),
        }
        .emit(cx);
    }
}
