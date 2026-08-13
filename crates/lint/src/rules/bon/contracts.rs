extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

#[derive(Clone)]
pub(super) struct BonStructContract {
    pub(super) span: Span,
    pub(super) name: Symbol,
    pub(super) has_restricted_fields: bool,
}

#[derive(Default)]
pub(super) struct BonContractCatalog {
    structs: HashMap<LocalDefId, BonStructContract>,
    derived: HashSet<LocalDefId>,
}

impl BonContractCatalog {
    pub(super) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if item.span.from_expansion() {
            self.record_generated_impl(cx, item);
            return;
        }
        let ItemKind::Struct(identifier, _, data) = item.kind else {
            return;
        };
        self.structs.insert(
            item.owner_id.def_id,
            BonStructContract {
                span: identifier.span,
                name: identifier.name,
                has_restricted_fields: data.fields().iter().any(|field| field.vis_span.is_empty()),
            },
        );
    }

    pub(super) fn derived_struct(&self, def_id: LocalDefId) -> Option<&BonStructContract> {
        self.derived
            .contains(&def_id)
            .then(|| self.structs.get(&def_id))
            .flatten()
    }

    fn record_generated_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if !is_bon_builder_expansion(cx, item.span) || !matches!(item.kind, ItemKind::Impl(_)) {
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
        self.derived.insert(definition);
    }
}

pub(super) fn is_bon_builder_expansion(cx: &LateContext<'_>, span: Span) -> bool {
    span.macro_backtrace().any(|expansion| {
        expansion.macro_def_id.is_some_and(|definition| {
            cx.tcx.crate_name(definition.krate).as_str() == "bon_macros"
                && cx.tcx.item_name(definition).as_str() == "Builder"
        })
    })
}
