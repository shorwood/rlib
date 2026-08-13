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

use super::contracts::{SerdeContractCatalog, SerdeDirection, apply_case, serde_attributes};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

#[derive(Clone)]
struct FieldContract {
    serialize_name: Option<String>,
    deserialize_name: Option<String>,
    flatten_target: Option<LocalDefId>,
}

#[derive(Clone)]
struct StructContract {
    definition: LocalDefId,
    span: Span,
    fields: Vec<FieldContract>,
}

struct Violation {
    span: Span,
    direction: &'static str,
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
struct SerdeFlattenedFieldCollisions {
    catalog: SerdeContractCatalog,
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
        let ItemKind::Struct(_, _, data) = item.kind else {
            return;
        };
        if item.span.from_expansion() {
            return;
        }
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
            return;
        };
        let container = serde_attributes(&structure.attrs);
        let fields = structure
            .fields
            .iter()
            .zip(data.fields())
            .filter_map(|(field, hir_field)| {
                let rust_name = field.ident.as_ref()?.to_string();
                let attributes = serde_attributes(&field.attrs);
                let flatten_target = attributes
                    .flatten
                    .then(|| {
                        cx.tcx
                            .type_of(hir_field.def_id)
                            .instantiate_identity()
                            .ty_adt_def()
                            .and_then(|definition| definition.did().as_local())
                    })
                    .flatten();
                Some(FieldContract {
                    serialize_name: (!attributes.skip_serialize && !attributes.flatten).then(
                        || {
                            attributes.rename_serialize.unwrap_or_else(|| {
                                apply_case(&rust_name, container.rename_all_serialize.as_deref())
                            })
                        },
                    ),
                    deserialize_name: (!attributes.skip_deserialize && !attributes.flatten).then(
                        || {
                            attributes.rename_deserialize.unwrap_or_else(|| {
                                apply_case(&rust_name, container.rename_all_deserialize.as_deref())
                            })
                        },
                    ),
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
        for structure in self.structs.values() {
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
                let collisions = collisions(structure, direction, &self.structs, &self.catalog);
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

fn collisions(
    structure: &StructContract,
    direction: SerdeDirection,
    structs: &HashMap<LocalDefId, StructContract>,
    catalog: &SerdeContractCatalog,
) -> BTreeSet<String> {
    let mut observed = BTreeSet::new();
    let mut collisions = BTreeSet::new();
    for field in &structure.fields {
        if let Some(name) = directional_name(field, direction) {
            observed.insert(name.clone());
        }
    }
    for field in &structure.fields {
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
            let Some(name) = directional_name(nested, direction) else {
                continue;
            };
            if !observed.insert(name.clone()) {
                collisions.insert(name.clone());
            }
        }
    }
    collisions
}

fn directional_name(field: &FieldContract, direction: SerdeDirection) -> Option<&String> {
    match direction {
        SerdeDirection::Serialize => field.serialize_name.as_ref(),
        SerdeDirection::Deserialize => field.deserialize_name.as_ref(),
    }
}
