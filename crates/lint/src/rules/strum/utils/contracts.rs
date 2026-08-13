extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use heck::{
    ToKebabCase, ToLowerCamelCase, ToShoutyKebabCase, ToShoutySnakeCase, ToSnakeCase, ToTitleCase,
    ToTrainCase, ToUpperCamelCase,
};
use rustc_hir::def::DefKind;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{Attribute, Expr, ExprKind, HirId, Item, ItemKind, QPath};
use rustc_lint::LateContext;
use rustc_span::symbol::sym;
use rustc_span::{Span, Symbol};
use syn::meta::ParseNestedMeta;
use syn::{LitBool, LitStr, Token};

use super::authored_contracts::StringTableCandidate;

/// One named value carried by a Strum `props` attribute.
#[derive(Clone)]
struct StrumProperty {
    /// Property name.
    _name: String,
    /// Property value.
    _value: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
/// Strum derive macros relevant to the framework-aware lint layer.
pub enum StrumDerive {
    /// Represents the `AsRefStr` case.
    AsRefStr,
    /// Represents the `Display` case.
    Display,
    /// Represents the `EnumCount` case.
    EnumCount,
    /// Represents the `EnumDiscriminants` case.
    EnumDiscriminants,
    /// Represents the `EnumIter` case.
    EnumIter,
    /// Represents the `EnumMessage` case.
    EnumMessage,
    /// Represents the `EnumProperty` case.
    EnumProperty,
    /// Represents the `EnumString` case.
    EnumString,
    /// Represents the `FromRepr` case.
    FromRepr,
    /// Represents the `IntoStaticStr` case.
    IntoStaticStr,
    /// Represents the `VariantArray` case.
    VariantArray,
    /// Represents the `VariantNames` case.
    VariantNames,
}

impl StrumDerive {
    /// Performs the `from_macro_name` operation for this value.
    fn from_macro_name(name: &str) -> Option<Self> {
        // Classify the current analyze_candidate.
        match name {
            "AsRefStr" => Some(Self::AsRefStr),
            "Display" => Some(Self::Display),
            "EnumCount" => Some(Self::EnumCount),
            "EnumDiscriminants" => Some(Self::EnumDiscriminants),
            "EnumIter" => Some(Self::EnumIter),
            "EnumMessage" => Some(Self::EnumMessage),
            "EnumProperty" => Some(Self::EnumProperty),
            "EnumString" => Some(Self::EnumString),
            "FromRepr" => Some(Self::FromRepr),
            "IntoStaticStr" => Some(Self::IntoStaticStr),
            "VariantArray" => Some(Self::VariantArray),
            "VariantNames" | "EnumVariantNames" => Some(Self::VariantNames),
            _ => None,
        }
    }
}

#[derive(Clone)]
/// Effective Strum naming and behavior for one authored enum.
pub struct EnumContract {
    /// Stores the `def_id` value used by this analysis.
    pub(crate) def_id: LocalDefId,
    /// Stores the `owner` value used by this analysis.
    pub(crate) owner: rustc_hir::HirId,
    /// Stores the `span` value used by this analysis.
    pub(crate) span: Span,
    /// Stores the `source_span` value used by this analysis.
    source_span: Span,
    /// Stores the `name` value used by this analysis.
    pub(crate) name: Symbol,
    /// Stores the `is_public` value used by this analysis.
    pub(crate) is_public: bool,
    /// Stores the `variants` value used by this analysis.
    pub(crate) variants: Vec<VariantContract>,
    /// Stores the `derives` value used by this analysis.
    derives: HashSet<StrumDerive>,
    /// Stores the `generated_discriminant` value used by this analysis.
    generated_discriminant: Option<LocalDefId>,
    /// Stores the `is_discriminant_external_schema` value used by this analysis.
    pub(crate) is_discriminant_external_schema: bool,
}

impl EnumContract {
    /// Performs the `analyze_enum_contract` step of the lint analysis.
    fn analyze_enum_contract(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Prepare the values used by this stage.
        let ItemKind::Enum(_, _, definition) = item.kind else {
            return None;
        };
        let source_attributes = SourceAttributes::from_item(cx, item);
        let type_attributes = source_attributes
            .as_ref()
            .map_or_else(TypeAttributes::default, |source| source.ty.clone());

        // Prepare the values used by this stage.
        let adt = cx.tcx.adt_def(item.owner_id.to_def_id());

        // Prepare the values used by this stage.
        let variants = definition
            .variants
            .iter()
            .map(|variant| {
                let def_id = variant.def_id;
                let attributes = source_attributes
                    .as_ref()
                    .and_then(|source| source.variants.get(variant.ident.name.as_str()))
                    .cloned()
                    .unwrap_or_default();
                let definition = adt
                    .variants()
                    .iter()
                    .find(|definition| definition.def_id.as_local() == Some(def_id))?;
                let has_payload = !definition.fields.is_empty();
                let has_domain_payload = definition.fields.iter().any(|field| {
                    let field_type = cx.tcx.type_of(field.did).instantiate_identity();
                    !field_type.is_unit()
                        && !field_type.ty_adt_def().is_some_and(|field_definition| {
                            cx.tcx.def_path_str(field_definition.did())
                                == "core::marker::PhantomData"
                        })
                });
                let implicit = apply_case(
                    variant.ident.name.as_str(),
                    type_attributes.analyze_case_style,
                );
                let mut parser_names = attributes.serializations.clone();
                if let Some(output) = &attributes.to_string {
                    parser_names.push(output.clone());
                }
                if parser_names.is_empty() {
                    parser_names.push(implicit.clone());
                }
                let preferred = attributes.to_string.clone().unwrap_or_else(|| {
                    attributes
                        .serializations
                        .iter()
                        .max_by_key(|serialization| serialization.len())
                        .cloned()
                        .unwrap_or(implicit)
                });
                let preferred_name = format!(
                    "{}{}{}",
                    type_attributes.prefix.as_deref().unwrap_or_default(),
                    preferred,
                    type_attributes.suffix.as_deref().unwrap_or_default()
                );
                Some(VariantContract {
                    def_id,
                    span: variant.span,
                    name: variant.ident.name,
                    parser_names,
                    preferred_name,
                    is_disabled: attributes.is_disabled,
                    is_default_capture: attributes.is_default_capture,
                    is_ascii_case_insensitive: attributes
                        .is_ascii_case_insensitive
                        .unwrap_or(type_attributes.is_ascii_case_insensitive),
                    has_payload,
                    has_domain_payload,
                    is_deprecated: cx
                        .tcx
                        .hir_attrs(variant.hir_id)
                        .iter()
                        .any(|attribute| attribute.has_name(sym::deprecated)),
                    message: attributes.message,
                    detailed_message: attributes.detailed_message,
                    documentation: Self::variant_documentation(cx, variant.hir_id),
                    properties: attributes.properties,
                    has_explicit_output: attributes.to_string.is_some(),
                })
            })
            .collect::<Option<Vec<_>>>()?;

        // Return the completed analysis result.
        Some(Self {
            def_id: item.owner_id.def_id,
            owner: item.hir_id(),
            span: item.span,
            source_span: source_attributes
                .as_ref()
                .map_or(item.span, |source| source.span),
            name: item.kind.ident()?.name,
            is_public: cx.tcx.visibility(item.owner_id.def_id).is_public(),
            variants,
            derives: HashSet::new(),
            generated_discriminant: None,
            is_discriminant_external_schema: false,
        })
    }

    /// Collects non-empty documentation lines for one enum variant.
    fn variant_documentation(cx: &LateContext<'_>, hir_id: HirId) -> Option<String> {
        let attributes = cx.tcx.hir_attrs(hir_id);
        let documentation = attributes
            .iter()
            .filter_map(Attribute::doc_str)
            .map(|documentation| documentation.as_str().trim().to_owned())
            .filter(|documentation| !documentation.is_empty())
            .collect::<Vec<_>>();
        (!documentation.is_empty()).then(|| documentation.join("\n"))
    }
}

impl EnumContract {
    /// Returns whether this enum has one generated Strum contract.
    pub(crate) fn derives(&self, derive: StrumDerive) -> bool {
        self.derives.contains(&derive)
    }

    /// Returns active variants for derives that honor `#[strum(is_disabled)]`.
    pub(crate) fn enabled_variants(&self) -> impl Iterator<Item = &VariantContract> {
        self.variants.iter().filter(|variant| !variant.is_disabled)
    }

    /// Returns whether variants already carry authored message or property metadata.
    pub(crate) fn has_authored_metadata(&self) -> bool {
        self.variants.iter().any(|variant| {
            variant.message.is_some()
                || variant.detailed_message.is_some()
                || variant.documentation.is_some()
                || !variant.properties.is_empty()
        })
    }
}

#[derive(Clone)]
/// Effective Strum attributes for one compiled enum variant.
#[expect(
    clippy::struct_excessive_bools,
    reason = "these flags model independent authored Strum attributes and variant shapes"
)]
pub struct VariantContract {
    /// Stores the `def_id` value used by this analysis.
    pub(crate) def_id: LocalDefId,
    /// Stores the `span` value used by this analysis.
    pub(crate) span: Span,
    /// Stores the `name` value used by this analysis.
    pub(crate) name: Symbol,
    /// Stores the `parser_names` value used by this analysis.
    pub(crate) parser_names: Vec<String>,
    /// Stores the `preferred_name` value used by this analysis.
    pub(crate) preferred_name: String,
    /// Stores the `is_disabled` value used by this analysis.
    pub(crate) is_disabled: bool,
    /// Stores the `is_default_capture` value used by this analysis.
    pub(crate) is_default_capture: bool,
    /// Stores the `is_ascii_case_insensitive` value used by this analysis.
    pub(crate) is_ascii_case_insensitive: bool,
    /// Stores the `has_payload` value used by this analysis.
    pub(crate) has_payload: bool,
    /// Stores the `has_domain_payload` value used by this analysis.
    pub(crate) has_domain_payload: bool,
    /// Stores the `is_deprecated` value used by this analysis.
    pub(crate) is_deprecated: bool,
    /// Stores the `message` value used by this analysis.
    message: Option<String>,
    /// Stores the `detailed_message` value used by this analysis.
    detailed_message: Option<String>,
    /// Stores the `documentation` value used by this analysis.
    documentation: Option<String>,
    /// Stores the `properties` value used by this analysis.
    properties: Vec<StrumProperty>,
    /// Stores the `has_explicit_output` value used by this analysis.
    pub(crate) has_explicit_output: bool,
}

#[derive(Default)]
/// Crate-wide authored enums plus generated Strum derive evidence.
pub struct ContractCatalog {
    /// Stores the `contracts` value used by this analysis.
    contracts: HashMap<LocalDefId, EnumContract>,
    /// Stores the `derives` value used by this analysis.
    derives: HashMap<LocalDefId, HashSet<StrumDerive>>,
    /// Stores the `discriminants` value used by this analysis.
    discriminants: Vec<GeneratedDiscriminant>,
    /// Stores the `external_schema_types` value used by this analysis.
    external_schema_types: HashSet<LocalDefId>,
}

impl ContractCatalog {
    /// Returns the completed authored contracts after associating generated derives.
    pub(crate) fn contracts(&self) -> Vec<EnumContract> {
        self.contracts
            .values()
            .cloned()
            .map(|mut contract| {
                contract.derives = self
                    .derives
                    .get(&contract.def_id)
                    .cloned()
                    .unwrap_or_default();
                contract.generated_discriminant = self
                    .discriminants
                    .iter()
                    .find(|generated| contract.source_span.contains(generated.source_callsite))
                    .map(|generated| generated.def_id);
                contract.is_discriminant_external_schema = contract
                    .generated_discriminant
                    .is_some_and(|definition| self.external_schema_types.contains(&definition));
                contract
            })
            .collect()
    }

    /// Finds the unique enum contract represented by an authored variant-name table.
    pub(crate) fn matching_string_table(
        &self,
        table: &StringTableCandidate,
    ) -> Option<EnumContract> {
        let mut matches = self.contracts().into_iter().filter(|contract| {
            table
                .enum_def
                .is_none_or(|enum_def| enum_def == contract.def_id)
                && table.values
                    == contract
                        .variants
                        .iter()
                        .map(|variant| variant.preferred_name.clone())
                        .collect::<Vec<_>>()
                && (table.enum_def.is_some()
                    || table
                        .name
                        .as_str()
                        .to_ascii_lowercase()
                        .contains(&contract.name.as_str().to_ascii_lowercase()))
        });
        let first = matches.next()?;
        matches.next().is_none().then_some(first)
    }

    /// Performs the `record_external_schema_impl` operation for this value.
    fn record_external_schema_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Prepare the values used by this stage.
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };
        let Some(trait_def) = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())
        else {
            return;
        };

        // Reject inputs that do not satisfy this stage.
        if !matches!(
            cx.tcx.item_name(trait_def).as_str(),
            "Serialize" | "Deserialize"
        ) || !matches!(
            cx.tcx.crate_name(trait_def.krate).as_str(),
            "serde" | "serde_core"
        ) {
            return;
        }

        // Reject inputs that do not satisfy this stage.
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
        self.external_schema_types.insert(definition);
    }

    /// Performs the `record_generated_item` operation for this value.
    fn record_generated_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Update the accumulated analysis state.
        self.record_external_schema_impl(cx, item);
        let Some(derive) = analyze_strum_derive(cx, item.span) else {
            return;
        };

        // Classify the current analyze_candidate.
        match item.kind {
            ItemKind::Impl(_) => {
                // Prepare the values used by this stage.
                let Some(enum_def) = cx
                    .tcx
                    .type_of(item.owner_id)
                    .instantiate_identity()
                    .ty_adt_def()
                    .and_then(|definition| definition.did().as_local())
                // Perform the next step of the analysis.
                else {
                    return;
                };
                self.derives.entry(enum_def).or_default().insert(derive);
            }
            ItemKind::Enum(..) if derive == StrumDerive::EnumDiscriminants => {
                self.discriminants.push(GeneratedDiscriminant {
                    def_id: item.owner_id.def_id,
                    source_callsite: item.span.source_callsite(),
                });
            }
            _ => {}
        }
    }

    /// Records authored enums and Strum-generated items.
    pub(crate) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if item.span.from_expansion() {
            self.record_generated_item(cx, item);
            return;
        }
        let Some(contract) = EnumContract::analyze_enum_contract(cx, item) else {
            return;
        };
        self.contracts.insert(contract.def_id, contract);
    }
}

/// Carries the `GeneratedDiscriminant` state used by this analysis.
struct GeneratedDiscriminant {
    /// Stores the `def_id` value used by this analysis.
    def_id: LocalDefId,
    /// Stores the `source_callsite` value used by this analysis.
    source_callsite: Span,
}

/// Returns whether an item span came from one known Strum derive macro.
fn analyze_strum_derive(cx: &LateContext<'_>, span: Span) -> Option<StrumDerive> {
    span.macro_backtrace().find_map(|expansion| {
        let definition = expansion.macro_def_id?;
        (cx.tcx.crate_name(definition.krate).as_str() == "strum_macros")
            .then(|| StrumDerive::from_macro_name(cx.tcx.item_name(definition).as_str()))
            .flatten()
    })
}

#[derive(Clone, Copy)]
/// Classifies `CaseStyle` cases used by this analysis.
enum CaseStyle {
    /// Represents the `Camel` case.
    Camel,
    /// Represents the `Kebab` case.
    Kebab,
    /// Represents the `Lower` case.
    Lower,
    /// Represents the `Mixed` case.
    Mixed,
    /// Represents the `Pascal` case.
    Pascal,
    /// Represents the `ScreamingKebab` case.
    ScreamingKebab,
    /// Represents the `ScreamingSnake` case.
    ScreamingSnake,
    /// Represents the `Snake` case.
    Snake,
    /// Represents the `Title` case.
    Title,
    /// Represents the `Train` case.
    Train,
    /// Represents the `Upper` case.
    Upper,
}

impl CaseStyle {
    /// Performs the `analyze_case_style` step of the lint analysis.
    fn analyze_case_style(value: &str) -> Option<Self> {
        // Classify the current analyze_candidate.
        match value {
            "camelCase" => Some(Self::Camel),
            "kebab-case" => Some(Self::Kebab),
            "lowercase" => Some(Self::Lower),
            "mixed_case" => Some(Self::Mixed),
            "PascalCase" => Some(Self::Pascal),
            "SCREAMING-KEBAB-CASE" => Some(Self::ScreamingKebab),
            "SCREAMING_SNAKE_CASE" => Some(Self::ScreamingSnake),
            "snake_case" => Some(Self::Snake),
            "title_case" => Some(Self::Title),
            "Train-Case" => Some(Self::Train),
            "UPPERCASE" => Some(Self::Upper),
            _ => None,
        }
    }
}

#[derive(Clone, Default)]
/// Carries the `TypeAttributes` state used by this analysis.
struct TypeAttributes {
    /// Stores the `analyze_case_style` value used by this analysis.
    analyze_case_style: Option<CaseStyle>,
    /// Stores the `is_ascii_case_insensitive` value used by this analysis.
    is_ascii_case_insensitive: bool,
    /// Stores the `prefix` value used by this analysis.
    prefix: Option<String>,
    /// Stores the `suffix` value used by this analysis.
    suffix: Option<String>,
}

impl TypeAttributes {
    /// Performs the `from_attrs` operation for this value.
    fn from_attrs(attributes: &[syn::Attribute]) -> Self {
        let mut output = Self::default();
        for attribute in attributes {
            if !attribute.path().is_ident("strum") {
                continue;
            }
            let parsing = attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("serialize_all") {
                    output.analyze_case_style = CaseStyle::analyze_case_style(&meta_string(&meta)?);
                } else if meta.path.is_ident("is_ascii_case_insensitive") {
                    output.is_ascii_case_insensitive = meta_bool(&meta)?;
                } else if meta.path.is_ident("prefix") {
                    output.prefix = Some(meta_string(&meta)?);
                } else if meta.path.is_ident("suffix") {
                    output.suffix = Some(meta_string(&meta)?);
                }
                Ok(())
            });
            if parsing.is_err() {
                return output;
            }
        }
        output
    }
}

#[derive(Clone, Default)]
/// Carries the `VariantAttributes` state used by this analysis.
struct VariantAttributes {
    /// Stores the `serializations` value used by this analysis.
    serializations: Vec<String>,
    /// Stores the `to_string` value used by this analysis.
    to_string: Option<String>,
    /// Stores the `is_disabled` value used by this analysis.
    is_disabled: bool,
    /// Stores the `is_default_capture` value used by this analysis.
    is_default_capture: bool,
    /// Stores the `is_ascii_case_insensitive` value used by this analysis.
    is_ascii_case_insensitive: Option<bool>,
    /// Stores the `message` value used by this analysis.
    message: Option<String>,
    /// Stores the `detailed_message` value used by this analysis.
    detailed_message: Option<String>,
    /// Stores the `properties` value used by this analysis.
    properties: Vec<StrumProperty>,
}

impl VariantAttributes {
    /// Performs the `from_attrs` operation for this value.
    fn from_attrs(attributes: &[syn::Attribute]) -> Self {
        let mut output = Self::default();
        for attribute in attributes {
            if !attribute.path().is_ident("strum") {
                continue;
            }
            let parsing = attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("serialize") {
                    output.serializations.push(meta_string(&meta)?);
                } else if meta.path.is_ident("to_string") {
                    output.to_string = Some(meta_string(&meta)?);
                } else if meta.path.is_ident("is_disabled") {
                    output.is_disabled = true;
                } else if meta.path.is_ident("default") {
                    output.is_default_capture = true;
                } else if meta.path.is_ident("is_ascii_case_insensitive") {
                    output.is_ascii_case_insensitive = Some(meta_bool(&meta)?);
                } else if meta.path.is_ident("message") {
                    output.message = Some(meta_string(&meta)?);
                } else if meta.path.is_ident("detailed_message") {
                    output.detailed_message = Some(meta_string(&meta)?);
                } else if meta.path.is_ident("props") {
                    meta.parse_nested_meta(|property| {
                        let Some(name) = property.path.get_ident() else {
                            return Ok(());
                        };
                        output.properties.push(StrumProperty {
                            _name: name.to_string(),
                            _value: meta_string(&property)?,
                        });
                        Ok(())
                    })?;
                }
                Ok(())
            });
            if parsing.is_err() {
                return output;
            }
        }
        output
    }
}

/// Carries the `SourceAttributes` state used by this analysis.
struct SourceAttributes {
    /// Stores the `ty` value used by this analysis.
    ty: TypeAttributes,
    /// Stores the `variants` value used by this analysis.
    variants: HashMap<String, VariantAttributes>,
    /// Stores the `span` value used by this analysis.
    span: Span,
}

impl SourceAttributes {
    /// Performs the `from_item` operation for this value.
    fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Prepare the values used by this stage.
        let EnumSource { source, span } = EnumSource::for_item(cx, item)?;
        let parsed = match syn::parse_str::<syn::ItemEnum>(&source) {
            Ok(parsed) => parsed,
            Err(_error) => return None,
        };

        // Return the completed analysis result.
        Some(Self {
            ty: TypeAttributes::from_attrs(&parsed.attrs),
            variants: parsed
                .variants
                .into_iter()
                .map(|variant| {
                    (
                        variant.ident.to_string(),
                        VariantAttributes::from_attrs(&variant.attrs),
                    )
                })
                .collect(),
            span,
        })
    }
}

/// Authored enum source together with the span that includes its attributes.
struct EnumSource {
    /// Source text parsed for Strum attributes.
    source: String,
    /// Span corresponding to the recovered source.
    span: Span,
}

impl EnumSource {
    /// Recovers an enum declaration together with its outer attributes.
    fn for_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Prepare the values used by this stage.
        let source_map = cx.tcx.sess.source_map();
        let item_source = match source_map.span_to_snippet(item.span) {
            Ok(source) => source,
            Err(_error) => return None,
        };
        let location = source_map.lookup_char_pos(item.span.lo());
        let file_source = location.file.src.as_deref()?;

        // Prepare the values used by this stage.
        let offset = match usize::try_from(item.span.lo().0.checked_sub(location.file.start_pos.0)?)
        {
            Ok(offset) => offset,
            Err(_error) => return None,
        };
        let bytes = file_source.as_bytes();
        let mut start = offset;

        // Process the candidates handled by this stage.
        loop {
            while start > 0 && bytes[start - 1].is_ascii_whitespace() {
                start -= 1;
            }
            if start == 0 || bytes[start - 1] != b']' {
                break;
            }
            let mut cursor = start - 1;
            let mut depth = 1_u32;
            while cursor > 0 && depth > 0 {
                cursor -= 1;
                match bytes[cursor] {
                    b']' => depth += 1,
                    b'[' => depth -= 1,
                    _ => {}
                }
            }
            if depth != 0 || cursor == 0 || bytes[cursor - 1] != b'#' {
                break;
            }
            start = cursor - 1;
        }

        // Prepare the values used by this stage.
        let source_offset = match u32::try_from(start) {
            Ok(offset) => offset,
            Err(_error) => return None,
        };
        let source_lo = rustc_span::BytePos(location.file.start_pos.0 + source_offset);

        // Return the completed analysis result.
        Some(Self {
            source: format!("{}{}", &file_source[start..offset], item_source),
            span: item.span.with_lo(source_lo),
        })
    }
}

/// Performs the `meta_string` step of the lint analysis.
fn meta_string(meta: &ParseNestedMeta<'_>) -> syn::Result<String> {
    Ok(meta.value()?.parse::<LitStr>()?.value())
}

/// Performs the `meta_bool` step of the lint analysis.
fn meta_bool(meta: &ParseNestedMeta<'_>) -> syn::Result<bool> {
    if meta.input.peek(Token![=]) {
        Ok(meta.value()?.parse::<LitBool>()?.value)
    } else {
        Ok(true)
    }
}

/// Performs the `apply_case` step of the lint analysis.
fn apply_case(value: &str, style: Option<CaseStyle>) -> String {
    // Classify the current analyze_candidate.
    match style {
        Some(CaseStyle::Camel) => value.to_lower_camel_case(),
        Some(CaseStyle::Kebab) => value.to_kebab_case(),
        Some(CaseStyle::Lower) => value.to_lowercase(),
        Some(CaseStyle::Mixed | CaseStyle::Snake) => value.to_snake_case(),
        Some(CaseStyle::Pascal) => value.to_upper_camel_case(),
        Some(CaseStyle::ScreamingKebab) => value.to_shouty_kebab_case(),
        Some(CaseStyle::ScreamingSnake) => value.to_shouty_snake_case(),
        Some(CaseStyle::Title) => value.to_title_case(),
        Some(CaseStyle::Train) => value.to_train_case(),
        Some(CaseStyle::Upper) => value.to_uppercase(),
        None => value.to_owned(),
    }
}

/// Trait and item names that identify one Strum associated contract.
pub struct StrumAssociatedItem<'name> {
    /// Strum trait owning the associated item.
    pub(crate) trait_name: &'name str,
    /// Associated function or constant name.
    pub(crate) item_name: &'name str,
}

impl StrumAssociatedItem<'_> {
    /// Resolves the enum type in a `Type::ITEM` path owned by this Strum contract.
    pub(crate) fn enum_definition(
        &self,
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
    ) -> Option<LocalDefId> {
        // Prepare the values used by this stage.
        let ExprKind::Path(path) = expression.kind else {
            return None;
        };
        let definition = cx.qpath_res(&path, expression.hir_id).opt_def_id()?;

        // Reject inputs that do not satisfy this stage.
        if !matches!(
            cx.tcx.def_kind(definition),
            DefKind::AssocFn | DefKind::AssocConst { .. }
        ) {
            return None;
        }

        // Prepare the values used by this stage.
        let contract_definition = cx
            .tcx
            .associated_item(definition)
            .trait_item_def_id()
            .unwrap_or(definition);

        // Reject inputs that do not satisfy this stage.
        if cx.tcx.item_name(contract_definition).as_str() != self.item_name {
            return None;
        }
        let path_name = cx.tcx.def_path_str(contract_definition);
        if !path_name.contains("strum") || !path_name.contains(self.trait_name) {
            return None;
        }

        // Prepare the values used by this stage.
        let QPath::TypeRelative(ty, _) = path else {
            return None;
        };

        // Perform the next step of the analysis.
        cx.typeck_results()
            .node_type(ty.hir_id)
            .ty_adt_def()?
            .did()
            .as_local()
    }
}
