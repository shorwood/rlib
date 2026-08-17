extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;
use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{
    AmbigArg, FnRetTy, ImplItem, ImplItemKind, Item, ItemKind, TraitItem, TraitItemKind,
    Ty as HirTy, TyKind,
};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_session::config::CrateType;
use rustc_span::def_id::LocalDefId;
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

/// Resolves the explicitly authored return type hidden by async lowering.
struct AuthoredReportTypeVisitor<'analysis, 'tcx> {
    /// Compiler context used to resolve type paths and aliases.
    cx: &'analysis LateContext<'tcx>,
    /// Whether a Miette report has been found.
    is_found: bool,
}

impl<'hir> Visitor<'hir> for AuthoredReportTypeVisitor<'_, '_> {
    fn visit_ty(&mut self, ty: &'hir HirTy<'hir, AmbigArg>) {
        if self.is_found {
            return;
        }
        if let TyKind::Path(path) = ty.kind
            && let Res::Def(kind, definition) = self.cx.qpath_res(&path, ty.hir_id)
        {
            self.is_found = (self.cx.tcx.crate_name(definition.krate).as_str() == "miette"
                && matches!(
                    self.cx.tcx.item_name(definition).as_str(),
                    "Report" | "Result"
                ))
                || (matches!(kind, DefKind::TyAlias)
                    && MietteReportsInLibraryInterfaces::contains_report(
                        self.cx,
                        self.cx.tcx.type_of(definition).instantiate_identity(),
                    ));
        }
        if self.is_found {
            return;
        }
        intravisit::walk_ty(self, ty);
    }
}

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
        let is_pointer = cx.tcx.crate_name(definition.did().krate).as_str() == "alloc"
            && matches!(
                cx.tcx.item_name(definition.did()).as_str(),
                "Box" | "Rc" | "Arc"
            );
        if is_pointer && !arguments.is_empty() {
            return Self::contains_report(cx, arguments.type_at(0));
        }
        cx.tcx.is_diagnostic_item(sym::Result, definition.did())
            && arguments.len() == 2
            && Self::contains_report(cx, arguments.type_at(1))
    }

    /// Checks one exported function-like definition.
    fn check_boundary(
        cx: &LateContext<'_>,
        owner: rustc_hir::HirId,
        definition: LocalDefId,
        span: Span,
        declared_output: Option<&HirTy<'_>>,
    ) {
        if !Self::library_crate(cx) || !cx.tcx.effective_visibilities(()).is_exported(definition) {
            return;
        }
        let output = cx
            .tcx
            .fn_sig(definition)
            .instantiate_identity()
            .skip_binder()
            .output();
        let authored_report = declared_output.is_some_and(|output| {
            let mut visitor = AuthoredReportTypeVisitor {
                cx,
                is_found: false,
            };
            if let Some(output) = output.try_as_ambig_ty() {
                visitor.visit_ty(output);
            }
            visitor.is_found
        });
        if !Self::contains_report(cx, output) && !authored_report {
            return;
        }

        Violation {
            owner,
            span,
            function: cx.tcx.def_path_str(definition),
        }
        .emit(cx);
    }
}
impl LateLintPass<'_> for MietteReportsInLibraryInterfaces {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Executables may choose their final rendering boundary freely.
        let ItemKind::Fn { sig, .. } = item.kind else {
            return;
        };
        if item.span.from_expansion() {
            return;
        }
        let declared_output = match sig.decl.output {
            FnRetTy::Return(output) => Some(output),
            FnRetTy::DefaultReturn(_) => None,
        };
        Self::check_boundary(
            cx,
            item.hir_id(),
            item.owner_id.def_id,
            item.span,
            declared_output,
        );
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        let ImplItemKind::Fn(signature, _) = item.kind else {
            return;
        };
        if item.span.from_expansion() {
            return;
        }
        let declared_output = match signature.decl.output {
            FnRetTy::Return(output) => Some(output),
            FnRetTy::DefaultReturn(_) => None,
        };
        Self::check_boundary(
            cx,
            item.hir_id(),
            item.owner_id.def_id,
            item.span,
            declared_output,
        );
    }

    fn check_trait_item(&mut self, cx: &LateContext<'_>, item: &TraitItem<'_>) {
        let TraitItemKind::Fn(signature, _) = item.kind else {
            return;
        };
        if item.span.from_expansion() {
            return;
        }
        let declared_output = match signature.decl.output {
            FnRetTy::Return(output) => Some(output),
            FnRetTy::DefaultReturn(_) => None,
        };
        Self::check_boundary(
            cx,
            item.hir_id(),
            item.owner_id.def_id,
            item.span,
            declared_output,
        );
    }
}
