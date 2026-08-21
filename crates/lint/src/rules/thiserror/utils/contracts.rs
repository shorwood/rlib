extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_span::def_id::LocalDefId;

// -----------------------------------------------------------------------------
// ThiserrorContractCatalog: Derived error recognition
// -----------------------------------------------------------------------------

/// Authored error types correlated with thiserror-generated implementations.
#[derive(Default)]
pub struct ThiserrorContractCatalog {
    /// Authored local structs and enums eligible to be errors.
    types: HashSet<LocalDefId>,
    /// Local types for which thiserror generated an `Error` implementation.
    derives: HashSet<LocalDefId>,
}

impl ThiserrorContractCatalog {
    /// Returns whether an authored local type derives thiserror's `Error` contract.
    pub(crate) fn derived_type(&self, definition: LocalDefId) -> Option<()> {
        (self.derives.contains(&definition) && self.types.contains(&definition)).then_some(())
    }

    /// Correlates a generated `Error` implementation with its authored target.
    fn record_generated_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Only implementations emitted by thiserror's `Error` derive establish derive evidence.
        if !matches!(item.kind, ItemKind::Impl(_))
            || !item.span.macro_backtrace().any(|expansion| {
                expansion.macro_def_id.is_some_and(|definition| {
                    cx.tcx.crate_name(definition.krate).as_str() == "thiserror_impl"
                        && cx.tcx.item_name(definition).as_str() == "Error"
                })
            })
        {
            return;
        }

        // Generated implementations without a local aggregate target cannot be correlated.
        let Some(definition) = cx
            .tcx
            .type_of(item.owner_id)
            .instantiate_identity()
            .ty_adt_def()
            .and_then(|definition| definition.did().as_local())
        else {
            return;
        };
        self.derives.insert(definition);
    }

    /// Records either generated derive evidence or an authored error-shaped type.
    pub(crate) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Expanded items contribute only generated implementation evidence.
        if item.span.from_expansion() {
            self.record_generated_impl(cx, item);
            return;
        }

        // Only authored structs and enums can be targets of the error contract.
        let (ItemKind::Struct(..) | ItemKind::Enum(..)) = item.kind else {
            return;
        };
        self.types.insert(item.owner_id.def_id);
    }
}

// -----------------------------------------------------------------------------
// ThiserrorAttributes: Field role classification
// -----------------------------------------------------------------------------

/// Causal and backtrace roles declared on one thiserror field.
#[derive(Default)]
pub struct ThiserrorAttributes {
    /// Whether the field participates in `Error::source`.
    pub is_source: bool,
    /// Whether the field also generates an input conversion.
    pub is_from: bool,
    /// Whether the field supplies captured or forwarded backtrace state.
    pub is_backtrace: bool,
}

impl ThiserrorAttributes {
    /// Reads thiserror field attributes.
    pub fn from_attributes(attributes: &[syn::Attribute]) -> Self {
        let mut result = Self::default();
        for attribute in attributes {
            if attribute.path().is_ident("source") {
                result.is_source = true;
            } else if attribute.path().is_ident("from") {
                result.is_from = true;
                result.is_source = true;
            } else if attribute.path().is_ident("backtrace") {
                result.is_backtrace = true;
            }
        }
        result
    }
}

// -----------------------------------------------------------------------------
// ErrorMessage: Literal presentation extraction
// -----------------------------------------------------------------------------

/// Recovers authored error presentation without interpolation.
pub struct ErrorMessage;

impl ErrorMessage {
    /// Returns an error message only when it contains no interpolation.
    pub fn static_from(attributes: &[syn::Attribute]) -> Option<String> {
        let attribute = attributes
            .iter()
            .find(|attribute| attribute.path().is_ident("error"))?;

        // Non-literal error arguments cannot establish a static presentation string.
        let message = match attribute.parse_args::<syn::LitStr>() {
            Ok(message) => message.value(),
            // A parse failure means the attribute does not contain one literal message.
            Err(_error) => return None,
        };

        let mut rendered = String::with_capacity(message.len());
        let mut characters = message.chars().peekable();
        while let Some(character) = characters.next() {
            match character {
                '{' if characters.next_if_eq(&'{').is_some() => rendered.push('{'),
                '}' if characters.next_if_eq(&'}').is_some() => rendered.push('}'),
                // Unescaped braces introduce interpolation and make the message dynamic.
                '{' | '}' => return None,
                _ => rendered.push(character),
            }
        }
        Some(rendered)
    }
}

/// Returns whether a thiserror variant delegates its complete error contract to one field.
pub(super) fn is_transparent_error(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("error")
            && attribute
                .parse_args::<syn::Ident>()
                .is_ok_and(|argument| argument == "transparent")
    })
}
