extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::source_provenance::authored_item_source;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum DiagnosticFieldRole {
    DiagnosticSource,
    Label,
    Primary,
    Related,
    Source,
    SourceCode,
}

#[derive(Clone, Default)]
pub struct DiagnosticFieldRoles {
    values: HashSet<DiagnosticFieldRole>,
}

impl DiagnosticFieldRoles {
    pub(crate) fn contains(&self, role: DiagnosticFieldRole) -> bool {
        self.values.contains(&role)
    }

    fn insert(&mut self, role: DiagnosticFieldRole) {
        self.values.insert(role);
    }
}

#[derive(Clone)]
pub struct DiagnosticField {
    pub(crate) span: Span,
    pub(crate) name: String,
    pub(crate) roles: DiagnosticFieldRoles,
    pub(crate) target: Option<LocalDefId>,
}

#[derive(Clone, Default)]
pub struct DiagnosticMetadata {
    pub(crate) code: Option<String>,
    pub(crate) help: Option<String>,
    pub(crate) severity: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) transparent: bool,
}

#[derive(Clone)]
pub struct DiagnosticMember {
    pub(crate) span: Span,
    pub(crate) name: String,
    pub(crate) metadata: DiagnosticMetadata,
    pub(crate) fields: Vec<DiagnosticField>,
}

#[derive(Clone)]
pub struct DiagnosticContract {
    pub(crate) span: Span,
    pub(crate) name: String,
    pub(crate) metadata: DiagnosticMetadata,
    pub(crate) fields: Vec<DiagnosticField>,
    pub(crate) members: Vec<DiagnosticMember>,
}

#[derive(Default)]
pub struct DiagnosticCatalog {
    contracts: HashMap<LocalDefId, DiagnosticContract>,
    derives: HashSet<LocalDefId>,
}

impl DiagnosticCatalog {
    pub(crate) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if item.span.from_expansion() {
            self.record_generated_impl(cx, item);
            return;
        }
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let contract = match item.kind {
            ItemKind::Struct(identifier, _, data) => {
                let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
                    return;
                };
                DiagnosticContract {
                    span: item.span,
                    name: identifier.name.to_string(),
                    metadata: diagnostic_metadata(&structure.attrs),
                    fields: diagnostic_fields(cx, &structure.fields, data.fields()),
                    members: Vec::new(),
                }
            }
            ItemKind::Enum(identifier, _, definition) => {
                let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
                    return;
                };
                let members = enumeration
                    .variants
                    .iter()
                    .zip(definition.variants)
                    .map(|(variant, hir_variant)| DiagnosticMember {
                        span: hir_variant.span,
                        name: variant.ident.to_string(),
                        metadata: diagnostic_metadata(&variant.attrs),
                        fields: diagnostic_fields(cx, &variant.fields, hir_variant.data.fields()),
                    })
                    .collect();
                DiagnosticContract {
                    span: item.span,
                    name: identifier.name.to_string(),
                    metadata: diagnostic_metadata(&enumeration.attrs),
                    fields: Vec::new(),
                    members,
                }
            }
            _ => return,
        };
        self.contracts.insert(item.owner_id.def_id, contract);
    }

    pub(crate) fn derived_contracts(&self) -> impl Iterator<Item = &DiagnosticContract> {
        let mut contracts = self
            .derives
            .iter()
            .filter_map(|definition| self.contracts.get(definition))
            .collect::<Vec<_>>();
        contracts.sort_by_key(|contract| contract.span.lo());
        contracts.into_iter()
    }

    pub(crate) fn derived_type(&self, definition: LocalDefId) -> Option<&DiagnosticContract> {
        self.derives
            .contains(&definition)
            .then(|| self.contracts.get(&definition))
            .flatten()
    }

    fn record_generated_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if !matches!(item.kind, ItemKind::Impl(_))
            || !item.span.macro_backtrace().any(|expansion| {
                expansion.macro_def_id.is_some_and(|definition| {
                    cx.tcx.crate_name(definition.krate).as_str() == "miette_derive"
                        && cx.tcx.item_name(definition).as_str() == "Diagnostic"
                })
            })
        {
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
        self.derives.insert(definition);
    }
}

fn diagnostic_fields(
    cx: &LateContext<'_>,
    fields: &syn::Fields,
    hir_fields: &[rustc_hir::FieldDef<'_>],
) -> Vec<DiagnosticField> {
    fields
        .iter()
        .zip(hir_fields)
        .enumerate()
        .map(|(index, (field, hir_field))| {
            let name = field
                .ident
                .as_ref()
                .map_or_else(|| index.to_string(), ToString::to_string);
            let mut roles = diagnostic_field_roles(&field.attrs);
            if name == "source" {
                roles.insert(DiagnosticFieldRole::Source);
            }
            DiagnosticField {
                span: hir_field.span,
                name,
                roles,
                target: cx
                    .tcx
                    .type_of(hir_field.def_id)
                    .instantiate_identity()
                    .ty_adt_def()
                    .and_then(|definition| definition.did().as_local()),
            }
        })
        .collect()
}

fn diagnostic_field_roles(attributes: &[syn::Attribute]) -> DiagnosticFieldRoles {
    let mut roles = DiagnosticFieldRoles::default();
    for attribute in attributes {
        if attribute.path().is_ident("diagnostic_source") {
            roles.insert(DiagnosticFieldRole::DiagnosticSource);
        } else if attribute.path().is_ident("related") {
            roles.insert(DiagnosticFieldRole::Related);
        } else if attribute.path().is_ident("source") {
            roles.insert(DiagnosticFieldRole::Source);
        } else if attribute.path().is_ident("source_code") {
            roles.insert(DiagnosticFieldRole::SourceCode);
        } else if attribute.path().is_ident("label") {
            roles.insert(DiagnosticFieldRole::Label);
            let _ = attribute.parse_nested_meta(|nested| {
                if nested.path.is_ident("primary") {
                    roles.insert(DiagnosticFieldRole::Primary);
                }
                Ok(())
            });
        }
    }
    roles
}

pub fn diagnostic_metadata(attributes: &[syn::Attribute]) -> DiagnosticMetadata {
    let mut metadata = DiagnosticMetadata::default();
    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("diagnostic"))
    {
        let _ = attribute.parse_nested_meta(|nested| {
            if nested.path.is_ident("transparent") {
                metadata.transparent = true;
                return Ok(());
            }
            let key = nested.path.get_ident().map(ToString::to_string);
            let content;
            syn::parenthesized!(content in nested.input);
            let value = if content.peek(syn::LitStr) {
                content.parse::<syn::LitStr>()?.value()
            } else {
                content
                    .parse::<syn::Path>()?
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::")
            };
            match key.as_deref() {
                Some("code") => metadata.code = Some(value),
                Some("help") => metadata.help = Some(value),
                Some("severity") => metadata.severity = Some(value),
                Some("url") => metadata.url = Some(value),
                _ => {}
            }
            Ok(())
        });
    }
    metadata
}
