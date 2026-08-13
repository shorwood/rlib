extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_span::def_id::LocalDefId;

#[derive(Default)]
pub struct ThiserrorContractCatalog {
    types: HashSet<LocalDefId>,
    derives: HashSet<LocalDefId>,
}

impl ThiserrorContractCatalog {
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

    pub(crate) fn derived_type(&self, definition: LocalDefId) -> Option<()> {
        (self.derives.contains(&definition) && self.types.contains(&definition)).then_some(())
    }

    fn record_generated_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
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
}

pub(super) fn static_error_message(attributes: &[syn::Attribute]) -> Option<String> {
    let attribute = attributes
        .iter()
        .find(|attribute| attribute.path().is_ident("error"))?;
    let message = attribute.parse_args::<syn::LitStr>().ok()?.value();
    (!message.contains('{')).then_some(message)
}

#[derive(Default)]
pub(super) struct ThiserrorAttributes {
    pub(super) source: bool,
    pub(super) from: bool,
    pub(super) backtrace: bool,
}

pub(super) fn thiserror_attributes(attributes: &[syn::Attribute]) -> ThiserrorAttributes {
    let mut result = ThiserrorAttributes::default();
    for attribute in attributes {
        if attribute.path().is_ident("source") {
            result.source = true;
        } else if attribute.path().is_ident("from") {
            result.from = true;
            result.source = true;
        } else if attribute.path().is_ident("backtrace") {
            result.backtrace = true;
        }
    }
    result
}
