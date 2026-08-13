extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{
    SerdeAttributes, SerdeCase, SerdeContractCatalog, SerdeDirection, SerdeFlag,
};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

#[derive(Clone)]
/// Carries the `FieldContract` state used by this analysis.
struct FieldContract {
    /// Stores the `serialize_name` value used by this analysis.
    serialize_name: Option<String>,
    /// Stores the `deserialize_name` value used by this analysis.
    deserialize_name: Option<String>,
    /// Stores the `flatten_target` value used by this analysis.
    flatten_target: Option<LocalDefId>,
}

impl FieldContract {
    /// Returns this field's name in one Serde direction.
    const fn directional_name(&self, direction: SerdeDirection) -> Option<&String> {
        match direction {
            SerdeDirection::Serialize => self.serialize_name.as_ref(),
            SerdeDirection::Deserialize => self.deserialize_name.as_ref(),
        }
    }
}

#[derive(Clone)]
/// Carries the `StructContract` state used by this analysis.
struct StructContract {
    /// Stores the `definition` value used by this analysis.
    definition: LocalDefId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `fields` value used by this analysis.
    fields: Vec<FieldContract>,
}

impl StructContract {
    /// Finds wire names duplicated by this structure's flattened fields.
    fn collisions(
        &self,
        direction: SerdeDirection,
        structs: &HashMap<LocalDefId, Self>,
        catalog: &SerdeContractCatalog,
    ) -> BTreeSet<String> {
        let mut observed = BTreeSet::new();
        let mut collisions = BTreeSet::new();
        for field in &self.fields {
            let Some(name) = field.directional_name(direction) else {
                continue;
            };
            observed.insert(name.clone());
        }
        for field in &self.fields {
            let Some(target) = field.flatten_target else {
                continue;
            };
            let derive = match direction {
                SerdeDirection::Serialize => "Serialize",
                SerdeDirection::Deserialize => "Deserialize",
            };
            if catalog.derived_type(target, derive).is_none() {
                continue;
            }
            let Some(flattened) = structs.get(&target) else {
                continue;
            };
            for nested in &flattened.fields {
                let Some(name) = nested.directional_name(direction) else {
                    continue;
                };
                if observed.insert(name.clone()) {
                    continue;
                }
                collisions.insert(name.clone());
            }
        }
        collisions
    }
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `direction` value used by this analysis.
    direction: &'static str,
    /// Stores the `names` value used by this analysis.
    names: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Serde flattening creates duplicate {} names",
            self.direction
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "flattening places {} more than once in the same wire namespace",
            self.names.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("rename a colliding field or retain a nested representation")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_FLATTENED_FIELD_COLLISIONS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "these flattened schemas share wire keys");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
/// Carries the `SerdeFlattenedFieldCollisions` state used by this analysis.
struct SerdeFlattenedFieldCollisions {
    /// Stores the `catalog` value used by this analysis.
    catalog: SerdeContractCatalog,
    /// Stores the `structs` value used by this analysis.
    structs: HashMap<LocalDefId, StructContract>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_FLATTENED_FIELD_COLLISIONS,
    Warn,
    "finds concrete collisions in locally flattened Serde schemas",
    SerdeFlattenedFieldCollisions::default()
}

impl LateLintPass<'_> for SerdeFlattenedFieldCollisions {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Update the accumulated analysis state.
        self.catalog.check_item(cx, item);
        let ItemKind::Struct(_, _, data) = item.kind else {
            return;
        };
        if item.span.from_expansion() {
            return;
        }

        // Prepare the values used by this stage.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };
        let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
            return;
        };
        let container = SerdeAttributes::analyze_serde_attributes(&structure.attrs);

        // Prepare the values used by this stage.
        let fields = structure
            .fields
            .iter()
            .zip(data.fields())
            .filter_map(|(field, hir_field)| {
                let rust_name = field.ident.as_ref()?.to_string();
                let attributes = SerdeAttributes::analyze_serde_attributes(&field.attrs);
                let flattened = attributes.has(SerdeFlag::Flatten);
                let serialize = !attributes.has(SerdeFlag::SkipSerialize) && !flattened;
                let deserialize = !attributes.has(SerdeFlag::SkipDeserialize) && !flattened;
                let flatten_target = flattened
                    .then(|| {
                        cx.tcx
                            .type_of(hir_field.def_id)
                            .instantiate_identity()
                            .ty_adt_def()
                            .and_then(|definition| definition.did().as_local())
                    })
                    .flatten();
                Some(FieldContract {
                    serialize_name: serialize.then(|| {
                        attributes.rename_serialize.unwrap_or_else(|| {
                            SerdeCase::apply(&rust_name, container.rename_all_serialize.as_deref())
                        })
                    }),
                    deserialize_name: deserialize.then(|| {
                        attributes.rename_deserialize.unwrap_or_else(|| {
                            SerdeCase::apply(
                                &rust_name,
                                container.rename_all_deserialize.as_deref(),
                            )
                        })
                    }),
                    flatten_target,
                })
            })
            .collect();

        // Update the accumulated analysis state.
        self.structs.insert(
            item.owner_id.def_id,
            StructContract {
                definition: item.owner_id.def_id,
                span: item.span,
                fields,
            },
        );
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for structure in self.structs.values() {
            for direction in [SerdeDirection::Serialize, SerdeDirection::Deserialize] {
                // Reject inputs that do not satisfy this stage.
                if self
                    .catalog
                    .derived_type(
                        structure.definition,
                        match direction {
                            SerdeDirection::Serialize => "Serialize",
                            SerdeDirection::Deserialize => "Deserialize",
                        },
                    )
                    .is_none()
                // Perform the next step of the analysis.
                {
                    continue;
                }
                let collisions = structure.collisions(direction, &self.structs, &self.catalog);
                if collisions.is_empty() {
                    continue;
                }

                // Perform the next step of the analysis.
                Violation {
                    span: structure.span,
                    direction: direction.label(),
                    names: collisions
                        .into_iter()
                        .map(|name| format!("`{name}`"))
                        .collect(),
                }
                .emit(cx);
            }
        }
    }
}
