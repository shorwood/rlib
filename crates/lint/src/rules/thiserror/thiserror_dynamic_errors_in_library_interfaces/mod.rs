extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, ImplItemKind, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, sym};

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Dynamic error exposed by a public function
// -----------------------------------------------------------------------------

/// A public function whose error type erases its failure vocabulary.
struct Violation {
    /// Function used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Public function highlighted by the diagnostic.
    span: Span,
    /// Function name shown to the caller.
    function: String,
    /// Dynamic error type exposed by the function.
    error: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "public library API `{}` returns dynamic error `{}`",
            self.function, self.error
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "callers cannot exhaustively classify or recover from failures through a stable concrete contract",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "return a public thiserror domain type and keep dynamic errors behind explicit source variants",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            THISERROR_DYNAMIC_ERRORS_IN_LIBRARY_INTERFACES,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this public boundary erases its error vocabulary",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// ThiserrorDynamicErrorsInLibraryInterfaces: Concrete public error policy
// -----------------------------------------------------------------------------

/// Checks public functions for dynamically typed errors.
struct ThiserrorDynamicErrorsInLibraryInterfaces;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_DYNAMIC_ERRORS_IN_LIBRARY_INTERFACES,
    Warn,
    "finds dynamic error types in public library interfaces",
    ThiserrorDynamicErrorsInLibraryInterfaces
}

impl ThiserrorDynamicErrorsInLibraryInterfaces {
    /// Success and error arguments carried by `Result`.
    const RESULT_TYPE_ARGUMENT_COUNT: usize = 2;

    /// Recognizes `Box<dyn std::error::Error>` and its `core` spelling.
    fn boxed_trait_name(cx: &LateContext<'_>, inner: Ty<'_>) -> Option<String> {
        let ty::Dynamic(predicates, ..) = inner.kind() else {
            return None;
        };
        let principal = predicates.principal()?;
        let definition = principal.def_id();
        (cx.tcx.item_name(definition).as_str() == "Error"
            && matches!(cx.tcx.crate_name(definition.krate).as_str(), "core" | "std"))
        .then(|| "Box<dyn Error>".to_owned())
    }

    /// Names dynamic error types that should stay behind a concrete domain error.
    fn error_name(cx: &LateContext<'_>, error: Ty<'_>) -> Option<String> {
        if let ty::Adt(definition, arguments) = error.kind() {
            let name_symbol = cx.tcx.item_name(definition.did());
            let name = name_symbol.as_str();
            let crate_symbol = cx.tcx.crate_name(definition.did().krate);
            let krate = crate_symbol.as_str();
            if (krate == "anyhow" && name == "Error") || (krate == "miette" && name == "Report") {
                return Some(format!("{krate}::{name}"));
            }
            if krate == "alloc" && name == "Box" && !arguments.is_empty() {
                return Self::boxed_trait_name(cx, arguments.type_at(0));
            }
        }
        None
    }

    /// Checks one effectively exported free or inherent function.
    fn check_function(
        cx: &LateContext<'_>,
        owner: rustc_hir::HirId,
        definition: LocalDefId,
        span: Span,
    ) {
        if span.from_expansion() || !cx.tcx.effective_visibilities(()).is_exported(definition) {
            return;
        }

        let output = cx
            .tcx
            .fn_sig(definition)
            .instantiate_identity()
            .skip_binder()
            .output();

        let ty::Adt(result, arguments) = output.kind() else {
            return;
        };
        if !cx.tcx.is_diagnostic_item(sym::Result, result.did())
            || arguments.len() != Self::RESULT_TYPE_ARGUMENT_COUNT
        {
            return;
        }
        let Some(error) = Self::error_name(cx, arguments.type_at(1)) else {
            return;
        };

        Violation {
            owner,
            span,
            function: cx.tcx.item_name(definition).to_string(),
            error,
        }
        .emit(cx);
    }
}
impl LateLintPass<'_> for ThiserrorDynamicErrorsInLibraryInterfaces {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if !matches!(item.kind, ItemKind::Fn { .. }) {
            return;
        }
        Self::check_function(cx, item.hir_id(), item.owner_id.def_id, item.span);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        if !matches!(item.kind, ImplItemKind::Fn(..)) {
            return;
        }
        Self::check_function(cx, item.hir_id(), item.owner_id.def_id, item.span);
    }
}
