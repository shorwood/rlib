extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

#[derive(Clone)]
pub(super) struct DeriveMoreTypeContract {
    pub(super) span: Span,
    pub(super) name: Symbol,
    pub(super) has_restricted_fields: bool,
}

#[derive(Default)]
pub(super) struct DeriveMoreContractCatalog {
    types: HashMap<LocalDefId, DeriveMoreTypeContract>,
    derives: HashMap<&'static str, HashSet<LocalDefId>>,
}

impl DeriveMoreContractCatalog {
    pub(super) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if item.span.from_expansion() {
            self.record_generated_impl(cx, item);
            return;
        }
        let ItemKind::Struct(identifier, _, data) = item.kind else {
            return;
        };
        self.types.insert(
            item.owner_id.def_id,
            DeriveMoreTypeContract {
                span: identifier.span,
                name: identifier.name,
                has_restricted_fields: data.fields().iter().any(|field| field.vis_span.is_empty()),
            },
        );
    }

    pub(super) fn derived_type(
        &self,
        def_id: LocalDefId,
        derive: &'static str,
    ) -> Option<&DeriveMoreTypeContract> {
        self.derives
            .get(derive)
            .is_some_and(|definitions| definitions.contains(&def_id))
            .then(|| self.types.get(&def_id))
            .flatten()
    }

    pub(super) fn derives_for(
        &self,
        def_id: LocalDefId,
        derives: &[&'static str],
    ) -> Vec<&'static str> {
        derives
            .iter()
            .copied()
            .filter(|derive| {
                self.derives
                    .get(derive)
                    .is_some_and(|definitions| definitions.contains(&def_id))
            })
            .collect()
    }

    pub(super) fn type_contract(&self, def_id: LocalDefId) -> Option<&DeriveMoreTypeContract> {
        self.types.get(&def_id)
    }

    fn record_generated_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if !matches!(item.kind, ItemKind::Impl(_)) {
            return;
        }
        let Some(derive) = derive_more_expansion(cx, item.span) else {
            return;
        };
        let Some(definition) = cx
            .tcx
            .type_of(item.owner_id)
            .instantiate_identity()
            .ty_adt_def()
            .and_then(|definition| definition.did().as_local())
        else {
            return;
        };
        self.derives.entry(derive).or_default().insert(definition);
    }
}

fn derive_more_expansion(cx: &LateContext<'_>, span: Span) -> Option<&'static str> {
    span.macro_backtrace().find_map(|expansion| {
        let definition = expansion.macro_def_id?;
        if cx.tcx.crate_name(definition.krate).as_str() != "derive_more_impl" {
            return None;
        }
        match cx.tcx.item_name(definition).as_str() {
            "Constructor" => Some("Constructor"),
            "AsMut" => Some("AsMut"),
            "DerefMut" => Some("DerefMut"),
            "IndexMut" => Some("IndexMut"),
            "PartialEq" => Some("PartialEq"),
            _ => None,
        }
    })
}
