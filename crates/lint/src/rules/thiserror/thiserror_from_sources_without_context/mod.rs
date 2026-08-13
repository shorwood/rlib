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

// -----------------------------------------------------------------------------
// Violation: Transparent conversion losing operation context
// -----------------------------------------------------------------------------

/// `#[from]` variant shared by several distinct propagation sites.
struct Violation {
    /// Error enum receiving the diagnostic.
    span: Span,
    /// Transparent conversion variant.
    variant: String,
    /// Distinct propagated operations named in the rationale.
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

// -----------------------------------------------------------------------------
// ContextCandidate: Error declarations and propagation uses
// -----------------------------------------------------------------------------

/// Transparent conversion variant retained for use-site correlation.
struct ContextCandidateError {
    /// Error enum receiving a later diagnostic.
    span: Span,
    /// Sole transparent `#[from]` variant.
    variant: String,
}

/// Function result and the propagated operations it contains.
struct ContextCandidateUse {
    /// Local error returned by the function.
    error: LocalDefId,
    /// Named calls immediately propagated with `?`.
    operations: BTreeSet<String>,
}

// -----------------------------------------------------------------------------
// TryOperationVisitor: Propagated call discovery
// -----------------------------------------------------------------------------

/// Collects names of calls whose results are immediately propagated.
#[derive(Default)]
struct TryOperationVisitor {
    /// Distinct operation names found beneath `?` expressions.
    operations: BTreeSet<String>,
}

impl TryOperationVisitor {
    /// Names direct function and method calls beneath a `?` expression.
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
}

impl<'ast> Visit<'ast> for TryOperationVisitor {
    fn visit_expr_try(&mut self, expression: &'ast syn::ExprTry) {
        if let Some(operation) = Self::operation_name(&expression.expr) {
            self.operations.insert(operation);
        }
        visit_expr_try(self, expression);
    }
}

// -----------------------------------------------------------------------------
// ThiserrorFromSourcesWithoutContext: Context-preserving conversion policy
// -----------------------------------------------------------------------------

/// Correlates transparent conversion variants with their propagation sites.
#[derive(Default)]
struct ThiserrorFromSourcesWithoutContext {
    /// Local derived error contracts.
    catalog: ThiserrorContractCatalog,
    /// Transparent variants indexed by their error enum.
    errors: HashMap<LocalDefId, ContextCandidateError>,
    /// Functions returning those errors and their propagated operations.
    uses: Vec<ContextCandidateUse>,
}

impl ThiserrorFromSourcesWithoutContext {
    /// Smallest propagation chain that demonstrates repeated context loss.
    const MINIMUM_PROPAGATION_OPERATIONS: usize = 2;
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
            if self.catalog.derived_type(usage.error).is_none()
                || usage.operations.len() < Self::MINIMUM_PROPAGATION_OPERATIONS
            {
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
    /// Records an enum with exactly one transparent `#[from]` variant.
    fn record_error(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
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
                (transparent && ThiserrorAttributes::from_attributes(&field.attrs).is_from)
                    .then(|| variant.ident.to_string())
            })
            .collect::<Vec<_>>();

        let [variant] = variants.as_slice() else {
            return;
        };

        self.errors.insert(
            item.owner_id.def_id,
            ContextCandidateError {
                span: item.span,
                variant: variant.clone(),
            },
        );
    }

    /// Records propagated operations in a function returning a local error.
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
        if !cx.tcx.is_diagnostic_item(sym::Result, result.did()) {
            return;
        }

        let Some(error) = arguments
            .type_at(1)
            .ty_adt_def()
            .and_then(|definition| definition.did().as_local())
        else {
            return;
        };

        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };
        let Ok(function) = syn::parse_str::<syn::ItemFn>(&source) else {
            return;
        };
        let mut visitor = TryOperationVisitor::default();

        visitor.visit_block(&function.block);
        self.uses.push(ContextCandidateUse {
            error,
            operations: visitor.operations,
        });
    }
}
