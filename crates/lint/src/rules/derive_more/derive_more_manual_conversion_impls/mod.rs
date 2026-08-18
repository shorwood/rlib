extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::{ExprKind, ImplItem, ImplItemKind, ItemKind, Node, StructTailExpr};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

/// Describes a struct wrapper and the generic arguments applied to it.
struct OneFieldStruct<'tcx> {
    /// Definition of the wrapper struct.
    definition: ty::AdtDef<'tcx>,
    /// Generic arguments applied at the analyzed use site.
    arguments: ty::GenericArgsRef<'tcx>,
}

impl<'tcx> OneFieldStruct<'tcx> {
    /// Recognizes a struct wrapper with exactly one field.
    fn from_ty(ty: ty::Ty<'tcx>) -> Option<Self> {
        // Only algebraic data types can provide a transparent wrapper field.
        let ty::Adt(definition, arguments) = ty.kind() else {
            return None;
        };
        (definition.is_struct() && definition.non_enum_variant().fields.len() == 1).then_some(
            Self {
                definition: *definition,
                arguments,
            },
        )
    }
}

// -----------------------------------------------------------------------------
// Violation: Derivable conversion implementation
// -----------------------------------------------------------------------------

/// Transparent wrapping or extraction implementation reproducible by `derive_more`.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// `derive_more` macro capable of replacing the implementation.
    derive: &'static str,
    /// Wrapper type whose conversion is transparent.
    wrapper: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual transparent conversion for `{}` is derivable",
            self.wrapper
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the implementation only constructs or extracts the wrapper's sole field without validation, normalization, or adaptation",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "replace this implementation with `#[derive(derive_more::{})]` on `{}`",
            self.derive, self.wrapper
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_MANUAL_CONVERSION_IMPLS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this conversion is exact newtype plumbing");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// DeriveMoreManualConversionImpls: Declarative conversion policy
// -----------------------------------------------------------------------------

/// Finds transparent conversion implementations reproducible by `derive_more`.
struct DeriveMoreManualConversionImpls;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_CONVERSION_IMPLS,
    Warn,
    "finds transparent conversion implementations reproducible by derive_more",
    DeriveMoreManualConversionImpls
}

impl DeriveMoreManualConversionImpls {
    /// Returns whether a construction path names the wrapper definition.
    fn path_targets(cx: &LateContext<'_>, resolution: Res, definition: DefId) -> bool {
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

    /// Recognizes `From` implementations that only place an input in a newtype.
    fn exact_wrapping<'tcx>(
        cx: &LateContext<'tcx>,
        wrapper: ty::Ty<'tcx>,
        inner: ty::Ty<'tcx>,
        owner: LocalDefId,
        expression: &rustc_hir::Expr<'_>,
        binding: rustc_hir::HirId,
    ) -> Option<String> {
        let OneFieldStruct {
            definition,
            arguments,
        } = OneFieldStruct::from_ty(wrapper)?;
        let field = definition.non_enum_variant().fields.iter().next()?;

        // A field with a different type performs more than transparent wrapping.
        if field.ty(cx.tcx, arguments) != inner {
            return None;
        }

        let is_exact = match expression.kind {
            ExprKind::Call(_, _) => {
                DirectForwarding::call(cx, owner, expression).is_some_and(|call| {
                    // Exact construction forwards one and only one input argument.
                    let [argument] = call.arguments.as_slice() else {
                        return false;
                    };
                    (call.target == definition.did()
                        || cx.tcx.opt_parent(call.target) == Some(definition.did()))
                        && DirectForwarding::is_binding(cx, argument, binding)
                })
            }
            ExprKind::Struct(path, fields, StructTailExpr::None) => {
                // Transparent struct syntax initializes only the wrapper's sole field.
                let [constructed] = fields else {
                    return None;
                };
                Self::path_targets(cx, cx.qpath_res(path, expression.hir_id), definition.did())
                    && constructed.ident.name == field.name
                    && DirectForwarding::is_binding(cx, constructed.expr, binding)
            }
            _ => false,
        };
        is_exact.then(|| cx.tcx.item_name(definition.did()).to_string())
    }

    /// Recognizes `Into` implementations that only return a newtype's field.
    fn exact_extraction<'tcx>(
        cx: &LateContext<'tcx>,
        wrapper: ty::Ty<'tcx>,
        inner: ty::Ty<'tcx>,
        expression: &rustc_hir::Expr<'_>,
        binding: rustc_hir::HirId,
    ) -> Option<String> {
        let OneFieldStruct {
            definition,
            arguments,
        } = OneFieldStruct::from_ty(wrapper)?;

        // Exact extraction must directly access the wrapper field.
        let ExprKind::Field(base, field) = expression.kind else {
            return None;
        };
        let sole_field = definition.non_enum_variant().fields.iter().next()?;

        (field.name == sole_field.name
            && DirectForwarding::is_binding(cx, base, binding)
            && sole_field.ty(cx.tcx, arguments) == inner)
            .then(|| cx.tcx.item_name(definition.did()).to_string())
    }
}

impl LateLintPass<'_> for DeriveMoreManualConversionImpls {
    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Conversion candidates must be function implementation items.
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return;
        };

        // Generated methods and receiver methods are outside static conversion policy.
        if item.span.from_expansion() || signature.decl.implicit_self.has_implicit_self() {
            return;
        }
        let implementation = cx.tcx.local_parent(item.owner_id.def_id);

        // The method must belong to a source-level implementation item.
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
            return;
        };

        // Nonimplementation parents cannot define a conversion contract.
        let ItemKind::Impl(implementation_item) = parent.kind else {
            return;
        };

        // Attributes or extra predicates may encode behavior a derive would not preserve.
        if !cx.tcx.hir_attrs(parent.hir_id()).is_empty()
            || !cx.tcx.hir_attrs(item.hir_id()).is_empty()
            || !implementation_item.generics.predicates.is_empty()
        {
            return;
        }

        // Inherent implementations do not implement the standard conversion trait.
        let Some(trait_ref) = implementation_item
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())
        else {
            return;
        };

        // Exclude methods outside the canonical core conversion implementation.
        if cx.tcx.crate_name(trait_ref.krate).as_str() != "core"
            || cx.tcx.item_name(trait_ref).as_str() != "From"
            || item.ident.name.as_str() != "from"
        {
            return;
        }
        let trait_ref = cx.tcx.impl_trait_ref(implementation).instantiate_identity();

        let source = trait_ref.args.type_at(1);
        let target = trait_ref.self_ty();
        let body = cx.tcx.hir_body(body_id);

        // Bodies with additional behavior are not exact forwarding conversions.
        let Some(forwarding) =
            DirectForwarding::expression(cx, item.owner_id.def_id, signature.header, body)
        else {
            return;
        };

        // Exact conversions forward a single source binding.
        let [binding] = forwarding.bindings.as_slice() else {
            return;
        };

        if let Some(wrapper) = Self::exact_wrapping(
            cx,
            target,
            source,
            forwarding.typeck_owner,
            forwarding.forwarded,
            *binding,
        ) {
            Violation {
                span: parent.span,
                derive: "From",
                wrapper,
            }
            .emit(cx);
        } else if let Some(wrapper) =
            Self::exact_extraction(cx, source, target, forwarding.forwarded, *binding)
        {
            Violation {
                span: parent.span,
                derive: "Into",
                wrapper,
            }
            .emit(cx);
        }
    }
}
