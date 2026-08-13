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
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, sym};
use syn::visit::{Visit, visit_expr_try};

use super::contracts::{ThiserrorAttributes, ThiserrorContractCatalog};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Carries the `ErrorCandidate` state used by this analysis.
struct ErrorCandidate {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `variant` value used by this analysis.
    variant: String,
}

/// Carries the `UseCandidate` state used by this analysis.
struct UseCandidate {
    /// Stores the `error` value used by this analysis.
    error: LocalDefId,
    /// Stores the `operations` value used by this analysis.
    operations: BTreeSet<String>,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `variant` value used by this analysis.
    variant: String,
    /// Stores the `operations` value used by this analysis.
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

/// Performs the `operation_name` step of the lint analysis.
fn operation_name(expression: &syn::Expr) -> Option<String> {
    // Classify the current analyze_candidate.
    match expression {
        syn::Expr::Call(call) => match call.func.as_ref() {
            syn::Expr::Path(path) => Some(path.path.segments.last()?.ident.to_string()),
            _ => None,
        },
        syn::Expr::MethodCall(call) => Some(call.method.to_string()),
        _ => None,
    }
}

#[derive(Default)]
/// Carries the `TryOperationVisitor` state used by this analysis.
struct TryOperationVisitor {
    /// Stores the `operations` value used by this analysis.
    operations: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for TryOperationVisitor {
    fn visit_expr_try(&mut self, expression: &'ast syn::ExprTry) {
        if let Some(operation) = operation_name(&expression.expr) {
            self.operations.insert(operation);
        }
        visit_expr_try(self, expression);
    }
}

#[derive(Default)]
/// Carries the `ThiserrorFromSourcesWithoutContext` state used by this analysis.
struct ThiserrorFromSourcesWithoutContext {
    /// Stores the `catalog` value used by this analysis.
    catalog: ThiserrorContractCatalog,
    /// Stores the `errors` value used by this analysis.
    errors: HashMap<LocalDefId, ErrorCandidate>,
    /// Stores the `uses` value used by this analysis.
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
        /// Smallest propagation chain that demonstrates repeated context loss.
        const MINIMUM_PROPAGATION_OPERATIONS: usize = 2;

        for usage in self.uses.drain(..) {
            // Prepare the values used by this stage.
            let Some(error) = self.errors.get(&usage.error) else {
                continue;
            };
            if self.catalog.derived_type(usage.error).is_none()
                || usage.operations.len() < MINIMUM_PROPAGATION_OPERATIONS
            {
                continue;
            }

            // Perform the next step of the analysis.
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
    /// Performs the `record_error` operation for this value.
    fn record_error(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Prepare the values used by this stage.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };

        // Prepare the values used by this stage.
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
                (transparent && ThiserrorAttributes::from_attributes(&field.attrs).is_from)
                    .then(|| variant.ident.to_string())
            })
            .collect::<Vec<_>>();

        // Prepare the values used by this stage.
        let [variant] = variants.as_slice() else {
            return;
        };

        // Update the accumulated analysis state.
        self.errors.insert(
            item.owner_id.def_id,
            ErrorCandidate {
                span: item.span,
                variant: variant.clone(),
            },
        );
    }

    /// Performs the `record_uses` operation for this value.
    fn record_uses(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Prepare the values used by this stage.
        let output = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .output();

        // Prepare the values used by this stage.
        let ty::Adt(result, arguments) = output.kind() else {
            return;
        };
        if !cx.tcx.is_diagnostic_item(sym::Result, result.did()) {
            return;
        }

        // Prepare the values used by this stage.
        let Some(error) = arguments
            .type_at(1)
            .ty_adt_def()
            .and_then(|definition| definition.did().as_local())
        else {
            return;
        };

        // Prepare the values used by this stage.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };
        let Ok(function) = syn::parse_str::<syn::ItemFn>(&source) else {
            return;
        };
        let mut visitor = TryOperationVisitor::default();

        // Perform the next step of the analysis.
        visitor.visit_block(&function.block);
        self.uses.push(UseCandidate {
            error,
            operations: visitor.operations,
        });
    }
}
