extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;
use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_session::config::CrateType;
use rustc_span::{Span, sym};

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Application report exposed by a library API
// -----------------------------------------------------------------------------

/// Public library function returning an erased Miette report.
struct Violation {
    /// Public declaration whose lint level governs the report-boundary finding.
    owner: rustc_hir::HirId,
    /// Public function declaration.
    span: Span,
    /// Function name shown to the author.
    function: String,
}
impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "public library API `{}` returns `miette::Report`",
            self.function
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "a report is an application rendering boundary and erases the concrete failure vocabulary library callers need",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "return a public concrete error type, optionally deriving both `thiserror::Error` and `miette::Diagnostic`",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            MIETTE_REPORTS_IN_LIBRARY_INTERFACES,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this library boundary exposes an application report",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}
// -----------------------------------------------------------------------------
// MietteReportsInLibraryInterfaces: Concrete library error vocabulary
// -----------------------------------------------------------------------------

/// Rejects application-oriented reports at public library boundaries.
struct MietteReportsInLibraryInterfaces;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_REPORTS_IN_LIBRARY_INTERFACES,
    Warn,
    "finds Miette reports in public library APIs",
    MietteReportsInLibraryInterfaces
}

impl MietteReportsInLibraryInterfaces {
    /// Returns whether the current crate produces any library artifact.
    fn library_crate(cx: &LateContext<'_>) -> bool {
        cx.sess()
            .opts
            .crate_types
            .iter()
            .any(|kind| !matches!(kind, CrateType::Executable))
    }

    /// Finds `miette::Report` directly or in a result error position.
    fn contains_report(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
        let ty::Adt(definition, arguments) = ty.kind() else {
            return false;
        };
        if cx.tcx.crate_name(definition.did().krate).as_str() == "miette"
            && cx.tcx.item_name(definition.did()).as_str() == "Report"
        {
            return true;
        }
        cx.tcx.is_diagnostic_item(sym::Result, definition.did())
            && arguments.len() == 2
            && Self::contains_report(cx, arguments.type_at(1))
    }
}
impl LateLintPass<'_> for MietteReportsInLibraryInterfaces {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Executables may choose their final rendering boundary freely.
        if item.span.from_expansion()
            || !Self::library_crate(cx)
            || !matches!(item.kind, ItemKind::Fn { .. })
            || !cx.tcx.visibility(item.owner_id.def_id).is_public()
        {
            return;
        }

        let output = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .output();

        if !Self::contains_report(cx, output) {
            return;
        }

        Violation {
            owner: item.hir_id(),
            span: item.span,
            function: cx.tcx.item_name(item.owner_id.def_id).to_string(),
        }
        .emit(cx);
    }
}
