extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::{Item, ItemKind};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// DiagnosticField: Presentation roles and referenced types
// -----------------------------------------------------------------------------

/// Miette presentation behavior assigned to a diagnostic field.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub enum DiagnosticFieldRole {
    /// Expands one field into a collection of rendered labels.
    Collection,
    /// Forwards another diagnostic's structured metadata.
    DiagnosticSource,
    /// Selects a span within source code.
    Label,
    /// Marks a label as the diagnostic's principal location.
    Primary,
    /// Presents independent sibling diagnostics.
    Related,
    /// Participates in the standard error source chain.
    Source,
    /// Supplies text from which labels render excerpts.
    SourceCode,
}

/// Set of Miette roles authored on one field.
#[derive(Clone, Default)]
pub struct DiagnosticFieldRoles {
    /// Distinct roles recognized from field attributes and conventions.
    values: HashSet<DiagnosticFieldRole>,
}

impl DiagnosticFieldRoles {
    /// Returns whether the field carries a particular presentation role.
    pub fn contains(&self, role: DiagnosticFieldRole) -> bool {
        self.values.contains(&role)
    }

    /// Adds an explicitly authored or convention-derived role.
    fn insert(&mut self, role: DiagnosticFieldRole) {
        self.values.insert(role);
    }

    /// Interprets the Miette attributes attached to a field.
    fn from_attributes(attributes: &[syn::Attribute]) -> Self {
        let mut roles = Self::default();
        for attribute in attributes {
            if attribute.path().is_ident("diagnostic_source") {
                roles.insert(DiagnosticFieldRole::DiagnosticSource);
            } else if attribute.path().is_ident("related") {
                roles.insert(DiagnosticFieldRole::Related);
            } else if attribute.path().is_ident("source") || attribute.path().is_ident("from") {
                roles.insert(DiagnosticFieldRole::Source);
            } else if attribute.path().is_ident("source_code") {
                roles.insert(DiagnosticFieldRole::SourceCode);
            } else if attribute.path().is_ident("label") {
                roles.insert(DiagnosticFieldRole::Label);
                let parsing = attribute.parse_nested_meta(|nested| {
                    if nested.path.is_ident("primary") {
                        roles.insert(DiagnosticFieldRole::Primary);
                    } else if nested.path.is_ident("collection") {
                        roles.insert(DiagnosticFieldRole::Collection);
                    }
                    Ok(())
                });

                // Preserve recognized roles when malformed nested metadata stops parsing.
                if parsing.is_err() {
                    return roles;
                }
            }
        }
        roles
    }
}

/// Field-level evidence shared by the Miette policy lints.
#[derive(Clone)]
pub struct DiagnosticField {
    /// Authored field declaration.
    pub span: Span,
    /// Named field or tuple position.
    pub name: String,
    /// Presentation roles assigned to the field.
    pub roles: DiagnosticFieldRoles,
    /// Local named type represented directly or through a standard owning pointer.
    pub target: Option<LocalDefId>,
}

impl DiagnosticField {
    /// Correlates authored field attributes with resolved HIR field types.
    fn from_fields(
        cx: &LateContext<'_>,
        fields: &syn::Fields,
        hir_fields: &[rustc_hir::FieldDef<'_>],
    ) -> Vec<Self> {
        fields
            .iter()
            .zip(hir_fields)
            .enumerate()
            .map(|(index, (field, hir_field))| {
                let name = field
                    .ident
                    .as_ref()
                    .map_or_else(|| index.to_string(), ToString::to_string);
                let mut roles = DiagnosticFieldRoles::from_attributes(&field.attrs);
                if name == "source" {
                    roles.insert(DiagnosticFieldRole::Source);
                }
                Self {
                    span: hir_field.span,
                    name,
                    roles,
                    target: Self::local_target(
                        cx,
                        cx.tcx.type_of(hir_field.def_id).instantiate_identity(),
                    ),
                }
            })
            .collect()
    }

    /// Resolves a local field subject through transparent standard pointer wrappers.
    fn local_target(cx: &LateContext<'_>, ty: Ty<'_>) -> Option<LocalDefId> {
        // Only algebraic types can name local diagnostics or transparent containers.
        let ty::Adt(definition, arguments) = ty.kind() else {
            return None;
        };

        // A directly local definition is already the resolved diagnostic target.
        if let Some(local) = definition.did().as_local() {
            return Some(local);
        }
        let crate_name = cx.tcx.crate_name(definition.did().krate);
        let is_transparent_container = (crate_name.as_str() == "alloc"
            && matches!(
                cx.tcx.item_name(definition.did()).as_str(),
                "Box" | "Rc" | "Arc" | "Vec"
            ))
            || (crate_name.as_str() == "core"
                && cx.tcx.item_name(definition.did()).as_str() == "Option");
        (is_transparent_container && !arguments.is_empty())
            .then(|| Self::local_target(cx, arguments.type_at(0)))
            .flatten()
    }
}

// -----------------------------------------------------------------------------
// DiagnosticMetadata: Static derive metadata
// -----------------------------------------------------------------------------

/// Static metadata declared through `#[diagnostic(...)]`.
#[derive(Clone, Default)]
pub struct DiagnosticMetadata {
    /// Stable machine identifier, when declared.
    pub code: Option<String>,
    /// Static recovery guidance, when declared.
    pub help: Option<String>,
    /// Declared Miette severity, when overridden.
    pub severity: Option<String>,
    /// External documentation link, when declared.
    pub url: Option<String>,
    /// Whether presentation delegates to a nested diagnostic.
    pub is_transparent: bool,
}

impl DiagnosticMetadata {
    /// Interprets static metadata from diagnostic attributes.
    fn from_attributes(attributes: &[syn::Attribute]) -> Self {
        let mut metadata = Self::default();
        for attribute in attributes
            .iter()
            .filter(|attribute| attribute.path().is_ident("diagnostic"))
        {
            let parsing = attribute.parse_nested_meta(|nested| {
                // Transparent metadata is a flag and needs no value parsing.
                if nested.path.is_ident("transparent") || nested.path.is_ident("is_transparent") {
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

            // Preserve metadata recognized before malformed nested syntax stopped parsing.
            if parsing.is_err() {
                return metadata;
            }
        }
        metadata
    }
}

// -----------------------------------------------------------------------------
// DiagnosticMember: Variant-level diagnostic contract
// -----------------------------------------------------------------------------

/// Effective diagnostic contract for one enum variant.
#[derive(Clone)]
pub struct DiagnosticMember {
    /// Authored variant declaration.
    pub span: Span,
    /// Variant name.
    pub name: String,
    /// Metadata declared directly on the variant.
    pub metadata: DiagnosticMetadata,
    /// Variant fields and their presentation roles.
    pub fields: Vec<DiagnosticField>,
}

// -----------------------------------------------------------------------------
// DiagnosticContract: Complete type-level diagnostic contract
// -----------------------------------------------------------------------------

/// Authored Miette diagnostic type and its variant-level contracts.
#[derive(Clone)]
pub struct DiagnosticContract {
    /// Authored type declaration.
    pub span: Span,
    /// Diagnostic type name.
    pub name: String,
    /// Metadata shared by the complete type.
    pub metadata: DiagnosticMetadata,
    /// Fields on a diagnostic struct.
    pub fields: Vec<DiagnosticField>,
    /// Variant contracts on a diagnostic enum.
    pub members: Vec<DiagnosticMember>,
}

impl DiagnosticContract {
    /// Builds a diagnostic contract from an authored enum declaration.
    fn from_enum(cx: &LateContext<'_>, item: &Item<'_>, source: &str) -> Option<Self> {
        // Confirm the HIR and authored source both describe an enum.
        // Other HIR item kinds cannot supply variant-level diagnostic contracts.
        let ItemKind::Enum(identifier, _, definition) = item.kind else {
            return None;
        };
        let enumeration = match syn::parse_str::<syn::ItemEnum>(source) {
            Ok(enumeration) => enumeration,
            // Unparseable authored enum text cannot be correlated with HIR variants.
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
                metadata: DiagnosticMetadata::from_attributes(&variant.attrs),
                fields: DiagnosticField::from_fields(
                    cx,
                    &variant.fields,
                    hir_variant.data.fields(),
                ),
            })
            .collect();

        Some(Self {
            span: item.span,
            name: identifier.name.to_string(),
            metadata: DiagnosticMetadata::from_attributes(&enumeration.attrs),
            fields: Vec::new(),
            members,
        })
    }
}

// -----------------------------------------------------------------------------
// DiagnosticCatalog: Crate-wide derived diagnostic index
// -----------------------------------------------------------------------------

/// Index of authored contracts confirmed to derive `miette::Diagnostic`.
#[derive(Default)]
pub struct DiagnosticCatalog {
    /// Parsed contracts keyed by their local type definition.
    contracts: HashMap<LocalDefId, DiagnosticContract>,
    /// Types confirmed by generated Miette implementation expansions.
    derives: HashSet<LocalDefId>,
}

impl DiagnosticCatalog {
    /// Iterates confirmed derived contracts in source order.
    pub fn derived_contracts(&self) -> impl Iterator<Item = &DiagnosticContract> {
        let mut contracts = self
            .derives
            .iter()
            .filter_map(|definition| self.contracts.get(definition))
            .collect::<Vec<_>>();
        contracts.sort_by_key(|contract| contract.span.lo());
        contracts.into_iter()
    }

    /// Resolves a local type only when Miette generated its implementation.
    pub fn derived_type(&self, definition: LocalDefId) -> Option<&DiagnosticContract> {
        self.derives
            .contains(&definition)
            .then(|| self.contracts.get(&definition))
            .flatten()
    }

    /// Records the target of a Miette-generated `Diagnostic` implementation.
    fn record_generated_impl(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Macro provenance distinguishes a real derive from unrelated implementations.
        // Retain only implementation expansions produced by Miette's diagnostic derive.
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

        // Generated implementations without a local algebraic target provide no catalog key.
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

    /// Records authored diagnostic types and generated derive evidence.
    pub fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Expanded items contribute derive provenance rather than authored contracts.
        if item.span.from_expansion() {
            self.record_generated_impl(cx, item);
            return;
        }

        // Missing authored source prevents attribute and field correlation.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        let contract = match item.kind {
            ItemKind::Struct(identifier, _, data) => {
                // Unparseable struct text cannot provide authored diagnostic metadata.
                let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
                    return;
                };
                DiagnosticContract {
                    span: item.span,
                    name: identifier.name.to_string(),
                    metadata: DiagnosticMetadata::from_attributes(&structure.attrs),
                    fields: DiagnosticField::from_fields(cx, &structure.fields, data.fields()),
                    members: Vec::new(),
                }
            }
            ItemKind::Enum(..) => {
                // Invalid enum contracts cannot enter the derived diagnostic catalog.
                let Some(contract) = DiagnosticContract::from_enum(cx, item, &source) else {
                    return;
                };
                contract
            }
            // Other declarations cannot derive a nominal diagnostic contract.
            _ => return,
        };

        self.contracts.insert(item.owner_id.def_id, contract);
    }
}
