extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;
use syn::visit::Visit;

use super::contracts::{ThiserrorContractCatalog, thiserror_attributes};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct ErrorCandidate {
    span: Span,
    variant: String,
}

struct UseCandidate {
    error: LocalDefId,
    operations: BTreeSet<String>,
}

struct Violation {
    span: Span,
    variant: String,
    operations: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "thiserror `#[from]` variant `{}` erases operation context",
            self.variant
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "automatic conversion receives `?` failures from distinct operations {} without recording which one failed",
            self.operations.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("use contextual variants or explicit `map_err` at each operation boundary")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            THISERROR_FROM_SOURCES_WITHOUT_CONTEXT,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this transparent conversion merges distinct operations",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct ThiserrorFromSourcesWithoutContext {
    catalog: ThiserrorContractCatalog,
    errors: HashMap<LocalDefId, ErrorCandidate>,
    uses: Vec<UseCandidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_FROM_SOURCES_WITHOUT_CONTEXT,
    Warn,
    "finds transparent thiserror conversions shared by distinct operations",
    ThiserrorFromSourcesWithoutContext::default()
}

impl LateLintPass<'_> for ThiserrorFromSourcesWithoutContext {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }
        match item.kind {
            ItemKind::Enum(..) => self.record_error(cx, item),
            ItemKind::Fn { .. } => self.record_uses(cx, item),
            _ => {}
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for usage in self.uses.drain(..) {
            let Some(error) = self.errors.get(&usage.error) else {
                continue;
            };
            if self.catalog.derived_type(usage.error).is_none() || usage.operations.len() < 2 {
                continue;
            }
            Violation {
                span: error.span,
                variant: error.variant.clone(),
                operations: usage
                    .operations
                    .into_iter()
                    .map(|operation| format!("`{operation}`"))
                    .collect(),
            }
            .emit(cx);
        }
    }
}

impl ThiserrorFromSourcesWithoutContext {
    fn record_error(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        let variants = enumeration
            .variants
            .iter()
            .filter_map(|variant| {
                let transparent = variant.attrs.iter().any(|attribute| {
                    attribute.path().is_ident("error")
                        && attribute
                            .parse_args::<syn::Path>()
                            .is_ok_and(|path| path.is_ident("transparent"))
                });
                let fields = variant.fields.iter().collect::<Vec<_>>();
                let [field] = fields.as_slice() else {
                    return None;
                };
                (transparent && thiserror_attributes(&field.attrs).from)
                    .then(|| variant.ident.to_string())
            })
            .collect::<Vec<_>>();
        let [variant] = variants.as_slice() else {
            return;
        };
        self.errors.insert(
            item.owner_id.def_id,
            ErrorCandidate {
                span: item.span,
                variant: variant.clone(),
            },
        );
    }

    fn record_uses(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        let output = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .output();
        let ty::Adt(result, arguments) = output.kind() else {
            return;
        };
        if !cx
            .tcx
            .is_diagnostic_item(rustc_span::sym::Result, result.did())
        {
            return;
        }
        let Some(error) = arguments
            .type_at(1)
            .ty_adt_def()
            .and_then(|definition| definition.did().as_local())
        else {
            return;
        };
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let Ok(function) = syn::parse_str::<syn::ItemFn>(&source) else {
            return;
        };
        let mut visitor = TryOperationVisitor::default();
        visitor.visit_block(&function.block);
        self.uses.push(UseCandidate {
            error,
            operations: visitor.operations,
        });
    }
}

#[derive(Default)]
struct TryOperationVisitor {
    operations: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for TryOperationVisitor {
    fn visit_expr_try(&mut self, expression: &'ast syn::ExprTry) {
        if let Some(operation) = operation_name(&expression.expr) {
            self.operations.insert(operation);
        }
        syn::visit::visit_expr_try(self, expression);
    }
}

fn operation_name(expression: &syn::Expr) -> Option<String> {
    match expression {
        syn::Expr::Call(call) => match call.func.as_ref() {
            syn::Expr::Path(path) => Some(path.path.segments.last()?.ident.to_string()),
            _ => None,
        },
        syn::Expr::MethodCall(call) => Some(call.method.to_string()),
        _ => None,
    }
}
