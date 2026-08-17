extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;
use std::borrow::Cow;
use std::collections::HashMap;
use std::mem;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, ImplItemKind, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, sym};

use super::utils::contracts::DiagnosticCatalog;
use crate::utils::diagnostic::LateViolation;

/// Type arguments required by the standard result type.
const RESULT_TYPE_ARGUMENTS: usize = 2;

// -----------------------------------------------------------------------------
// Violation: Advisory diagnostic propagated as an error
// -----------------------------------------------------------------------------

/// Public result boundary that returns a diagnostic through its error channel.
struct ViolationUse {
    /// Function declaration exposing the result.
    span: Span,
    /// Function name shown to the author.
    function: String,
}

/// Advisory diagnostic whose declared severity contradicts its propagation.
struct Violation {
    /// Diagnostic type declaration.
    span: Span,
    /// Diagnostic type name.
    diagnostic: String,
    /// Authored advisory severity.
    severity: String,
    /// Functions returning the type as an error.
    uses: Vec<ViolationUse>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "diagnostic `{}` declares `{}` severity but is returned as an error",
            self.diagnostic, self.severity
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the type is the error channel of {}",
            self.uses
                .iter()
                .map(|usage| format!("`{}`", usage.function))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use error severity, or return this diagnostic through an advisory collection instead of `Result::Err`",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MIETTE_INCOHERENT_DIAGNOSTIC_SEVERITY,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this severity contradicts propagation");
                for usage in &self.uses {
                    diag.span_label(
                        usage.span,
                        format!("`{}` returns it as an error", usage.function),
                    );
                }
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// MietteIncoherentDiagnosticSeverity: Propagation and severity agreement
// -----------------------------------------------------------------------------

/// Correlates derived severity metadata with function result types.
#[derive(Default)]
struct MietteIncoherentDiagnosticSeverity {
    /// Derived diagnostic declarations in the crate.
    catalog: DiagnosticCatalog,
    /// Error-channel uses grouped by returned local type.
    uses: HashMap<LocalDefId, Vec<ViolationUse>>,
}

impl MietteIncoherentDiagnosticSeverity {
    /// Resolves a local diagnostic through transparent standard pointer wrappers.
    fn local_error_type(cx: &LateContext<'_>, ty: ty::Ty<'_>) -> Option<LocalDefId> {
        // Non-aggregate types cannot name a local diagnostic or transparent pointer wrapper.
        let ty::Adt(definition, arguments) = ty.kind() else {
            return None;
        };

        // A directly local aggregate is the diagnostic identity sought by the caller.
        if let Some(local) = definition.did().as_local() {
            return Some(local);
        }
        let is_pointer = cx.tcx.crate_name(definition.did().krate).as_str() == "alloc"
            && matches!(
                cx.tcx.item_name(definition.did()).as_str(),
                "Box" | "Rc" | "Arc"
            );
        (is_pointer && !arguments.is_empty())
            .then(|| Self::local_error_type(cx, arguments.type_at(0)))
            .flatten()
    }

    /// Records one function-like boundary whose `Result` error resolves locally.
    fn record_boundary(
        &mut self,
        cx: &LateContext<'_>,
        definition: LocalDefId,
        span: Span,
        function: String,
    ) {
        let output = cx
            .tcx
            .fn_sig(definition)
            .instantiate_identity()
            .skip_binder()
            .output();

        // Non-aggregate return types cannot be the standard `Result` boundary.
        let ty::Adt(result, arguments) = output.kind() else {
            return;
        };

        // Only a well-formed standard `Result` exposes a semantic error type argument.
        if !cx.tcx.is_diagnostic_item(sym::Result, result.did())
            || arguments.len() != RESULT_TYPE_ARGUMENTS
        {
            return;
        }

        // External or opaque error types cannot correlate with a local diagnostic declaration.
        let Some(error) = Self::local_error_type(cx, arguments.type_at(1)) else {
            return;
        };
        self.uses
            .entry(error)
            .or_default()
            .push(ViolationUse { span, function });
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_INCOHERENT_DIAGNOSTIC_SEVERITY,
    Warn,
    "finds advisory Miette diagnostics propagated as errors",
    MietteIncoherentDiagnosticSeverity::default()
}

impl LateLintPass<'_> for MietteIncoherentDiagnosticSeverity {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Record the diagnostic declaration before inspecting function signatures.
        self.catalog.check_item(cx, item);

        // Generated and non-function items do not define authored result boundaries.
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Fn { .. }) {
            return;
        }

        self.record_boundary(
            cx,
            item.owner_id.def_id,
            item.span,
            cx.tcx.item_name(item.owner_id.def_id).to_string(),
        );
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Generated and non-function associated items do not define authored result boundaries.
        if item.span.from_expansion() || !matches!(item.kind, ImplItemKind::Fn(..)) {
            return;
        }
        self.record_boundary(
            cx,
            item.owner_id.def_id,
            item.span,
            cx.tcx.def_path_str(item.owner_id.def_id),
        );
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        let mut violations = Vec::new();
        for (definition, mut uses) in mem::take(&mut self.uses) {
            let Some(contract) = self.catalog.derived_type(definition) else {
                continue;
            };
            let Some(severity) = contract.metadata.severity.as_deref() else {
                continue;
            };
            if !matches!(severity, "Warning" | "Advice") {
                continue;
            }
            uses.sort_by_key(|usage| usage.span.lo());

            violations.push(Violation {
                span: contract.span,
                diagnostic: contract.name.clone(),
                severity: severity.to_owned(),
                uses,
            });
        }
        violations.sort_by_key(|violation| violation.span.lo());
        for violation in violations {
            violation.emit(cx);
        }
    }
}
