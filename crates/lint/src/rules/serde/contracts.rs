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

#[derive(Clone, Copy, strum::EnumProperty)]
/// Classifies `SerdeDirection` cases used by this analysis.
pub(super) enum SerdeDirection {
    #[strum(props(label = "serialization"))]
    /// Represents the `Serialize` case.
    Serialize,
    #[strum(props(label = "deserialization"))]
    /// Represents the `Deserialize` case.
    Deserialize,
}

impl SerdeDirection {
    /// Performs the `label` operation for this value.
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

#[derive(Default)]
/// Carries the `SerdeAttributes` state used by this analysis.
pub(super) struct SerdeAttributes {
    /// Stores the `rename_serialize` value used by this analysis.
    pub(super) rename_serialize: Option<String>,
    /// Stores the `rename_deserialize` value used by this analysis.
    pub(super) rename_deserialize: Option<String>,
    /// Stores the `rename_all_serialize` value used by this analysis.
    pub(super) rename_all_serialize: Option<String>,
    /// Stores the `rename_all_deserialize` value used by this analysis.
    pub(super) rename_all_deserialize: Option<String>,
    /// Stores the `aliases` value used by this analysis.
    pub(super) aliases: Vec<String>,
    /// Stores the `flags` value used by this analysis.
    flags: HashSet<SerdeFlag>,
    /// Stores the `skip_serializing_if` value used by this analysis.
    pub(super) skip_serializing_if: Option<String>,
    /// Stores the `serialize_with` value used by this analysis.
    pub(super) serialize_with: Option<String>,
    /// Stores the `deserialize_with` value used by this analysis.
    pub(super) deserialize_with: Option<String>,
    /// Stores the `remote` value used by this analysis.
    pub(super) remote: Option<String>,
}

impl SerdeAttributes {
    /// Performs the `analyze_serde_attributes` step of the lint analysis.
    pub(super) fn analyze_serde_attributes(attributes: &[syn::Attribute]) -> Self {
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
    /// Performs the `has` operation for this value.
    pub(super) fn has(&self, flag: SerdeFlag) -> bool {
        self.flags.contains(&flag)
    }

    /// Performs the `insert` operation for this value.
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

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
/// Classifies `SerdeFlag` cases used by this analysis.
pub(super) enum SerdeFlag {
    /// Represents the `DenyUnknownFields` case.
    DenyUnknownFields,
    /// Represents the `Flatten` case.
    Flatten,
    /// Represents the `HasDefault` case.
    HasDefault,
    /// Represents the `ImplicitDefault` case.
    ImplicitDefault,
    /// Represents the `Other` case.
    Other,
    /// Represents the `SkipDeserialize` case.
    SkipDeserialize,
    /// Represents the `SkipSerialize` case.
    SkipSerialize,
    /// Represents the `Untagged` case.
    Untagged,
}

/// Applies Serde's supported container case rules to Rust member names.
pub(super) struct SerdeCase;

impl SerdeCase {
    /// Applies an optional Serde case rule to a member name.
    pub(super) fn apply(name: &str, rule: Option<&str>) -> String {
        // Classify the current analyze_candidate.
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

#[derive(Clone)]
/// Carries the `SerdeTypeContract` state used by this analysis.
pub(super) struct SerdeTypeContract {
    /// Stores the `span` value used by this analysis.
    pub(super) span: Span,
    /// Stores the `name` value used by this analysis.
    pub(super) name: Symbol,
    /// Stores the `has_restricted_fields` value used by this analysis.
    pub(super) has_restricted_fields: bool,
}

#[derive(Default)]
/// Carries the `SerdeContractCatalog` state used by this analysis.
pub(super) struct SerdeContractCatalog {
    /// Stores the `types` value used by this analysis.
    types: HashMap<LocalDefId, SerdeTypeContract>,
    /// Stores the `derives` value used by this analysis.
    derives: HashMap<&'static str, HashSet<LocalDefId>>,
}

impl SerdeContractCatalog {
    /// Performs the `derived_type` operation for this value.
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

    /// Performs the `record_generated_impl` operation for this value.
    fn record_generated_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Reject inputs that do not satisfy this stage.
        if !matches!(item.kind, ItemKind::Impl(_)) {
            return;
        }

        // Prepare the values used by this stage.
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
            SerdeTypeContract {
                span: identifier.span,
                name: identifier.name,
                has_restricted_fields,
            },
        );
    }
}
