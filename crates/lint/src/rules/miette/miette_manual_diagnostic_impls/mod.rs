extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;
use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::{CtorOf, DefKind, Res};
use rustc_hir::{Body, Expr, ExprKind, ImplItemKind, Item, ItemKind, Mutability, PatKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty;
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::direct_forwarding::DirectForwarding;

// -----------------------------------------------------------------------------
// Violation: Derivable manual Diagnostic implementation
// -----------------------------------------------------------------------------

/// Manual Miette implementation expressible through derive attributes.
struct Violation {
    /// Declaration whose lint level governs the manual implementation finding.
    owner: rustc_hir::HirId,
    /// Complete implementation declaration.
    span: Span,
    /// Implementing type name.
    name: String,
    /// Derivable methods used to explain the finding.
    methods: Vec<String>,
}
impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "manual `Diagnostic` implementation for `{}` is derivable",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(if self.methods.is_empty() {
            "the implementation uses only `Diagnostic` defaults".to_owned()
        } else {
            format!(
                "methods {} contain only static metadata or direct field forwarding",
                self.methods
                    .iter()
                    .map(|method| format!("`{method}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "replace the implementation with `#[derive(miette::Diagnostic)]` and equivalent diagnostic or field attributes",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            MIETTE_MANUAL_DIAGNOSTIC_IMPLS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this complete diagnostic contract is derivable");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// MietteManualDiagnosticImpls: Declarative diagnostic implementation policy
// -----------------------------------------------------------------------------

/// Detects complete manual implementations that Miette can derive.
struct MietteManualDiagnosticImpls;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_MANUAL_DIAGNOSTIC_IMPLS,
    Warn,
    "finds Miette Diagnostic implementations reproducible by derive",
    MietteManualDiagnosticImpls
}

impl MietteManualDiagnosticImpls {
    /// Extracts the sole value wrapped by standard `Option::Some`.
    fn some_argument<'hir>(
        cx: &LateContext<'_>,
        expression: &'hir Expr<'hir>,
    ) -> Option<&'hir Expr<'hir>> {
        let ExprKind::Call(callee, [argument]) = expression.kind else {
            return None;
        };
        let ExprKind::Path(path) = callee.kind else {
            return None;
        };
        let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
            cx.qpath_res(&path, callee.hir_id)
        else {
            return None;
        };
        let variant = cx.tcx.parent(constructor);
        (cx.tcx.item_name(variant).as_str() == "Some"
            && cx
                .tcx
                .is_diagnostic_item(rustc_span::symbol::sym::Option, cx.tcx.parent(variant)))
        .then_some(argument)
    }

    /// Recognizes boxed static text used by code, help, and URL methods.
    fn static_box(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        let Some(expression) = Self::some_argument(cx, expression) else {
            return false;
        };
        let ExprKind::Call(callee, [value]) = expression.kind else {
            return false;
        };
        let ExprKind::Path(path) = callee.kind else {
            return false;
        };
        let Res::Def(DefKind::AssocFn, method) = cx.qpath_res(&path, callee.hir_id) else {
            return false;
        };
        let Some(implementation) = cx.tcx.impl_of_assoc(method) else {
            return false;
        };
        let Some(box_type) = cx
            .tcx
            .type_of(implementation)
            .instantiate_identity()
            .ty_adt_def()
        else {
            return false;
        };
        cx.tcx.item_name(method).as_str() == "new"
            && cx.tcx.crate_name(box_type.did().krate).as_str() == "alloc"
            && cx.tcx.item_name(box_type.did()).as_str() == "Box"
            && matches!(value.kind, ExprKind::Lit(literal) if matches!(literal.node, rustc_ast::LitKind::Str(..)))
    }

    /// Recognizes a static Miette severity variant wrapped by `Some`.
    fn static_severity(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        let Some(Expr {
            kind: ExprKind::Path(path),
            hir_id,
            ..
        }) = Self::some_argument(cx, expression)
        else {
            return false;
        };
        let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) = cx.qpath_res(path, *hir_id)
        else {
            return false;
        };
        let variant = cx.tcx.parent(constructor);
        let severity = cx.tcx.parent(variant);
        cx.tcx.crate_name(severity.krate).as_str() == "miette"
            && cx.tcx.item_name(severity).as_str() == "Severity"
            && matches!(
                cx.tcx.item_name(variant).as_str(),
                "Error" | "Warning" | "Advice"
            )
    }

    /// Recognizes a direct reference to one field on `self`.
    fn direct_reference(cx: &LateContext<'_>, body: &Body<'_>, expression: &Expr<'_>) -> bool {
        let Some(Expr {
            kind: ExprKind::AddrOf(_, Mutability::Not, field),
            ..
        }) = Self::some_argument(cx, expression)
        else {
            return false;
        };
        let ExprKind::Field(base, _) = field.kind else {
            return false;
        };
        let ExprKind::Path(path) = base.kind else {
            return false;
        };
        let Some(parameter) = body.params.first() else {
            return false;
        };
        let PatKind::Binding(_, receiver, _, None) = parameter.pat.kind else {
            return false;
        };
        matches!(cx.qpath_res(&path, base.hir_id), Res::Local(binding) if binding == receiver)
    }

    /// Returns all methods when the complete implementation is derive-equivalent.
    fn derivable_methods(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Vec<String>> {
        let ItemKind::Impl(implementation) = item.kind else {
            return None;
        };
        let mut names = Vec::new();
        for reference in implementation.items {
            let method = cx.tcx.hir_impl_item(*reference);
            let ImplItemKind::Fn(_, body_id) = method.kind else {
                return None;
            };
            let body = cx.tcx.hir_body(body_id);
            let expression = DirectForwarding::single_body_expression(body.value)?;
            let name = method.ident.name.to_string();

            let derivable = match name.as_str() {
                "code" | "help" | "url" => Self::static_box(cx, expression),
                "severity" => Self::static_severity(cx, expression),
                "source_code" | "diagnostic_source" => Self::direct_reference(cx, body, expression),
                _ => false,
            };

            if !derivable {
                return None;
            }
            names.push(name);
        }
        Some(names)
    }
}
impl LateLintPass<'_> for MietteManualDiagnosticImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };
        if item.span.from_expansion() {
            return;
        }

        let Some(trait_id) = implementation
            .of_trait
            .and_then(|reference| reference.trait_ref.trait_def_id())
        else {
            return;
        };

        if cx.tcx.crate_name(trait_id.krate).as_str() != "miette"
            || cx.tcx.item_name(trait_id).as_str() != "Diagnostic"
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

        if definition.did().as_local().is_none() {
            return;
        }

        let Some(methods) = Self::derivable_methods(cx, item) else {
            return;
        };

        Violation {
            owner: item.hir_id(),
            span: item.span,
            name: cx.tcx.item_name(definition.did()).to_string(),
            methods,
        }
        .emit(cx);
    }
}
