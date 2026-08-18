extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::component_props::ComponentProps;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Boolean component property diagnostic
// -----------------------------------------------------------------------------

/// Leptos component property whose two unnamed states form part of the component API.
struct Violation {
    /// Component entry point used to honor lint attributes on the authored component.
    owner: rustc_hir::HirId,
    /// Authored property name.
    name: Symbol,
    /// Authored property identifier highlighted by the diagnostic.
    span: Span,
    /// Resolved boolean-bearing type retained as concrete semantic evidence.
    ty: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "boolean component prop `{}` hides the state selected by its value",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "component props form a declarative UI vocabulary, while true and false leave their domain meaning and future states implicit",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "replace the boolean with a domain enum whose variants name the component states",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_BOOLEAN_COMPONENT_PROPS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    format!("resolves to boolean-bearing type `{}`", self.ty),
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosBooleanComponentProps: Component api policy
// -----------------------------------------------------------------------------

/// Late lint pass that rejects boolean state in Leptos component properties.
struct LeptosBooleanComponentProps;

impl LeptosBooleanComponentProps {
    /// Recognizes boolean state whose name already carries a standard platform meaning.
    fn is_platform_state(name: &str) -> bool {
        matches!(name, "disabled" | "invalid")
    }
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_BOOLEAN_COMPONENT_PROPS,
    Warn,
    "rejects boolean state in Leptos component properties",
    LeptosBooleanComponentProps
}

impl<'tcx> LateLintPass<'tcx> for LeptosBooleanComponentProps {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Items without generated component properties expose no prop API to inspect.
        let Some(properties) = ComponentProps::from_impl(cx, item) else {
            return;
        };

        // Report every authored property whose resolved API type carries boolean state.
        for property in properties {
            // Ignore props whose resolved wrapper tree carries no boolean state.
            if Self::is_platform_state(property.name.as_str())
                || !ComponentProps::carries_boolean(cx, property.ty)
            {
                continue;
            }

            // Retain the resolved type so the diagnostic exposes the semantic evidence.
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

// -----------------------------------------------------------------------------
// Tests: Unit tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::LeptosBooleanComponentProps;

    #[test]
    fn accepts_only_established_platform_states() {
        assert!(LeptosBooleanComponentProps::is_platform_state("disabled"));
        assert!(LeptosBooleanComponentProps::is_platform_state("invalid"));
        assert!(!LeptosBooleanComponentProps::is_platform_state("active"));
        assert!(!LeptosBooleanComponentProps::is_platform_state("compact"));
        assert!(!LeptosBooleanComponentProps::is_platform_state("featured"));
        assert!(!LeptosBooleanComponentProps::is_platform_state(
            "is_disabled"
        ));
    }
}
