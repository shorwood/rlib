extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

#[derive(Clone)]
/// Carries the `DeriveMoreTypeContract` state used by this analysis.
pub(super) struct DeriveMoreTypeContract {
    /// Stores the `span` value used by this analysis.
    pub(super) span: Span,
    /// Stores the `name` value used by this analysis.
    pub(super) name: Symbol,
    /// Stores the `has_restricted_fields` value used by this analysis.
    pub(super) has_restricted_fields: bool,
}

/// Performs the `derive_more_expansion` step of the lint analysis.
fn derive_more_expansion(cx: &LateContext<'_>, span: Span) -> Option<&'static str> {
    span.macro_backtrace().find_map(|expansion| {
        let definition = expansion.macro_def_id?;
        if cx.tcx.crate_name(definition.krate).as_str() != "derive_more_impl" {
            return None;
        }
        match cx.tcx.item_name(definition).as_str() {
            "Add" => Some("Add"),
            "AddAssign" => Some("AddAssign"),
            "BitAnd" => Some("BitAnd"),
            "BitAndAssign" => Some("BitAndAssign"),
            "BitOr" => Some("BitOr"),
            "BitOrAssign" => Some("BitOrAssign"),
            "BitXor" => Some("BitXor"),
            "BitXorAssign" => Some("BitXorAssign"),
            "Constructor" => Some("Constructor"),
            "AsMut" => Some("AsMut"),
            "DerefMut" => Some("DerefMut"),
            "Display" => Some("Display"),
            "Error" => Some("Error"),
            "Div" => Some("Div"),
            "DivAssign" => Some("DivAssign"),
            "From" => Some("From"),
            "FromStr" => Some("FromStr"),
            "IndexMut" => Some("IndexMut"),
            "Mul" => Some("Mul"),
            "MulAssign" => Some("MulAssign"),
            "Neg" => Some("Neg"),
            "Not" => Some("Not"),
            "PartialEq" => Some("PartialEq"),
            "Product" => Some("Product"),
            "Rem" => Some("Rem"),
            "RemAssign" => Some("RemAssign"),
            "Shl" => Some("Shl"),
            "ShlAssign" => Some("ShlAssign"),
            "Shr" => Some("Shr"),
            "ShrAssign" => Some("ShrAssign"),
            "Sub" => Some("Sub"),
            "SubAssign" => Some("SubAssign"),
            "Sum" => Some("Sum"),
            "TryFrom" => Some("TryFrom"),
            "Unwrap" => Some("Unwrap"),
            _ => None,
        }
    })
}
#[derive(Default)]
/// Carries the `DeriveMoreContractCatalog` state used by this analysis.
pub(super) struct DeriveMoreContractCatalog {
    /// Stores the `types` value used by this analysis.
    types: HashMap<LocalDefId, DeriveMoreTypeContract>,
    /// Stores the `derives` value used by this analysis.
    derives: HashMap<&'static str, HashSet<LocalDefId>>,
}

impl DeriveMoreContractCatalog {
    /// Performs the `derived_type` operation for this value.
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

    /// Performs the `derives_for` operation for this value.
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

    /// Performs the `type_contract` operation for this value.
    pub(super) fn type_contract(&self, def_id: LocalDefId) -> Option<&DeriveMoreTypeContract> {
        self.types.get(&def_id)
    }

    /// Performs the `record_generated_impl` operation for this value.
    fn record_generated_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Reject inputs that do not satisfy this stage.
        if !matches!(item.kind, ItemKind::Impl(_)) {
            return;
        }
        let Some(derive) = derive_more_expansion(cx, item.span) else {
            return;
        };

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
        self.derives.entry(derive).or_default().insert(definition);
    }

    /// Performs the `check_item` operation for this value.
    pub(super) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() {
            self.record_generated_impl(cx, item);
            return;
        }

        // Prepare the values used by this stage.
        let (identifier, has_restricted_fields) = match item.kind {
            ItemKind::Struct(identifier, _, data) => (
                identifier,
                data.fields().iter().any(|field| field.vis_span.is_empty()),
            ),
            ItemKind::Enum(identifier, _, definition) => (
                identifier,
                definition
                    .variants
                    .iter()
                    .flat_map(|variant| variant.data.fields())
                    .any(|field| field.vis_span.is_empty()),
            ),
            _ => return,
        };

        // Update the accumulated analysis state.
        self.types.insert(
            item.owner_id.def_id,
            DeriveMoreTypeContract {
                span: identifier.span,
                name: identifier.name,
                has_restricted_fields,
            },
        );
    }
}
