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
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{FnArg, Meta, Token};

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
            LEPTOS_IMPLICIT_DEFAULT_COMPONENT_PROPS,
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
// LeptosImplicitDefaultComponentProps: Component api policy
// -----------------------------------------------------------------------------

/// Late lint pass that requires visible defaults for non-optional Leptos properties.
struct LeptosImplicitDefaultComponentProps;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_IMPLICIT_DEFAULT_COMPONENT_PROPS,
    Warn,
    "requires explicit defaults for implicitly optional Leptos component properties",
    LeptosImplicitDefaultComponentProps
}

impl LeptosImplicitDefaultComponentProps {
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
        let Ok(mut source) = cx.sess().source_map().span_to_snippet(prefix) else {
            return false;
        };
        // Complete the partial signature with a probe parameter so syn can associate every
        // preceding outer attribute with this exact property, regardless of attribute order.
        source.push_str("__rlib_prop_probe: ()) {}");
        let Ok(function) = syn::parse_str::<syn::ItemFn>(&source) else {
            return false;
        };
        let Some(FnArg::Typed(parameter)) = function.sig.inputs.last() else {
            return false;
        };
        parameter
            .attrs
            .iter()
            .any(|attribute| Self::is_optional_prop_meta(&attribute.meta))
    }

    /// Recognizes `prop` attributes whose argument list contains the `optional` option.
    fn is_optional_prop_meta(attribute: &Meta) -> bool {
        let Meta::List(attribute) = attribute else {
            return false;
        };
        if !attribute.path.is_ident("prop") {
            return false;
        }
        Punctuated::<Meta, Token![,]>::parse_terminated
            .parse2(attribute.tokens.clone())
            .is_ok_and(|options| {
                options
                    .iter()
                    .any(|option| option.path().is_ident("optional"))
            })
    }

    /// Parses one attribute body for focused syntax tests.
    #[cfg(test)]
    fn is_optional_prop_attribute(attribute: &str) -> bool {
        syn::parse_str::<Meta>(attribute)
            .is_ok_and(|attribute| Self::is_optional_prop_meta(&attribute))
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosImplicitDefaultComponentProps {
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
    use super::LeptosImplicitDefaultComponentProps;

    #[test]
    fn recognizes_optional_prop_attributes() {
        assert!(LeptosImplicitDefaultComponentProps::is_optional_prop_attribute("prop(optional)"));
        assert!(
            LeptosImplicitDefaultComponentProps::is_optional_prop_attribute("prop(into, optional)")
        );
        assert!(
            !LeptosImplicitDefaultComponentProps::is_optional_prop_attribute("prop(default = 100)")
        );
        assert!(
            !LeptosImplicitDefaultComponentProps::is_optional_prop_attribute("allow(dead_code)")
        );
    }
}

// -----------------------------------------------------------------------------
// Tests: Unit tests
// -----------------------------------------------------------------------------
