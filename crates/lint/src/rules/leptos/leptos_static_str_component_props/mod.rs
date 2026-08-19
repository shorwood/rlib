extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};
use rustc_span::def_id::DefId;
use rustc_span::{Span, Symbol};

use super::utils::component_props::ComponentProps;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Static string component property
// -----------------------------------------------------------------------------

/// Component property that permanently fixes text to the program binary.
struct Violation {
    /// Component entry point used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored property name.
    name: Symbol,
    /// Authored property identifier highlighted by the diagnostic.
    span: Span,
    /// Resolved property type retained as concrete semantic evidence.
    ty: String,
}

// -----------------------------------------------------------------------------
// LeptosStaticStrComponentProps: Localizable component api policy
// -----------------------------------------------------------------------------

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "component prop `{}` carries `&'static str`",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "component text is runtime presentation data and must remain replaceable by localization",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use `TextProp` for displayed text, `String` for technical values, or a domain enum for closed behavior",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_STATIC_STR_COMPONENT_PROPS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, format!("resolves to `{}`", self.ty));
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Rejects permanently borrowed strings at authored Leptos component boundaries.
struct LeptosStaticStrComponentProps;

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_STATIC_STR_COMPONENT_PROPS,
    Warn,
    "rejects static string data in Leptos component properties",
    LeptosStaticStrComponentProps
}

impl LeptosStaticStrComponentProps {
    /// Recognizes stable component plumbing whose string identity is not presentation copy.
    fn is_technical_property(name: &str) -> bool {
        matches!(
            name,
            "id" | "ids"
                | "labelled_by"
                | "described_by"
                | "controls"
                | "radio_name"
                | "field_name"
                | "filename"
        ) || name.ends_with("_id")
            || name.ends_with("_ids")
    }

    /// Finds static string references through wrappers and locally defined carriers.
    fn carries_static_str<'tcx>(
        cx: &LateContext<'tcx>,
        ty: Ty<'tcx>,
        visited: &mut HashSet<DefId>,
    ) -> bool {
        // A direct immutable or mutable static string reference violates the boundary.
        if let ty::Ref(region, inner, _) = ty.kind()
            && region.is_static()
            && inner.is_str()
        {
            return true;
        }

        // Generic wrappers, tuples, callbacks, and function signatures expose nested types.
        let nested_static_string = ty
            .walk()
            .filter_map(ty::GenericArg::as_type)
            .any(|nested| {
                matches!(nested.kind(), ty::Ref(region, inner, _) if region.is_static() && inner.is_str())
            });

        // A nested static string is enough to make the complete property type inflexible.
        if nested_static_string {
            return true;
        }

        // Foreign algebraic types are inspected only through their public generic arguments.
        let ty::Adt(definition, arguments) = ty.kind() else {
            return false;
        };

        // Foreign and already visited types cannot add safe local carrier fields to inspect.
        if !definition.did().is_local() || !visited.insert(definition.did()) {
            return false;
        }

        // Local carrier fields are part of the authored component boundary vocabulary.
        definition
            .all_fields()
            .any(|field| Self::carries_static_str(cx, field.ty(cx.tcx, arguments), visited))
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosStaticStrComponentProps {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Items without generated component properties expose no prop API to inspect.
        let Some(properties) =
            ComponentProps::from_impl(cx, item).or_else(|| ComponentProps::from_slot(cx, item))
        else {
            return;
        };

        // Report every authored property that directly or indirectly carries static text.
        for property in properties {
            if Self::is_technical_property(property.name.as_str())
                || !Self::carries_static_str(cx, property.ty, &mut HashSet::new())
            {
                continue;
            }
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
// Tests: Property-role classification
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::LeptosStaticStrComponentProps;

    #[test]
    fn distinguishes_technical_relationships_from_presentation_names() {
        for technical in [
            "id",
            "title_id",
            "tab_ids",
            "labelled_by",
            "described_by",
            "controls",
            "radio_name",
            "field_name",
            "filename",
        ] {
            assert!(LeptosStaticStrComponentProps::is_technical_property(
                technical
            ));
        }
        for presentation in ["label", "title", "description", "status", "display_name"] {
            assert!(!LeptosStaticStrComponentProps::is_technical_property(
                presentation
            ));
        }
    }
}
