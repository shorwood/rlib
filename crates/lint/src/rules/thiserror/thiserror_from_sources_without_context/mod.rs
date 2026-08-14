extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Expr, ExprKind, Item, ItemKind, MatchSource, QPath};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::{Span, sym};

use super::utils::contracts::{ThiserrorAttributes, ThiserrorContractCatalog};
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
    /// Concrete source type converted by this variant.
    source: DefId,
}

/// Function result and the propagated operations it contains.
struct ContextCandidateUse {
    /// Local error returned by the function.
    error: LocalDefId,
    /// Named calls immediately propagated with `?`.
    operations: HashMap<DefId, BTreeSet<String>>,
}

// -----------------------------------------------------------------------------
// TryOperationVisitor: Propagated call discovery
// -----------------------------------------------------------------------------

/// Collects names of calls whose results are immediately propagated.
struct TryOperationVisitor<'cx, 'tcx> {
    /// Compiler context used to resolve operation signatures.
    cx: &'cx LateContext<'tcx>,
    /// Body owner whose type-checking results describe the visited expressions.
    owner: LocalDefId,
    /// Distinct operation names grouped by their concrete error type.
    operations: HashMap<DefId, BTreeSet<String>>,
}

impl TryOperationVisitor<'_, '_> {
    /// Resolves one authored call beneath a compiler-generated try desugaring.
    fn operation(&self, expression: &Expr<'_>) -> Option<(DefId, String)> {
        if expression.span.from_expansion() {
            return None;
        }
        let name = match expression.kind {
            ExprKind::Call(callee, _) => {
                let ExprKind::Path(path) = callee.kind else {
                    return None;
                };
                match path {
                    QPath::Resolved(_, path) => path.segments.last()?.ident.name.to_string(),
                    QPath::TypeRelative(_, segment) => segment.ident.name.to_string(),
                }
            }
            ExprKind::MethodCall(segment, ..) => segment.ident.name.to_string(),
            _ => return None,
        };
        let ty::Adt(result, arguments) = self.cx.tcx.typeck(self.owner).expr_ty(expression).kind()
        else {
            return None;
        };
        if !self.cx.tcx.is_diagnostic_item(sym::Result, result.did()) {
            return None;
        }
        let source = arguments.type_at(1).ty_adt_def()?.did();
        Some((source, name))
    }
}

impl<'tcx> Visitor<'tcx> for TryOperationVisitor<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Match(scrutinee, _, MatchSource::TryDesugar(_)) = expression.kind {
            let mut finder = AuthoredOperationFinder {
                visitor: self,
                operation: None,
            };
            finder.visit_expr(scrutinee);
            if let Some((source, name)) = finder.operation {
                self.operations.entry(source).or_default().insert(name);
            }
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_nested_body(&mut self, _: rustc_hir::BodyId) {}
}

/// Finds the authored call wrapped by one try desugaring.
struct AuthoredOperationFinder<'visitor, 'cx, 'tcx> {
    visitor: &'visitor TryOperationVisitor<'cx, 'tcx>,
    operation: Option<(DefId, String)>,
}

impl<'tcx> Visitor<'tcx> for AuthoredOperationFinder<'_, '_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if self.operation.is_some() {
            return;
        }
        if let Some(operation) = self.visitor.operation(expression) {
            self.operation = Some(operation);
            return;
        }
        intravisit::walk_expr(self, expression);
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
    errors: HashMap<LocalDefId, Vec<ContextCandidateError>>,
    /// Functions returning those errors and their propagated operations.
    uses: Vec<ContextCandidateUse>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_FROM_SOURCES_WITHOUT_CONTEXT,
    Warn,
    "finds transparent thiserror conversions shared by distinct operations",
    ThiserrorFromSourcesWithoutContext::default()
}

impl ThiserrorFromSourcesWithoutContext {
    /// Smallest propagation chain that demonstrates repeated context loss.
    const MINIMUM_PROPAGATION_OPERATIONS: usize = 2;
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
            let Some(errors) = self.errors.get(&usage.error) else {
                continue;
            };
            if self.catalog.derived_type(usage.error).is_none() {
                continue;
            }
            for error in errors {
                let Some(operations) = usage.operations.get(&error.source) else {
                    continue;
                };
                if operations.len() < Self::MINIMUM_PROPAGATION_OPERATIONS {
                    continue;
                }
                Violation {
                    span: error.span,
                    variant: error.variant.clone(),
                    operations: operations
                        .iter()
                        .map(|operation| format!("`{operation}`"))
                        .collect(),
                }
                .emit(cx);
            }
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

        let ItemKind::Enum(_, _, hir_definition) = item.kind else {
            return;
        };
        let variants = enumeration
            .variants
            .iter()
            .zip(hir_definition.variants)
            .filter_map(|(variant, hir_variant)| {
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
                if !transparent || !ThiserrorAttributes::from_attributes(&field.attrs).is_from {
                    return None;
                }
                let [hir_field] = hir_variant.data.fields() else {
                    return None;
                };
                let source = cx
                    .tcx
                    .type_of(hir_field.def_id)
                    .instantiate_identity()
                    .ty_adt_def()?
                    .did();
                Some(ContextCandidateError {
                    span: hir_variant.span,
                    variant: variant.ident.to_string(),
                    source,
                })
            })
            .collect::<Vec<_>>();
        if variants.is_empty() {
            return;
        }
        self.errors.insert(item.owner_id.def_id, variants);
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

        let ItemKind::Fn { body, .. } = item.kind else {
            return;
        };
        let mut visitor = TryOperationVisitor {
            cx,
            owner: item.owner_id.def_id,
            operations: HashMap::new(),
        };
        visitor.visit_expr(cx.tcx.hir_body(body).value);
        self.uses.push(ContextCandidateUse {
            error,
            operations: visitor.operations,
        });
    }
}
