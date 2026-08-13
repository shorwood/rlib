extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use convert_case::{Case, Casing};
use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

#[derive(Clone, Copy)]
pub(super) enum SerdeDirection {
    Serialize,
    Deserialize,
}

impl SerdeDirection {
    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Serialize => "serialization",
            Self::Deserialize => "deserialization",
        }
    }
}

#[derive(Default)]
pub(super) struct SerdeAttributes {
    pub(super) rename_serialize: Option<String>,
    pub(super) rename_deserialize: Option<String>,
    pub(super) rename_all_serialize: Option<String>,
    pub(super) rename_all_deserialize: Option<String>,
    pub(super) aliases: Vec<String>,
    pub(super) skip_serialize: bool,
    pub(super) skip_deserialize: bool,
    pub(super) implicit_default: bool,
    pub(super) has_default: bool,
    pub(super) skip_serializing_if: Option<String>,
    pub(super) serialize_with: Option<String>,
    pub(super) deserialize_with: Option<String>,
    pub(super) untagged: bool,
}

pub(super) fn serde_attributes(attributes: &[syn::Attribute]) -> SerdeAttributes {
    let mut result = SerdeAttributes::default();
    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("serde"))
    {
        let _ = attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") || meta.path.is_ident("rename_all") {
                let rename_all = meta.path.is_ident("rename_all");
                if meta.input.peek(syn::Token![=]) {
                    let value = meta.value()?.parse::<syn::LitStr>()?.value();
                    set_directional_name(&mut result, rename_all, None, value);
                } else {
                    meta.parse_nested_meta(|direction| {
                        let value = direction.value()?.parse::<syn::LitStr>()?.value();
                        let direction = if direction.path.is_ident("serialize") {
                            Some(SerdeDirection::Serialize)
                        } else if direction.path.is_ident("deserialize") {
                            Some(SerdeDirection::Deserialize)
                        } else {
                            None
                        };
                        set_directional_name(&mut result, rename_all, direction, value);
                        Ok(())
                    })?;
                }
            } else if meta.path.is_ident("alias") {
                result
                    .aliases
                    .push(meta.value()?.parse::<syn::LitStr>()?.value());
            } else if meta.path.is_ident("skip") {
                result.skip_serialize = true;
                result.skip_deserialize = true;
            } else if meta.path.is_ident("skip_serializing") {
                result.skip_serialize = true;
            } else if meta.path.is_ident("skip_deserializing") {
                result.skip_deserialize = true;
            } else if meta.path.is_ident("default") {
                result.has_default = true;
                result.implicit_default = !meta.input.peek(syn::Token![=]);
                if meta.input.peek(syn::Token![=]) {
                    let _ = meta.value()?.parse::<syn::LitStr>()?;
                }
            } else if meta.path.is_ident("skip_serializing_if") {
                result.skip_serializing_if = Some(meta.value()?.parse::<syn::LitStr>()?.value());
            } else if meta.path.is_ident("serialize_with") {
                result.serialize_with = Some(meta.value()?.parse::<syn::LitStr>()?.value());
            } else if meta.path.is_ident("deserialize_with") {
                result.deserialize_with = Some(meta.value()?.parse::<syn::LitStr>()?.value());
            } else if meta.path.is_ident("untagged") {
                result.untagged = true;
            }
            Ok(())
        });
    }
    result
}

fn set_directional_name(
    attributes: &mut SerdeAttributes,
    rename_all: bool,
    direction: Option<SerdeDirection>,
    value: String,
) {
    let (serialize, deserialize) = if rename_all {
        (
            &mut attributes.rename_all_serialize,
            &mut attributes.rename_all_deserialize,
        )
    } else {
        (
            &mut attributes.rename_serialize,
            &mut attributes.rename_deserialize,
        )
    };
    match direction {
        Some(SerdeDirection::Serialize) => *serialize = Some(value),
        Some(SerdeDirection::Deserialize) => *deserialize = Some(value),
        None => {
            *serialize = Some(value.clone());
            *deserialize = Some(value);
        }
    }
}

pub(super) fn apply_case(name: &str, rule: Option<&str>) -> String {
    match rule {
        Some("lowercase") => name.to_ascii_lowercase(),
        Some("UPPERCASE") => name.to_ascii_uppercase(),
        Some("PascalCase") => name.to_case(Case::Pascal),
        Some("camelCase") => name.to_case(Case::Camel),
        Some("snake_case") => name.to_case(Case::Snake),
        Some("SCREAMING_SNAKE_CASE") => name.to_case(Case::Constant),
        Some("kebab-case") => name.to_case(Case::Kebab),
        Some("SCREAMING-KEBAB-CASE") => name.to_case(Case::Cobol),
        _ => name.to_owned(),
    }
}

#[derive(Clone)]
pub(super) struct SerdeTypeContract {
    pub(super) span: Span,
    pub(super) name: Symbol,
    pub(super) has_restricted_fields: bool,
}

#[derive(Default)]
pub(super) struct SerdeContractCatalog {
    types: HashMap<LocalDefId, SerdeTypeContract>,
    derives: HashMap<&'static str, HashSet<LocalDefId>>,
}

impl SerdeContractCatalog {
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
            SerdeTypeContract {
                span: identifier.span,
                name: identifier.name,
                has_restricted_fields,
            },
        );
    }

    pub(super) fn derived_type(
        &self,
        definition: LocalDefId,
        derive: &'static str,
    ) -> Option<&SerdeTypeContract> {
        self.derives
            .get(derive)
            .is_some_and(|definitions| definitions.contains(&definition))
            .then(|| self.types.get(&definition))
            .flatten()
    }

    fn record_generated_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if !matches!(item.kind, ItemKind::Impl(_)) {
            return;
        }
        let Some(derive) = item.span.macro_backtrace().find_map(|expansion| {
            let definition = expansion.macro_def_id?;
            (cx.tcx.crate_name(definition.krate).as_str() == "serde_derive")
                .then(|| match cx.tcx.item_name(definition).as_str() {
                    "Serialize" => Some("Serialize"),
                    "Deserialize" => Some("Deserialize"),
                    _ => None,
                })
                .flatten()
        }) else {
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
