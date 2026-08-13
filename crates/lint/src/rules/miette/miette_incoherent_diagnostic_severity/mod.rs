extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;
use std::borrow::Cow;
use std::collections::HashMap;
use std::mem;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, sym};

use super::contracts::DiagnosticCatalog;
use crate::utils::diagnostic::LateViolation;

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
dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_INCOHERENT_DIAGNOSTIC_SEVERITY,
    Warn,
    "finds advisory Miette diagnostics propagated as errors",
    MietteIncoherentDiagnosticSeverity::default()
}
impl LateLintPass<'_> for MietteIncoherentDiagnosticSeverity {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        /// Success and error arguments carried by `Result`.
        // Fix the expected shape before inspecting a function's result type.
        const RESULT_TYPE_ARGUMENT_COUNT: usize = 2;

        // Record the diagnostic declaration before inspecting function signatures.
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Fn { .. }) {
            return;
        }

        let output = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .output();

        let ty::Adt(result, arguments) = output.kind() else {
            return;
        };
        if !cx.tcx.is_diagnostic_item(sym::Result, result.did())
            || arguments.len() != RESULT_TYPE_ARGUMENT_COUNT
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

        self.uses.entry(error).or_default().push(ViolationUse {
            span: item.span,
            function: cx.tcx.item_name(item.owner_id.def_id).to_string(),
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for (definition, uses) in mem::take(&mut self.uses) {
            let Some(contract) = self.catalog.derived_type(definition) else {
                continue;
            };
            if !contract.members.is_empty() {
                continue;
            }

            let Some(severity) = contract.metadata.severity.as_deref() else {
                continue;
            };
            if !matches!(severity, "Warning" | "Advice") {
                continue;
            }

            Violation {
                span: contract.span,
                diagnostic: contract.name.clone(),
                severity: severity.to_owned(),
                uses,
            }
            .emit(cx);
        }
    }
}
