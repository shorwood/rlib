extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use convert_case::{Case, Casing};
use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};
use strum::EnumProperty as _;

// -----------------------------------------------------------------------------
// SerdeAttributes: Authored wire policy recovery
// -----------------------------------------------------------------------------

#[derive(Clone, Copy, strum::EnumProperty)]
/// Direction in which a value crosses its Serde wire boundary.
pub(super) enum SerdeDirection {
    #[strum(props(label = "serialization"))]
    /// Converts an in-memory value to its wire representation.
    Serialize,
    #[strum(props(label = "deserialization"))]
    /// Constructs an in-memory value from its wire representation.
    Deserialize,
}

impl SerdeDirection {
    /// Returns the public name used when explaining this Serde behavior.
    pub(super) fn label(self) -> &'static str {
        self.get_str("label")
            .expect("every Serde direction declares a label")
    }
}

#[derive(Clone, Copy)]
/// Distinguishes a member rename from a container-wide rename rule.
enum SerdeNameScope {
    /// Renames one field or variant.
    Member,
    /// Renames every field or variant in a container.
    Container,
}

impl SerdeNameScope {
    /// Classifies a parsed Serde rename attribute path.
    fn for_path(path: &syn::Path) -> Self {
        if path.is_ident("rename_all") {
            Self::Container
        } else {
            Self::Member
        }
    }
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
/// Independent Serde behaviors relevant to cross-rule contract analysis.
pub(super) enum SerdeFlag {
    /// Rejects object fields not declared by the target type.
    DenyUnknownFields,
    /// Merges a nested value's fields into the surrounding object.
    Flatten,
    /// Supplies an explicitly configured value when input is absent.
    HasDefault,
    /// Supplies `Default::default()` when input is absent.
    ImplicitDefault,
    /// An attribute outside the behaviors consumed by current rules.
    Other,
    /// Excludes the declaration while reading wire data.
    SkipDeserialize,
    /// Excludes the declaration while writing wire data.
    SkipSerialize,
    /// Selects enum variants from their data shape without a tag.
    Untagged,
}

#[derive(Default)]
/// Effective Serde naming, omission, adaptation, and representation policy.
pub(super) struct SerdeAttributes {
    /// Explicit wire name used while serializing this declaration.
    pub(super) rename_serialize: Option<String>,
    /// Explicit wire name accepted while deserializing this declaration.
    pub(super) rename_deserialize: Option<String>,
    /// Case conversion inherited by serialized child names.
    pub(super) rename_all_serialize: Option<String>,
    /// Case conversion inherited by deserialized child names.
    pub(super) rename_all_deserialize: Option<String>,
    /// Additional wire names accepted during deserialization.
    pub(super) aliases: Vec<String>,
    /// Independent Serde behaviors enabled by authored attributes.
    flags: HashSet<SerdeFlag>,
    /// Predicate that conditionally omits this value from serialized output.
    pub(super) skip_serializing_if: Option<String>,
    /// Authored adapter that controls this value's serialization.
    pub(super) serialize_with: Option<String>,
    /// Authored adapter that controls this value's deserialization.
    pub(super) deserialize_with: Option<String>,
    /// Source type represented by this Serde remote declaration.
    pub(super) remote: Option<String>,
}

impl SerdeAttributes {
    /// Parses the effective Serde policy from one declaration's attributes.
    pub(super) fn from_attributes(attributes: &[syn::Attribute]) -> Self {
        let mut result = Self::default();
        for attribute in attributes
            .iter()
            .filter(|attribute| attribute.path().is_ident("serde"))
        {
            let parsing = attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("rename") || meta.path.is_ident("rename_all") {
                    let scope = SerdeNameScope::for_path(&meta.path);
                    if meta.input.peek(syn::Token![=]) {
                        let value = meta.value()?.parse::<syn::LitStr>()?.value();
                        result.set_directional_name(scope, None, value);
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
                            result.set_directional_name(scope, direction, value);
                            Ok(())
                        })?;
                    }
                } else if meta.path.is_ident("alias") {
                    result
                        .aliases
                        .push(meta.value()?.parse::<syn::LitStr>()?.value());
                } else if meta.path.is_ident("skip") {
                    result.insert(SerdeFlag::SkipSerialize);
                    result.insert(SerdeFlag::SkipDeserialize);
                } else if meta.path.is_ident("skip_serializing") {
                    result.insert(SerdeFlag::SkipSerialize);
                } else if meta.path.is_ident("skip_deserializing") {
                    result.insert(SerdeFlag::SkipDeserialize);
                } else if meta.path.is_ident("default") {
                    result.insert(SerdeFlag::HasDefault);
                    if !meta.input.peek(syn::Token![=]) {
                        result.insert(SerdeFlag::ImplicitDefault);
                    }
                    if meta.input.peek(syn::Token![=]) {
                        let _ = meta.value()?.parse::<syn::LitStr>()?;
                    }
                } else if meta.path.is_ident("skip_serializing_if") {
                    result.skip_serializing_if =
                        Some(meta.value()?.parse::<syn::LitStr>()?.value());
                } else if meta.path.is_ident("serialize_with") {
                    result.serialize_with = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                } else if meta.path.is_ident("deserialize_with") {
                    result.deserialize_with = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                } else if meta.path.is_ident("untagged") {
                    result.insert(SerdeFlag::Untagged);
                } else if meta.path.is_ident("flatten") {
                    result.insert(SerdeFlag::Flatten);
                } else if meta.path.is_ident("deny_unknown_fields") {
                    result.insert(SerdeFlag::DenyUnknownFields);
                } else if meta.path.is_ident("other") {
                    result.insert(SerdeFlag::Other);
                } else if meta.path.is_ident("remote") {
                    result.remote = Some(meta.value()?.parse::<syn::LitStr>()?.value());
                }
                Ok(())
            });
            if parsing.is_err() {
                return result;
            }
        }
        result
    }
}

impl SerdeAttributes {
    /// Returns whether one Serde behavior flag is enabled.
    pub(super) fn has(&self, flag: SerdeFlag) -> bool {
        self.flags.contains(&flag)
    }

    /// Records one Serde behavior discovered while parsing attributes.
    fn insert(&mut self, flag: SerdeFlag) {
        self.flags.insert(flag);
    }

    /// Assigns a Serde name to the selected wire direction.
    fn set_directional_name(
        &mut self,
        scope: SerdeNameScope,
        direction: Option<SerdeDirection>,
        value: String,
    ) {
        let (serialize, deserialize) = match scope {
            SerdeNameScope::Container => (
                &mut self.rename_all_serialize,
                &mut self.rename_all_deserialize,
            ),
            SerdeNameScope::Member => (&mut self.rename_serialize, &mut self.rename_deserialize),
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
}

// -----------------------------------------------------------------------------
// SerdeCase: Container naming rules
// -----------------------------------------------------------------------------

/// Applies Serde's supported container case rules to Rust member names.
pub(super) struct SerdeCase;

impl SerdeCase {
    /// Applies an optional Serde case rule to a member name.
    pub(super) fn apply(name: &str, rule: Option<&str>) -> String {
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
}

// -----------------------------------------------------------------------------
// SerdeContractCatalog: Authored and generated contract correlation
// -----------------------------------------------------------------------------

#[derive(Clone)]
/// Authored type shape and Serde policy for one local declaration.
pub(super) struct SerdeTypeContract {
    /// Authored declaration or expression range used as the diagnostic anchor.
    pub(super) span: Span,
    /// Authored type or member name involved in the wire contract.
    pub(super) name: Symbol,
    /// Whether field visibility protects construction invariants.
    pub(super) has_restricted_fields: bool,
}

#[derive(Default)]
/// Crate-wide Serde contracts keyed by their compiler identities.
pub(super) struct SerdeContractCatalog {
    /// Authored type contracts available to cross-item Serde rules.
    types: HashMap<LocalDefId, SerdeTypeContract>,
    /// Derive macros that establish the generated behavior.
    derives: HashMap<&'static str, HashSet<LocalDefId>>,
}

impl SerdeContractCatalog {
    /// Resolves a local type confirmed to use the framework derive.
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

    /// Records the target of a framework-generated implementation.
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
            SerdeTypeContract {
                span: identifier.span,
                name: identifier.name,
                has_restricted_fields,
            },
        );
    }
}
