extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::source_provenance::AuthoredItemSource;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
/// Classifies `DiagnosticFieldRole` cases used by this analysis.
pub enum DiagnosticFieldRole {
    /// Represents the `DiagnosticSource` case.
    DiagnosticSource,
    /// Represents the `Label` case.
    Label,
    /// Represents the `Primary` case.
    Primary,
    /// Represents the `Related` case.
    Related,
    /// Represents the `Source` case.
    Source,
    /// Represents the `SourceCode` case.
    SourceCode,
}

#[derive(Clone, Default)]
/// Carries the `DiagnosticFieldRoles` state used by this analysis.
pub struct DiagnosticFieldRoles {
    /// Stores the `values` value used by this analysis.
    values: HashSet<DiagnosticFieldRole>,
}

impl DiagnosticFieldRoles {
    /// Performs the `analyze_diagnostic_field_roles` step of the lint analysis.
    fn analyze_diagnostic_field_roles(attributes: &[syn::Attribute]) -> Self {
        let mut roles = Self::default();
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
                let parsing = attribute.parse_nested_meta(|nested| {
                    if nested.path.is_ident("primary") {
                        roles.insert(DiagnosticFieldRole::Primary);
                    }
                    Ok(())
                });
                if parsing.is_err() {
                    return roles;
                }
            }
        }
        roles
    }
}

impl DiagnosticFieldRoles {
    /// Performs the `contains` operation for this value.
    pub(super) fn contains(&self, role: DiagnosticFieldRole) -> bool {
        self.values.contains(&role)
    }

    /// Performs the `insert` operation for this value.
    fn insert(&mut self, role: DiagnosticFieldRole) {
        self.values.insert(role);
    }
}

#[derive(Clone)]
/// Carries the `DiagnosticField` state used by this analysis.
pub struct DiagnosticField {
    /// Stores the `span` value used by this analysis.
    pub(super) span: Span,
    /// Stores the `name` value used by this analysis.
    pub(super) name: String,
    /// Stores the `roles` value used by this analysis.
    pub(super) roles: DiagnosticFieldRoles,
    /// Stores the `target` value used by this analysis.
    pub(super) target: Option<LocalDefId>,
}

#[derive(Clone, Default)]
/// Carries the `DiagnosticMetadata` state used by this analysis.
pub struct DiagnosticMetadata {
    /// Stores the `code` value used by this analysis.
    pub(super) code: Option<String>,
    /// Stores the `help` value used by this analysis.
    pub(super) help: Option<String>,
    /// Stores the `severity` value used by this analysis.
    pub(super) severity: Option<String>,
    /// Stores the `url` value used by this analysis.
    pub(super) url: Option<String>,
    /// Stores the `is_transparent` value used by this analysis.
    pub(super) is_transparent: bool,
}

impl DiagnosticMetadata {
    /// Performs the `analyze_diagnostic_metadata` step of the lint analysis.
    fn analyze_diagnostic_metadata(attributes: &[syn::Attribute]) -> Self {
        let mut metadata = Self::default();
        for attribute in attributes
            .iter()
            .filter(|attribute| attribute.path().is_ident("diagnostic"))
        {
            let parsing = attribute.parse_nested_meta(|nested| {
                if nested.path.is_ident("is_transparent") {
                    metadata.is_transparent = true;
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
            if parsing.is_err() {
                return metadata;
            }
        }
        metadata
    }
}

#[derive(Clone)]
/// Carries the `DiagnosticMember` state used by this analysis.
pub struct DiagnosticMember {
    /// Stores the `span` value used by this analysis.
    pub(super) span: Span,
    /// Stores the `name` value used by this analysis.
    pub(super) name: String,
    /// Stores the `metadata` value used by this analysis.
    pub(super) metadata: DiagnosticMetadata,
    /// Stores the `fields` value used by this analysis.
    pub(super) fields: Vec<DiagnosticField>,
}

#[derive(Clone)]
/// Carries the `DiagnosticContract` state used by this analysis.
pub struct DiagnosticContract {
    /// Stores the `span` value used by this analysis.
    pub(super) span: Span,
    /// Stores the `name` value used by this analysis.
    pub(super) name: String,
    /// Stores the `metadata` value used by this analysis.
    pub(super) metadata: DiagnosticMetadata,
    /// Stores the `fields` value used by this analysis.
    pub(super) fields: Vec<DiagnosticField>,
    /// Stores the `members` value used by this analysis.
    pub(super) members: Vec<DiagnosticMember>,
}

impl DiagnosticContract {
    /// Builds a diagnostic contract from an authored enum declaration.
    fn from_enum(cx: &LateContext<'_>, item: &Item<'_>, source: &str) -> Option<Self> {
        // Confirm the HIR and authored source both describe an enum.
        let ItemKind::Enum(identifier, _, definition) = item.kind else {
            return None;
        };
        let enumeration = match syn::parse_str::<syn::ItemEnum>(source) {
            Ok(enumeration) => enumeration,
            Err(_error) => return None,
        };

        // Build variant-level contracts in declaration order.
        let members = enumeration
            .variants
            .iter()
            .zip(definition.variants)
            .map(|(variant, hir_variant)| DiagnosticMember {
                span: hir_variant.span,
                name: variant.ident.to_string(),
                metadata: DiagnosticMetadata::analyze_diagnostic_metadata(&variant.attrs),
                fields: diagnostic_fields(cx, &variant.fields, hir_variant.data.fields()),
            })
            .collect();

        Some(Self {
            span: item.span,
            name: identifier.name.to_string(),
            metadata: DiagnosticMetadata::analyze_diagnostic_metadata(&enumeration.attrs),
            fields: Vec::new(),
            members,
        })
    }
}

/// Performs the `diagnostic_fields` step of the lint analysis.
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
            let mut roles = DiagnosticFieldRoles::analyze_diagnostic_field_roles(&field.attrs);
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

#[derive(Default)]
/// Carries the `DiagnosticCatalog` state used by this analysis.
pub struct DiagnosticCatalog {
    /// Stores the `contracts` value used by this analysis.
    contracts: HashMap<LocalDefId, DiagnosticContract>,
    /// Stores the `derives` value used by this analysis.
    derives: HashSet<LocalDefId>,
}

impl DiagnosticCatalog {
    /// Performs the `derived_contracts` operation for this value.
    pub(super) fn derived_contracts(&self) -> impl Iterator<Item = &DiagnosticContract> {
        let mut contracts = self
            .derives
            .iter()
            .filter_map(|definition| self.contracts.get(definition))
            .collect::<Vec<_>>();
        contracts.sort_by_key(|contract| contract.span.lo());
        contracts.into_iter()
    }

    /// Performs the `derived_type` operation for this value.
    pub(super) fn derived_type(&self, definition: LocalDefId) -> Option<&DiagnosticContract> {
        self.derives
            .contains(&definition)
            .then(|| self.contracts.get(&definition))
            .flatten()
    }

    /// Performs the `record_generated_impl` operation for this value.
    fn record_generated_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Reject inputs that do not satisfy this stage.
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
        self.derives.insert(definition);
    }

    /// Performs the `check_item` operation for this value.
    pub(super) fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() {
            self.record_generated_impl(cx, item);
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Prepare the values used by this stage.
        let contract = match item.kind {
            ItemKind::Struct(identifier, _, data) => {
                let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
                    return;
                };
                DiagnosticContract {
                    span: item.span,
                    name: identifier.name.to_string(),
                    metadata: DiagnosticMetadata::analyze_diagnostic_metadata(&structure.attrs),
                    fields: diagnostic_fields(cx, &structure.fields, data.fields()),
                    members: Vec::new(),
                }
            }
            ItemKind::Enum(..) => {
                let Some(contract) = DiagnosticContract::from_enum(cx, item, &source) else {
                    return;
                };
                contract
            }
            _ => return,
        };

        // Update the accumulated analysis state.
        self.contracts.insert(item.owner_id.def_id, contract);
    }
}
