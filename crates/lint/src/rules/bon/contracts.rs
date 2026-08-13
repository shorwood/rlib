extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

#[derive(Clone)]
/// Carries the `BonStructContract` state used by this analysis.
pub(super) struct BonStructContract {
    /// Stores the `span` value used by this analysis.
    pub(super) span: Span,
    /// Stores the `name` value used by this analysis.
    pub(super) name: Symbol,
    /// Stores the `has_restricted_fields` value used by this analysis.
    pub(super) has_restricted_fields: bool,
}

/// Performs the `is_bon_expansion` step of the lint analysis.
fn is_bon_expansion(cx: &LateContext<'_>, span: Span) -> bool {
    span.macro_backtrace().any(|expansion| {
        expansion
            .macro_def_id
            .is_some_and(|definition| cx.tcx.crate_name(definition.krate).as_str() == "bon_macros")
    })
}

/// Performs the `is_bon_builder_expansion` step of the lint analysis.
fn is_bon_builder_expansion(cx: &LateContext<'_>, span: Span) -> bool {
    span.macro_backtrace().any(|expansion| {
        expansion.macro_def_id.is_some_and(|definition| {
            cx.tcx.crate_name(definition.krate).as_str() == "bon_macros"
                && cx.tcx.item_name(definition).as_str() == "Builder"
        })
    })
}
#[derive(Default)]
/// Carries the `BonContractCatalog` state used by this analysis.
pub(super) struct BonContractCatalog {
    /// Stores the `structs` value used by this analysis.
    structs: HashMap<LocalDefId, BonStructContract>,
    /// Stores the `derived` value used by this analysis.
    derived: HashSet<LocalDefId>,
    /// Stores the `generated_types` value used by this analysis.
    generated_types: HashSet<LocalDefId>,
    /// Stores the `generated_builders` value used by this analysis.
    generated_builders: HashSet<LocalDefId>,
}

impl BonContractCatalog {
    /// Performs the `derived_struct` operation for this value.
    pub(super) fn derived_struct(&self, def_id: LocalDefId) -> Option<&BonStructContract> {
        self.derived
            .contains(&def_id)
            .then(|| self.structs.get(&def_id))
            .flatten()
    }

    /// Performs the `is_generated_type` operation for this value.
    pub(super) fn is_generated_type(&self, def_id: LocalDefId) -> bool {
        self.generated_types.contains(&def_id)
    }

    /// Performs the `generated_builder_in_type` operation for this value.
    pub(super) fn generated_builder_in_type(&self, ty: Ty<'_>) -> Option<LocalDefId> {
        // Classify the current analyze_candidate.
        match ty.kind() {
            ty::Adt(definition, arguments) => {
                let local = definition.did().as_local();
                if local.is_some_and(|definition| self.generated_builders.contains(&definition)) {
                    return local;
                }
                arguments
                    .types()
                    .find_map(|nested| self.generated_builder_in_type(nested))
            }
            ty::Ref(_, nested, _) | ty::Slice(nested) | ty::Array(nested, _) => {
                self.generated_builder_in_type(*nested)
            }
            ty::Tuple(elements) => elements
                .iter()
                .find_map(|nested| self.generated_builder_in_type(nested)),
            _ => None,
        }
    }

    /// Performs the `record_generated_impl` operation for this value.
    fn record_generated_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Reject inputs that do not satisfy this stage.
        if !is_bon_expansion(cx, item.span) {
            return;
        }

        // Reject inputs that do not satisfy this stage.
        if matches!(item.kind, ItemKind::Struct(..) | ItemKind::Enum(..)) {
            self.generated_types.insert(item.owner_id.def_id);
            if item
                .kind
                .ident()
                .is_some_and(|identifier| identifier.name.as_str().ends_with("Builder"))
            {
                self.generated_builders.insert(item.owner_id.def_id);
            }
        }

        // Reject inputs that do not satisfy this stage.
        if !matches!(item.kind, ItemKind::Impl(_)) || !is_bon_builder_expansion(cx, item.span) {
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
        self.derived.insert(definition);
    }

    /// Performs the `check_item` operation for this value.
    pub(super) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() {
            self.record_generated_impl(cx, item);
            return;
        }
        let ItemKind::Struct(identifier, _, data) = item.kind else {
            return;
        };

        // Update the accumulated analysis state.
        self.structs.insert(
            item.owner_id.def_id,
            BonStructContract {
                span: identifier.span,
                name: identifier.name,
                has_restricted_fields: data.fields().iter().any(|field| field.vis_span.is_empty()),
            },
        );
    }
}
