extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap, HashSet};

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::{
    SerdeAttributes, SerdeCase, SerdeContractCatalog, SerdeDirection, SerdeFlag,
};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Flattened fields with colliding names
// -----------------------------------------------------------------------------

/// Effective wire names and flattening target for one struct field.
#[derive(Clone)]
struct FieldContract {
    /// Wire name emitted while serializing this field.
    serialize_names: Vec<String>,
    /// Wire name accepted while deserializing this field.
    deserialize_names: Vec<String>,
    /// Nested struct whose fields are merged into this struct's wire object.
    flatten_target: Option<LocalDefId>,
}

impl FieldContract {
    /// Returns this field's name in one Serde direction.
    fn directional_names(&self, direction: SerdeDirection) -> &[String] {
        match direction {
            SerdeDirection::Serialize => &self.serialize_names,
            SerdeDirection::Deserialize => &self.deserialize_names,
        }
    }
}

/// Direct and flattened wire fields contributed by one struct.
#[derive(Clone)]
struct StructContract {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Authored fields relevant to the contract.
    fields: Vec<FieldContract>,
}

impl StructContract {
    /// Resolves the names contributed by a flattened field graph.
    fn flattened_names(
        target: LocalDefId,
        direction: SerdeDirection,
        structs: &HashMap<LocalDefId, Self>,
        catalog: &SerdeContractCatalog,
        visiting: &mut HashSet<LocalDefId>,
    ) -> Vec<String> {
        let derive = match direction {
            SerdeDirection::Serialize => "Serialize",
            SerdeDirection::Deserialize => "Deserialize",
        };

        // Cycles and targets lacking the directional derive contribute no resolvable names.
        if !visiting.insert(target) || catalog.derived_type(target, derive).is_none() {
            return Vec::new();
        }

        // An unrecorded local target has no authored field contract to flatten.
        let Some(flattened) = structs.get(&target) else {
            visiting.remove(&target);
            return Vec::new();
        };
        let mut names = Vec::new();
        for field in &flattened.fields {
            names.extend(field.directional_names(direction).iter().cloned());
            let Some(nested) = field.flatten_target else {
                continue;
            };
            names.extend(Self::flattened_names(
                nested, direction, structs, catalog, visiting,
            ));
        }
        visiting.remove(&target);
        names
    }

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
            observed.extend(field.directional_names(direction).iter().cloned());
        }
        for field in &self.fields {
            let Some(target) = field.flatten_target else {
                continue;
            };
            for name in
                Self::flattened_names(target, direction, structs, catalog, &mut HashSet::new())
            {
                if observed.insert(name.clone()) {
                    continue;
                }
                collisions.insert(name);
            }
        }
        collisions
    }
}

/// Two direct or flattened fields that occupy the same wire name.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Serialization direction in which the behavior applies.
    direction: &'static str,
    /// Effective external names keyed by their declarations.
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

// -----------------------------------------------------------------------------
// SerdeFlattenedFieldCollisions: Collision-free flattening policy
// -----------------------------------------------------------------------------

/// Resolves flattened struct graphs and reports duplicate wire fields.
#[derive(Default)]
struct SerdeFlattenedFieldCollisions {
    /// Effective Serde contracts consulted after all local declarations are known.
    catalog: SerdeContractCatalog,
    /// Struct wire contracts keyed by their compiler identity.
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
        self.catalog.check_item(cx, item);

        // Only structs define the field namespace modeled by this flattening analysis.
        let ItemKind::Struct(_, _, data) = item.kind else {
            return;
        };

        // Generated structs do not define authored flattening policy.
        if item.span.from_expansion() {
            return;
        }

        // Missing authored source prevents reliable Serde attribute recovery.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Unparseable source cannot produce a trustworthy field-to-HIR correspondence.
        let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
            return;
        };
        let container = SerdeAttributes::from_attributes(&structure.attrs);

        let fields = structure
            .fields
            .iter()
            .zip(data.fields())
            .filter_map(|(field, hir_field)| {
                let rust_name = field.ident.as_ref()?.to_string();
                let attributes = SerdeAttributes::from_attributes(&field.attrs);
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
                let serialize_names = serialize
                    .then(|| {
                        attributes.rename_serialize.unwrap_or_else(|| {
                            SerdeCase::apply(&rust_name, container.rename_all_serialize.as_deref())
                        })
                    })
                    .into_iter()
                    .collect();
                let mut deserialize_names = deserialize
                    .then(|| {
                        attributes.rename_deserialize.clone().unwrap_or_else(|| {
                            SerdeCase::apply(
                                &rust_name,
                                container.rename_all_deserialize.as_deref(),
                            )
                        })
                    })
                    .into_iter()
                    .collect::<Vec<_>>();
                if deserialize {
                    deserialize_names.extend(attributes.aliases);
                }
                Some(FieldContract {
                    serialize_names,
                    deserialize_names,
                    flatten_target,
                })
            })
            .collect();

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
        let mut structures = self.structs.values().collect::<Vec<_>>();
        structures.sort_by_key(|structure| structure.span.lo());
        for structure in structures {
            for direction in [SerdeDirection::Serialize, SerdeDirection::Deserialize] {
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
                {
                    continue;
                }
                let collisions = structure.collisions(direction, &self.structs, &self.catalog);
                if collisions.is_empty() {
                    continue;
                }

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
