extern crate rustc_abi;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_abi::ExternAbi;
use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::{
    Constness, ExprKind, ImplItem, ImplItemKind, ItemKind, Node, PatKind, StructTailExpr,
};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

/// Connects a constructor parameter binding to its authored name.
struct ParameterBinding {
    /// HIR binding used to recognize forwarded parameter expressions.
    binding: rustc_hir::HirId,
    /// Authored parameter name expected to match the destination field.
    name: Symbol,
}

// -----------------------------------------------------------------------------
// Violation: Derivable manual constructor
// -----------------------------------------------------------------------------

/// Policy-free field constructor reproducible by `derive_more`.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Type name quoted in the diagnostic.
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

// -----------------------------------------------------------------------------
// DeriveMoreManualConstructors: Declarative construction policy
// -----------------------------------------------------------------------------

/// Finds policy-free constructors reproducible by `derive_more`'s `Constructor` derive.
struct DeriveMoreManualConstructors;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_CONSTRUCTORS,
    Warn,
    "finds structural constructors reproducible by derive_more",
    DeriveMoreManualConstructors
}

impl DeriveMoreManualConstructors {
    /// Proves that a struct expression forwards every parameter into its matching field.
    fn is_exact_struct_assembly(
        cx: &LateContext<'_>,
        definition: DefId,
        bindings: &[ParameterBinding],
        path: &rustc_hir::QPath<'_>,
        fields: &[rustc_hir::ExprField<'_>],
        expression: &rustc_hir::Expr<'_>,
    ) -> bool {
        Self::is_path_targeting(cx, cx.qpath_res(path, expression.hir_id), definition)
            && fields.len() == bindings.len()
            && cx
                .tcx
                .adt_def(definition)
                .non_enum_variant()
                .fields
                .iter()
                .zip(bindings)
                .all(|(field, parameter)| field.name == parameter.name)
            && fields.iter().all(|field| {
                bindings.iter().any(|parameter| {
                    field.ident.name == parameter.name
                        && DirectForwarding::is_binding(cx, field.expr, parameter.binding)
                })
            })
    }

    /// Collects plain constructor parameter names in declaration order.
    fn parameter_bindings(body: &rustc_hir::Body<'_>) -> Option<Vec<ParameterBinding>> {
        body.params
            .iter()
            .map(|parameter| {
                // Destructured parameters cannot map one-to-one onto named fields.
                let PatKind::Binding(_, binding, ident, None) = parameter.pat.kind else {
                    return None;
                };
                Some(ParameterBinding {
                    binding,
                    name: ident.name,
                })
            })
            .collect()
    }

    /// Returns whether a construction path names the enclosing type.
    fn is_path_targeting(cx: &LateContext<'_>, resolution: Res, definition: DefId) -> bool {
        match resolution {
            Res::Def(_, target) => target == definition,
            Res::SelfCtor(implementation) => cx
                .tcx
                .type_of(implementation)
                .instantiate_identity()
                .ty_adt_def()
                .is_some_and(|target| target.did() == definition),
            Res::SelfTyAlias { alias_to, .. } => cx
                .tcx
                .type_of(alias_to)
                .instantiate_identity()
                .ty_adt_def()
                .is_some_and(|target| target.did() == definition),
            _ => false,
        }
    }

    /// Proves that a constructor assigns every parameter directly to its matching field.
    fn is_exact_field_assembly(
        cx: &LateContext<'_>,
        definition: DefId,
        owner: LocalDefId,
        bindings: &[ParameterBinding],
        expression: &rustc_hir::Expr<'_>,
    ) -> bool {
        match expression.kind {
            ExprKind::Struct(path, fields, StructTailExpr::None) => {
                Self::is_exact_struct_assembly(cx, definition, bindings, path, fields, expression)
            }
            ExprKind::Call(_, arguments) => DirectForwarding::call(cx, owner, expression)
                .is_some_and(|call| {
                    call.target == definition
                        && arguments.len() == bindings.len()
                        && arguments.iter().zip(bindings).all(|(argument, parameter)| {
                            DirectForwarding::is_binding(cx, argument, parameter.binding)
                        })
                }),
            ExprKind::Path(path) => {
                bindings.is_empty()
                    && Self::is_path_targeting(
                        cx,
                        cx.qpath_res(&path, expression.hir_id),
                        definition,
                    )
            }
            _ => false,
        }
    }

    /// Proves that the generated constructor preserves the authored callable type.
    fn is_signature_matching(
        cx: &LateContext<'_>,
        implementation: LocalDefId,
        method: LocalDefId,
        definition: DefId,
    ) -> bool {
        let self_ty = cx.tcx.type_of(implementation).instantiate_identity();

        // Only algebraic self types can be matched to a struct definition.
        let ty::Adt(adt, arguments) = self_ty.kind() else {
            return false;
        };

        // The implementation must construct the exact enclosing definition.
        if adt.did() != definition {
            return false;
        }

        let signature = cx.tcx.fn_sig(method).instantiate_identity().skip_binder();
        signature.output() == self_ty
            && signature.inputs().len() == adt.non_enum_variant().fields.len()
            && signature
                .inputs()
                .iter()
                .zip(&adt.non_enum_variant().fields)
                .all(|(input, field)| *input == field.ty(cx.tcx, arguments))
    }
}

impl LateLintPass<'_> for DeriveMoreManualConstructors {
    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Constructor candidates must be function implementation items.
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return;
        };

        // Preserve only the public, safe, constant constructor contract produced by the derive.
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

        // The method must belong to a source-level implementation item.
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
            return;
        };

        // Nonimplementation parents cannot define an inherent constructor.
        let ItemKind::Impl(implementation_item) = parent.kind else {
            return;
        };

        // Trait methods are not replaceable by an inherent constructor derive.
        if implementation_item.of_trait.is_some() {
            return;
        }

        // The enclosing self type must resolve to a concrete algebraic definition.
        let Some(definition) = cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .ty_adt_def()
        else {
            return;
        };

        // Constructor derivation applies to structs rather than enum or union assembly.
        if !definition.is_struct() {
            return;
        }

        // A differing callable signature carries policy the derive would not preserve.
        if !Self::is_signature_matching(cx, implementation, item.owner_id.def_id, definition.did())
        {
            return;
        }
        let body = cx.tcx.hir_body(body_id);

        // Destructured or otherwise complex parameters are not direct field inputs.
        let Some(bindings) = Self::parameter_bindings(body) else {
            return;
        };

        // Additional body work makes the constructor more than structural assembly.
        let Some(expression) = DirectForwarding::single_body_expression(body.value) else {
            return;
        };

        // Emit only when every input is assigned unchanged to its matching field.
        if !Self::is_exact_field_assembly(
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
