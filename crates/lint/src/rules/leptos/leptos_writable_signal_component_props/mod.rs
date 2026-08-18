extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::component_props::ComponentProps;
use super::utils::reactive_capability::ReactiveCapability;
use super::utils::view_bindings::ViewBindings;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Writable component property diagnostic
// -----------------------------------------------------------------------------

/// Leptos component property that exposes unrestricted mutation authority.
struct Violation {
    /// Component entry point used to honor lint attributes on the authored component.
    owner: rustc_hir::HirId,
    /// Authored property name.
    name: Symbol,
    /// Authored property identifier highlighted by the diagnostic.
    span: Span,
    /// Resolved writable type retained as concrete semantic evidence.
    ty: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "writable component prop `{}` exposes unrestricted reactive mutation",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "descendants can mutate parent-owned state without naming or constraining the permitted transitions",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "pass read-only reactive state plus intent-bearing callbacks or domain event types",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_WRITABLE_SIGNAL_COMPONENT_PROPS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    format!("resolves to writable type `{}`", self.ty),
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosWritableSignalComponentProps: Component capability policy
// -----------------------------------------------------------------------------

/// Late lint pass that rejects writable reactive authority in Leptos component properties.
struct LeptosWritableSignalComponentProps;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_WRITABLE_SIGNAL_COMPONENT_PROPS,
    Warn,
    "rejects writable reactive authority in Leptos component properties",
    LeptosWritableSignalComponentProps
}

impl<'tcx> LateLintPass<'tcx> for LeptosWritableSignalComponentProps {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Items without generated component properties expose no prop API to inspect.
        let Some(properties) = ComponentProps::from_impl(cx, item) else {
            return;
        };

        // Report writable props unless their complete use proves transparent native binding.
        for property in properties {
            // Recover the component body owner shared by typing and view-flow queries.
            let owner_def_id = property.owner.owner.def_id;

            // Establish both capability and escape evidence before constructing a diagnostic.
            if !ReactiveCapability::carries_mutation(cx, owner_def_id, property.ty)
                || ViewBindings::exclusively_forwards_to_native_bind(
                    cx,
                    owner_def_id,
                    property.binding,
                )
            {
                continue;
            }

            // Retain the authored identity and resolved type as diagnostic evidence.
            Violation {
                owner: property.owner,
                name: property.name,
                span: property.span,
                ty: property.ty.to_string(),
            }
            .emit(cx);
        }
    }
}
