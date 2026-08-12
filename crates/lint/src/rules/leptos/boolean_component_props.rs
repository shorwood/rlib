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
            BOOLEAN_COMPONENT_PROPS,
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
// BooleanComponentProps: Component api policy
// -----------------------------------------------------------------------------

/// Late lint pass that rejects boolean state in Leptos component properties.
struct BooleanComponentProps;

impl BooleanComponentProps {
    /// Recognizes boolean state whose name already carries a standard platform meaning.
    fn is_platform_state(name: &str) -> bool {
        matches!(name, "disabled" | "invalid")
    }
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Rejects direct, optional, and reactive boolean properties on Leptos components when their
    /// names do not communicate a standard binary state. The platform states `disabled` and
    /// `invalid` are accepted because HTML and ARIA already define their meaning and conventional
    /// opposite. Booleans nested inside callback signatures or unknown application wrappers are
    /// not treated as component state by this rule.
    ///
    /// ### Why is this bad?
    ///
    /// Component calls read as declarative markup. A boolean value does not name the selected
    /// state at the call site, makes the opposite state implicit, and cannot grow to represent
    /// another state without changing the property type. A domain enum keeps the component's
    /// vocabulary visible in both its declaration and its uses. This does not apply to conventional
    /// platform states such as `disabled`: replacing that established binary contract with an enum
    /// would obscure interoperability rather than clarify domain vocabulary.
    ///
    /// For example, presentation flags leave the selected design variant implicit:
    ///
    /// ```rust,ignore
    /// #[component]
    /// fn Badge(compact: bool, featured: bool) -> impl IntoView {
    ///     // ...
    /// }
    /// ```
    ///
    /// Prefer a domain type that names every supported presentation:
    ///
    /// ```rust,ignore
    /// enum BadgeDensity { Comfortable, Compact }
    /// enum BadgeEmphasis { Standard, Featured }
    ///
    /// #[component]
    /// fn Badge(density: BadgeDensity, emphasis: BadgeEmphasis) -> impl IntoView {
    ///     // ...
    /// }
    /// ```
    pub BOOLEAN_COMPONENT_PROPS,
    Warn,
    "rejects boolean state in Leptos component properties",
    BooleanComponentProps
}

impl<'tcx> LateLintPass<'tcx> for BooleanComponentProps {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
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

#[cfg(test)]
mod tests {
    use super::BooleanComponentProps;

    #[test]
    fn accepts_only_established_platform_states() {
        assert!(BooleanComponentProps::is_platform_state("disabled"));
        assert!(BooleanComponentProps::is_platform_state("invalid"));
        assert!(!BooleanComponentProps::is_platform_state("active"));
        assert!(!BooleanComponentProps::is_platform_state("compact"));
        assert!(!BooleanComponentProps::is_platform_state("featured"));
        assert!(!BooleanComponentProps::is_platform_state("is_disabled"));
    }
}
