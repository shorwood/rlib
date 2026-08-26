extern crate rustc_abi;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_abi::ExternAbi;
use rustc_errors::DiagDecorator;
use rustc_hir::{Constness, Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::hygiene::{ExpnKind, MacroKind};
use rustc_span::{Span, Symbol};

use super::utils::view_contract::ViewContract;
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::{ItemProvenanceExt, SpanProvenanceExt};

// -----------------------------------------------------------------------------
// Violation: View function without a component boundary
// -----------------------------------------------------------------------------

/// Authored free function that exposes a view without component identity or props.
struct Violation {
    /// Function declaration used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored function identifier highlighted by the diagnostic.
    span: Span,
    /// Authored function name.
    name: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "view function `{}` is not declared as a Leptos component",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "an ordinary function call hides component identity and keeps its inputs outside Leptos's declarative props interface",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "add `#[component]` and update callers to use the generated PascalCase component tag and props",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_UNANNOTATED_VIEW_FUNCTIONS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this function exposes an explicit view contract");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosUnannotatedViewFunctions: Component boundary policy
// -----------------------------------------------------------------------------

/// Late lint pass that requires authored view-returning functions to be components.
struct LeptosUnannotatedViewFunctions;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_UNANNOTATED_VIEW_FUNCTIONS,
    Warn,
    "requires authored Leptos view functions to use the component attribute",
    LeptosUnannotatedViewFunctions
}

impl LeptosUnannotatedViewFunctions {
    /// Returns whether a function was synthesized by the component or island attribute.
    fn is_component_expansion(item: &Item<'_>) -> bool {
        item.span.macro_backtrace().any(|expansion| {
            matches!(
                expansion.kind,
                ExpnKind::Macro(MacroKind::Attr, name)
                    if matches!(name.as_str(), "component" | "island")
            )
        })
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosUnannotatedViewFunctions {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Associated and non-function items cannot define free component boundaries.
        let ItemKind::Fn { sig, has_body, .. } = item.kind else {
            return;
        };

        // Component generation cannot safely represent nonordinary callable semantics.
        if !has_body
            || sig.header.abi != ExternAbi::Rust
            || sig.header.is_unsafe()
            || sig.header.is_async()
            || sig.header.constness == Constness::Const
        {
            return;
        }

        // Generated definitions have no independent component annotation to add.
        if item.span.from_expansion()
            || item.span.is_build_generated(cx)
            || item.is_framework_generated()
            || Self::is_component_expansion(item)
        {
            return;
        }

        // The explicit output contract, rather than IntoView's blanket implementation, owns policy.
        let output = ViewContract::function_output(cx, item.owner_id.def_id);

        // Ordinary renderable data does not declare component identity.
        if !ViewContract::is_view_type(cx, output) {
            return;
        }

        Violation {
            owner: item.hir_id(),
            span: cx
                .tcx
                .def_ident_span(item.owner_id.to_def_id())
                .unwrap_or(item.span),
            name: cx.tcx.item_name(item.owner_id.to_def_id()),
        }
        .emit(cx);
    }
}
