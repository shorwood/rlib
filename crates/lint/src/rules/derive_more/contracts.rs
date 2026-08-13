extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

// -----------------------------------------------------------------------------
// DeriveMoreTypeContract: Authored invariants and generated derive identity
// -----------------------------------------------------------------------------

/// Authored type shape and `derive_more` macros that govern one local declaration.
#[derive(Clone)]
pub(super) struct DeriveMoreTypeContract {
    /// Authored declaration or expression range used as the diagnostic anchor.
    pub(super) span: Span,
    /// Local type name used to identify the affected derive contract.
    pub(super) name: Symbol,
    /// Whether field visibility prevents generated construction from widening access.
    pub(super) has_restricted_fields: bool,
}

/// Resolves the `derive_more` macro responsible for one generated item.
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

// -----------------------------------------------------------------------------
// DeriveMoreContractCatalog: Authored and generated contract correlation
// -----------------------------------------------------------------------------

/// Correlates authored type declarations with their `derive_more` expansions.
#[derive(Default)]
pub(super) struct DeriveMoreContractCatalog {
    /// Authored type contracts keyed by their compiler identity.
    types: HashMap<LocalDefId, DeriveMoreTypeContract>,
    /// Derive macros that establish the generated behavior.
    derives: HashMap<&'static str, HashSet<LocalDefId>>,
}

impl DeriveMoreContractCatalog {
    /// Resolves a local type confirmed to use the framework derive.
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

    /// Returns the `derive_more` macros associated with one local type.
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

    /// Returns the completed authored and generated contract for one local type.
    pub(super) fn type_contract(&self, def_id: LocalDefId) -> Option<&DeriveMoreTypeContract> {
        self.types.get(&def_id)
    }

    /// Records the target of a framework-generated implementation.
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

    /// Records authored contracts and generated implementation evidence.
    pub(super) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if item.span.from_expansion() {
            self.record_generated_impl(cx, item);
            return;
        }

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
