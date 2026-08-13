extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_span::def_id::LocalDefId;

#[derive(Default)]
/// Carries the `ThiserrorContractCatalog` state used by this analysis.
pub struct ThiserrorContractCatalog {
    /// Stores the `types` value used by this analysis.
    types: HashSet<LocalDefId>,
    /// Stores the `derives` value used by this analysis.
    derives: HashSet<LocalDefId>,
}

impl ThiserrorContractCatalog {
    /// Performs the `derived_type` operation for this value.
    pub(crate) fn derived_type(&self, definition: LocalDefId) -> Option<()> {
        (self.derives.contains(&definition) && self.types.contains(&definition)).then_some(())
    }

    /// Performs the `record_generated_impl` operation for this value.
    fn record_generated_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Reject inputs that do not satisfy this stage.
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

        // Prepare the values used by this stage.
        let Some(definition) = cx
            .tcx
            .type_of(item.owner_id)
            .instantiate_identity()
            .ty_adt_def()
            .and_then(|definition| definition.did().as_local())
        // Perform the next step of the analysis.
        else {
            return;
        };
        self.derives.insert(definition);
    }

    /// Performs the `check_item` operation for this value.
    pub(crate) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if item.span.from_expansion() {
            self.record_generated_impl(cx, item);
            return;
        }
        let (ItemKind::Struct(..) | ItemKind::Enum(..)) = item.kind else {
            return;
        };
        self.types.insert(item.owner_id.def_id);
    }
}
#[derive(Default)]
/// Carries the `ThiserrorAttributes` state used by this analysis.
pub(super) struct ThiserrorAttributes {
    /// Stores the `source` value used by this analysis.
    pub(super) is_source: bool,
    /// Stores the `from` value used by this analysis.
    pub(super) is_from: bool,
    /// Stores the `backtrace` value used by this analysis.
    pub(super) is_backtrace: bool,
}

impl ThiserrorAttributes {
    /// Reads thiserror field attributes.
    pub(super) fn from_attributes(attributes: &[syn::Attribute]) -> Self {
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

/// Performs the `static_error_message` step of the lint analysis.
pub(super) fn static_error_message(attributes: &[syn::Attribute]) -> Option<String> {
    // Prepare the values used by this stage.
    let attribute = attributes
        .iter()
        .find(|attribute| attribute.path().is_ident("error"))?;
    let message = match attribute.parse_args::<syn::LitStr>() {
        Ok(message) => message.value(),
        Err(_error) => return None,
    };

    // Perform the next step of the analysis.
    (!message.contains('{')).then_some(message)
}
