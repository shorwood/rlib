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

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: rustc_hir::HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `name` value used by this analysis.
    name: String,
    /// Stores the `methods` value used by this analysis.
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

/// Performs the `some_argument` step of the lint analysis.
fn some_argument(expression: &syn::Expr) -> Option<&syn::Expr> {
    let syn::Expr::Call(call) = expression else {
        return None;
    };
    if !matches!(call.func.as_ref(), syn::Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "Some"))
        || call.args.len() != 1
    {
        return None;
    }
    call.args.first()
}

/// Performs the `static_box` step of the lint analysis.
fn static_box(expression: &syn::Expr) -> bool {
    let Some(syn::Expr::Call(boxed)) = some_argument(expression) else {
        return false;
    };
    matches!(boxed.func.as_ref(), syn::Expr::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "new"))
        && matches!(boxed.args.first(), Some(syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(_), .. })) if boxed.args.len() == 1)
}

/// Performs the `static_severity` step of the lint analysis.
fn static_severity(expression: &syn::Expr) -> bool {
    matches!(some_argument(expression), Some(syn::Expr::Path(path)) if path.path.segments.last().is_some_and(|segment| matches!(segment.ident.to_string().as_str(), "Error" | "Warning" | "Advice")))
}

/// Performs the `direct_reference` step of the lint analysis.
fn direct_reference(expression: &syn::Expr) -> bool {
    let Some(syn::Expr::Reference(reference)) = some_argument(expression) else {
        return false;
    };
    matches!(reference.expr.as_ref(), syn::Expr::Field(field) if matches!(field.base.as_ref(), syn::Expr::Path(path) if path.path.is_ident("self")))
}

/// Performs the `derivable_methods` step of the lint analysis.
fn derivable_methods(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Vec<String>> {
    let source = AuthoredItemSource::for_item(cx, item)?;
    let implementation = match syn::parse_str::<syn::ItemImpl>(&source) {
        Ok(implementation) => implementation,
        Err(_error) => return None,
    };
    let mut names = Vec::new();
    for member in implementation.items {
        // Prepare the values used by this stage.
        let syn::ImplItem::Fn(method) = member else {
            return None;
        };
        let name = method.sig.ident.to_string();
        let [syn::Stmt::Expr(expression, _)] = method.block.stmts.as_slice() else {
            return None;
        };

        // Prepare the values used by this stage.
        let derivable = match name.as_str() {
            "code" | "help" | "url" => static_box(expression),
            "severity" => static_severity(expression),
            "source_code" | "diagnostic_source" => direct_reference(expression),
            _ => false,
        };

        // Reject inputs that do not satisfy this stage.
        if !derivable {
            return None;
        }
        names.push(name);
    }
    Some(names)
}

/// Carries the `MietteManualDiagnosticImpls` state used by this analysis.
struct MietteManualDiagnosticImpls;
dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_MANUAL_DIAGNOSTIC_IMPLS,
    Warn,
    "finds Miette Diagnostic implementations reproducible by derive",
    MietteManualDiagnosticImpls
}
impl LateLintPass<'_> for MietteManualDiagnosticImpls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Prepare the values used by this stage.
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };
        if item.span.from_expansion() || !cx.tcx.hir_attrs(item.hir_id()).is_empty() {
            return;
        }

        // Prepare the values used by this stage.
        let Some(trait_id) = implementation
            .of_trait
            .and_then(|reference| reference.trait_ref.trait_def_id())
        else {
            return;
        };

        // Reject inputs that do not satisfy this stage.
        if cx.tcx.crate_name(trait_id.krate).as_str() != "miette"
            || cx.tcx.item_name(trait_id).as_str() != "Diagnostic"
        {
            return;
        }

        // Prepare the values used by this stage.
        let trait_ref = cx
            .tcx
            .impl_trait_ref(item.owner_id.def_id)
            .instantiate_identity();
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return;
        };

        // Reject inputs that do not satisfy this stage.
        if definition.did().as_local().is_none()
            || !cx.tcx.generics_of(definition.did()).own_params.is_empty()
        {
            return;
        }

        // Prepare the values used by this stage.
        let Some(methods) = derivable_methods(cx, item) else {
            return;
        };

        // Perform the next step of the analysis.
        Violation {
            owner: item.hir_id(),
            span: item.span,
            name: cx.tcx.item_name(definition.did()).to_string(),
            methods,
        }
        .emit(cx);
    }
}
