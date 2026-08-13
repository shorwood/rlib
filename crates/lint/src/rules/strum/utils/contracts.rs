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
use rustc_hir::{Attribute, Expr, ExprKind, Item, ItemKind, QPath};
use rustc_lint::LateContext;
use rustc_span::symbol::sym;
use rustc_span::{Span, Symbol};
use syn::meta::ParseNestedMeta;
use syn::{LitBool, LitStr, Token};

// -----------------------------------------------------------------------------
// StrumDerive: Generated contract identity
// -----------------------------------------------------------------------------

/// Strum derive macros relevant to the framework-aware lint layer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StrumDerive {
    AsRefStr,
    Display,
    EnumCount,
    EnumDiscriminants,
    EnumIter,
    EnumMessage,
    EnumProperty,
    EnumString,
    FromRepr,
    IntoStaticStr,
    VariantArray,
    VariantNames,
}

impl StrumDerive {
    fn from_macro_name(name: &str) -> Option<Self> {
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

// -----------------------------------------------------------------------------
// EnumContract: Effective authored Strum contract
// -----------------------------------------------------------------------------

/// Effective Strum naming and behavior for one authored enum.
#[derive(Clone)]
pub struct EnumContract {
    pub(crate) def_id: LocalDefId,
    pub(crate) owner: rustc_hir::HirId,
    pub(crate) span: Span,
    source_span: Span,
    pub(crate) name: Symbol,
    pub(crate) is_public: bool,
    pub(crate) variants: Vec<VariantContract>,
    pub(crate) derives: HashSet<StrumDerive>,
    pub(crate) generated_discriminant: Option<LocalDefId>,
    pub(crate) discriminant_is_external_schema: bool,
}

impl EnumContract {
    /// Returns whether this enum has one generated Strum contract.
    pub(crate) fn derives(&self, derive: StrumDerive) -> bool {
        self.derives.contains(&derive)
    }

    /// Returns active variants for derives that honor `#[strum(disabled)]`.
    pub(crate) fn enabled_variants(&self) -> impl Iterator<Item = &VariantContract> {
        self.variants.iter().filter(|variant| !variant.disabled)
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

/// Effective Strum attributes for one compiled enum variant.
#[derive(Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "these flags model independent authored Strum attributes and variant shapes"
)]
pub struct VariantContract {
    pub(crate) def_id: LocalDefId,
    pub(crate) span: Span,
    pub(crate) name: Symbol,
    pub(crate) parser_names: Vec<String>,
    pub(crate) preferred_name: String,
    pub(crate) disabled: bool,
    pub(crate) default_capture: bool,
    pub(crate) ascii_case_insensitive: bool,
    pub(crate) has_payload: bool,
    pub(crate) has_domain_payload: bool,
    pub(crate) deprecated: bool,
    pub(crate) message: Option<String>,
    pub(crate) detailed_message: Option<String>,
    pub(crate) documentation: Option<String>,
    pub(crate) properties: Vec<(String, String)>,
    pub(crate) has_explicit_output: bool,
}

// -----------------------------------------------------------------------------
// ContractCatalog: Crate-wide derive and enum collection
// -----------------------------------------------------------------------------

/// Crate-wide authored enums plus generated Strum derive evidence.
#[derive(Default)]
pub struct ContractCatalog {
    contracts: HashMap<LocalDefId, EnumContract>,
    derives: HashMap<LocalDefId, HashSet<StrumDerive>>,
    discriminants: Vec<GeneratedDiscriminant>,
    external_schema_types: HashSet<LocalDefId>,
}

impl ContractCatalog {
    /// Records authored enums and Strum-generated items.
    pub(crate) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if item.span.from_expansion() {
            self.record_generated_item(cx, item);
            return;
        }
        if let Some(contract) = enum_contract(cx, item) {
            self.contracts.insert(contract.def_id, contract);
        }
    }

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
                contract.discriminant_is_external_schema = contract
                    .generated_discriminant
                    .is_some_and(|definition| self.external_schema_types.contains(&definition));
                contract
            })
            .collect()
    }

    fn record_generated_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.record_external_schema_impl(cx, item);
        let Some(derive) = strum_derive(cx, item.span) else {
            return;
        };
        match item.kind {
            ItemKind::Impl(_) => {
                let Some(enum_def) = cx
                    .tcx
                    .type_of(item.owner_id)
                    .instantiate_identity()
                    .ty_adt_def()
                    .and_then(|definition| definition.did().as_local())
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

    fn record_external_schema_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };
        let Some(trait_def) = implementation
            .of_trait
            .and_then(|trait_ref| trait_ref.trait_ref.trait_def_id())
        else {
            return;
        };
        if !matches!(
            cx.tcx.item_name(trait_def).as_str(),
            "Serialize" | "Deserialize"
        ) || !matches!(
            cx.tcx.crate_name(trait_def.krate).as_str(),
            "serde" | "serde_core"
        ) {
            return;
        }
        if let Some(definition) = cx
            .tcx
            .type_of(item.owner_id)
            .instantiate_identity()
            .ty_adt_def()
            .and_then(|definition| definition.did().as_local())
        {
            self.external_schema_types.insert(definition);
        }
    }
}

struct GeneratedDiscriminant {
    def_id: LocalDefId,
    source_callsite: Span,
}

/// Returns whether an item span came from one known Strum derive macro.
pub fn strum_derive(cx: &LateContext<'_>, span: Span) -> Option<StrumDerive> {
    span.macro_backtrace().find_map(|expansion| {
        let definition = expansion.macro_def_id?;
        (cx.tcx.crate_name(definition.krate).as_str() == "strum_macros")
            .then(|| StrumDerive::from_macro_name(cx.tcx.item_name(definition).as_str()))
            .flatten()
    })
}

fn enum_contract(cx: &LateContext<'_>, item: &Item<'_>) -> Option<EnumContract> {
    let ItemKind::Enum(_, _, definition) = item.kind else {
        return None;
    };
    let source_attributes = SourceAttributes::from_item(cx, item);
    let type_attributes = source_attributes
        .as_ref()
        .map_or_else(TypeAttributes::default, |source| source.ty.clone());
    let adt = cx.tcx.adt_def(item.owner_id.to_def_id());
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
                        cx.tcx.def_path_str(field_definition.did()) == "core::marker::PhantomData"
                    })
            });
            let implicit = apply_case(variant.ident.name.as_str(), type_attributes.case_style);
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
            let documentation = cx
                .tcx
                .hir_attrs(variant.hir_id)
                .iter()
                .filter_map(Attribute::doc_str)
                .map(|documentation| documentation.as_str().trim().to_owned())
                .filter(|documentation| !documentation.is_empty())
                .collect::<Vec<_>>();
            Some(VariantContract {
                def_id,
                span: variant.span,
                name: variant.ident.name,
                parser_names,
                preferred_name,
                disabled: attributes.disabled,
                default_capture: attributes.default_capture,
                ascii_case_insensitive: attributes
                    .ascii_case_insensitive
                    .unwrap_or(type_attributes.ascii_case_insensitive),
                has_payload,
                has_domain_payload,
                deprecated: cx
                    .tcx
                    .hir_attrs(variant.hir_id)
                    .iter()
                    .any(|attribute| attribute.has_name(sym::deprecated)),
                message: attributes.message,
                detailed_message: attributes.detailed_message,
                documentation: (!documentation.is_empty()).then(|| documentation.join("\n")),
                properties: attributes.properties,
                has_explicit_output: attributes.to_string.is_some(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(EnumContract {
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
        discriminant_is_external_schema: false,
    })
}

// -----------------------------------------------------------------------------
// Attribute models: Strum 0.28 helper attributes
// -----------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum CaseStyle {
    Camel,
    Kebab,
    Lower,
    Mixed,
    Pascal,
    ScreamingKebab,
    ScreamingSnake,
    Snake,
    Title,
    Train,
    Upper,
}

#[derive(Clone, Default)]
struct TypeAttributes {
    case_style: Option<CaseStyle>,
    ascii_case_insensitive: bool,
    prefix: Option<String>,
    suffix: Option<String>,
}

impl TypeAttributes {
    fn from_attrs(attributes: &[syn::Attribute]) -> Self {
        let mut output = Self::default();
        for attribute in attributes {
            if !attribute.path().is_ident("strum") {
                continue;
            }
            let _ = attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("serialize_all") {
                    output.case_style = case_style(&meta_string(&meta)?);
                } else if meta.path.is_ident("ascii_case_insensitive") {
                    output.ascii_case_insensitive = meta_bool(&meta)?;
                } else if meta.path.is_ident("prefix") {
                    output.prefix = Some(meta_string(&meta)?);
                } else if meta.path.is_ident("suffix") {
                    output.suffix = Some(meta_string(&meta)?);
                }
                Ok(())
            });
        }
        output
    }
}

#[derive(Clone, Default)]
struct VariantAttributes {
    serializations: Vec<String>,
    to_string: Option<String>,
    disabled: bool,
    default_capture: bool,
    ascii_case_insensitive: Option<bool>,
    message: Option<String>,
    detailed_message: Option<String>,
    properties: Vec<(String, String)>,
}

impl VariantAttributes {
    fn from_attrs(attributes: &[syn::Attribute]) -> Self {
        let mut output = Self::default();
        for attribute in attributes {
            if !attribute.path().is_ident("strum") {
                continue;
            }
            let _ = attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("serialize") {
                    output.serializations.push(meta_string(&meta)?);
                } else if meta.path.is_ident("to_string") {
                    output.to_string = Some(meta_string(&meta)?);
                } else if meta.path.is_ident("disabled") {
                    output.disabled = true;
                } else if meta.path.is_ident("default") {
                    output.default_capture = true;
                } else if meta.path.is_ident("ascii_case_insensitive") {
                    output.ascii_case_insensitive = Some(meta_bool(&meta)?);
                } else if meta.path.is_ident("message") {
                    output.message = Some(meta_string(&meta)?);
                } else if meta.path.is_ident("detailed_message") {
                    output.detailed_message = Some(meta_string(&meta)?);
                } else if meta.path.is_ident("props") {
                    meta.parse_nested_meta(|property| {
                        let Some(name) = property.path.get_ident() else {
                            return Ok(());
                        };
                        output
                            .properties
                            .push((name.to_string(), meta_string(&property)?));
                        Ok(())
                    })?;
                }
                Ok(())
            });
        }
        output
    }
}

struct SourceAttributes {
    ty: TypeAttributes,
    variants: HashMap<String, VariantAttributes>,
    span: Span,
}

impl SourceAttributes {
    fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        let (source, span) = enum_source(cx, item)?;
        let parsed = syn::parse_str::<syn::ItemEnum>(&source).ok()?;
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

fn enum_source(cx: &LateContext<'_>, item: &Item<'_>) -> Option<(String, Span)> {
    let source_map = cx.tcx.sess.source_map();
    let item_source = source_map.span_to_snippet(item.span).ok()?;
    let location = source_map.lookup_char_pos(item.span.lo());
    let file_source = location.file.src.as_deref()?;
    let offset = usize::try_from(item.span.lo().0.checked_sub(location.file.start_pos.0)?).ok()?;
    let bytes = file_source.as_bytes();
    let mut start = offset;
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
    let source_lo = rustc_span::BytePos(location.file.start_pos.0 + u32::try_from(start).ok()?);
    Some((
        format!("{}{}", &file_source[start..offset], item_source),
        item.span.with_lo(source_lo),
    ))
}

fn meta_string(meta: &ParseNestedMeta<'_>) -> syn::Result<String> {
    Ok(meta.value()?.parse::<LitStr>()?.value())
}

fn meta_bool(meta: &ParseNestedMeta<'_>) -> syn::Result<bool> {
    if meta.input.peek(Token![=]) {
        Ok(meta.value()?.parse::<LitBool>()?.value)
    } else {
        Ok(true)
    }
}

fn case_style(value: &str) -> Option<CaseStyle> {
    match value {
        "camelCase" => Some(CaseStyle::Camel),
        "kebab-case" => Some(CaseStyle::Kebab),
        "lowercase" => Some(CaseStyle::Lower),
        "mixed_case" => Some(CaseStyle::Mixed),
        "PascalCase" => Some(CaseStyle::Pascal),
        "SCREAMING-KEBAB-CASE" => Some(CaseStyle::ScreamingKebab),
        "SCREAMING_SNAKE_CASE" => Some(CaseStyle::ScreamingSnake),
        "snake_case" => Some(CaseStyle::Snake),
        "title_case" => Some(CaseStyle::Title),
        "Train-Case" => Some(CaseStyle::Train),
        "UPPERCASE" => Some(CaseStyle::Upper),
        _ => None,
    }
}

fn apply_case(value: &str, style: Option<CaseStyle>) -> String {
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

/// Resolves the enum type in a `Type::ITEM` path owned by one Strum trait.
pub fn strum_associated_enum(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    trait_name: &str,
    item_name: &str,
) -> Option<LocalDefId> {
    let ExprKind::Path(path) = expression.kind else {
        return None;
    };
    let definition = cx.qpath_res(&path, expression.hir_id).opt_def_id()?;
    if !matches!(
        cx.tcx.def_kind(definition),
        DefKind::AssocFn | DefKind::AssocConst { .. }
    ) {
        return None;
    }
    let contract_definition = cx
        .tcx
        .associated_item(definition)
        .trait_item_def_id()
        .unwrap_or(definition);
    if cx.tcx.item_name(contract_definition).as_str() != item_name {
        return None;
    }
    let path_name = cx.tcx.def_path_str(contract_definition);
    if !path_name.contains("strum") || !path_name.contains(trait_name) {
        return None;
    }
    let QPath::TypeRelative(ty, _) = path else {
        return None;
    };
    cx.typeck_results()
        .node_type(ty.hir_id)
        .ty_adt_def()?
        .did()
        .as_local()
}
