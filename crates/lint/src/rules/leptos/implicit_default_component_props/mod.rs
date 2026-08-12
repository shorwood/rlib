extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::{Span, Symbol};

use super::utils::component_props::{ComponentProp, ComponentProps};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Implicit component default diagnostic
// -----------------------------------------------------------------------------

/// Leptos component property whose omitted value inherits its type's generic default.
struct Violation {
    /// Component entry point used to honor lint attributes on the authored component.
    owner: rustc_hir::HirId,
    /// Authored property name.
    name: Symbol,
    /// Authored property identifier highlighted by the diagnostic.
    span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "component prop `{}` has an implicit default",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "a component default is part of its public UI contract, while `optional` silently inherits `Default::default()` from the property type",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "replace `#[prop(optional)]` with `#[prop(default = ...)]`, or use `Option<T>` when absence itself is meaningful",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            IMPLICIT_DEFAULT_COMPONENT_PROPS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "omission selects this type's generic default");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// ImplicitDefaultComponentProps: Component api policy
// -----------------------------------------------------------------------------

/// Late lint pass that requires visible defaults for non-optional Leptos properties.
struct ImplicitDefaultComponentProps;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub IMPLICIT_DEFAULT_COMPONENT_PROPS,
    Warn,
    "requires explicit defaults for implicitly optional Leptos component properties",
    ImplicitDefaultComponentProps
}

impl ImplicitDefaultComponentProps {
    /// Returns whether the resolved property type represents meaningful absence.
    fn is_option(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
        let ty::Adt(definition, _) = ty.kind() else {
            return false;
        };
        cx.tcx.crate_name(definition.did().krate).as_str() == "core"
            && cx.tcx.item_name(definition.did()).as_str() == "Option"
    }

    /// Recovers whether the authored property uses Leptos's implicit optional marker.
    fn is_implicitly_optional(cx: &LateContext<'_>, property: &ComponentProp<'_>) -> bool {
        let prefix = property.owner_span.with_hi(property.span.lo());
        let Ok(source) = cx.sess().source_map().span_to_snippet(prefix) else {
            return false;
        };
        Self::last_parameter_attribute(source.as_str())
            .is_some_and(Self::is_optional_prop_attribute)
    }

    /// Returns the outer attribute directly preceding a property binding.
    fn last_parameter_attribute(source: &str) -> Option<&str> {
        let source = source.trim_end();
        let close = source.ends_with(']').then_some(source.len() - 1)?;
        let open = source[..close].rfind("#[")?;
        Some(&source[open + 2..close])
    }

    /// Recognizes `prop` attributes whose argument list contains the `optional` option.
    fn is_optional_prop_attribute(attribute: &str) -> bool {
        let compact: String = attribute
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let Some(arguments) = compact
            .strip_prefix("prop(")
            .and_then(|attribute| attribute.strip_suffix(')'))
        else {
            return false;
        };
        arguments.split(',').any(|argument| argument == "optional")
    }
}

impl<'tcx> LateLintPass<'tcx> for ImplicitDefaultComponentProps {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let Some(properties) = ComponentProps::from_impl(cx, item) else {
            return;
        };

        // Require explicit policy only when omission is represented by a concrete value.
        for property in properties {
            if Self::is_option(cx, property.ty) || !Self::is_implicitly_optional(cx, &property) {
                continue;
            }
            Violation {
                owner: property.owner,
                name: property.name,
                span: property.span,
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
    use super::ImplicitDefaultComponentProps;

    #[test]
    fn recognizes_optional_prop_attributes() {
        assert!(ImplicitDefaultComponentProps::is_optional_prop_attribute(
            "prop(optional)"
        ));
        assert!(ImplicitDefaultComponentProps::is_optional_prop_attribute(
            "prop(into, optional)"
        ));
        assert!(!ImplicitDefaultComponentProps::is_optional_prop_attribute(
            "prop(default = 100)"
        ));
        assert!(!ImplicitDefaultComponentProps::is_optional_prop_attribute(
            "allow(dead_code)"
        ));
    }
}

// -----------------------------------------------------------------------------
// Tests: Unit tests
// -----------------------------------------------------------------------------
