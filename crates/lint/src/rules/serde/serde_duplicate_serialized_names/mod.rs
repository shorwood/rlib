extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{SerdeContractCatalog, SerdeDirection, apply_case, serde_attributes};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct Candidate {
    definition: LocalDefId,
    span: Span,
    direction: SerdeDirection,
    name: String,
    members: Vec<String>,
}

struct Violation {
    span: Span,
    direction: SerdeDirection,
    name: String,
    members: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Serde {} name `{}` is duplicated",
            self.direction.label(),
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the effective name is shared by {}; the wire representation cannot identify one member unambiguously",
            self.members.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("assign distinct Serde names or aliases for this direction")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_DUPLICATE_SERIALIZED_NAMES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "these declarations share an effective wire name");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct SerdeDuplicateSerializedNames {
    catalog: SerdeContractCatalog,
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_DUPLICATE_SERIALIZED_NAMES,
    Warn,
    "finds colliding effective Serde field and variant names",
    SerdeDuplicateSerializedNames::default()
}

impl LateLintPass<'_> for SerdeDuplicateSerializedNames {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let (attributes, members) = match item.kind {
            ItemKind::Struct(..) => {
                let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
                    return;
                };
                (
                    structure.attrs,
                    structure
                        .fields
                        .iter()
                        .filter_map(|field| {
                            Some((field.ident.as_ref()?.to_string(), field.attrs.clone()))
                        })
                        .collect::<Vec<_>>(),
                )
            }
            ItemKind::Enum(..) => {
                let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
                    return;
                };
                (
                    enumeration.attrs,
                    enumeration
                        .variants
                        .iter()
                        .map(|variant| (variant.ident.to_string(), variant.attrs.clone()))
                        .collect::<Vec<_>>(),
                )
            }
            _ => return,
        };
        let container = serde_attributes(&attributes);
        for direction in [SerdeDirection::Serialize, SerdeDirection::Deserialize] {
            let case = match direction {
                SerdeDirection::Serialize => container.rename_all_serialize.as_deref(),
                SerdeDirection::Deserialize => container.rename_all_deserialize.as_deref(),
            };
            let mut names: HashMap<String, Vec<String>> = HashMap::new();
            for (rust_name, attributes) in &members {
                let attributes = serde_attributes(attributes);
                if match direction {
                    SerdeDirection::Serialize => attributes.skip_serialize,
                    SerdeDirection::Deserialize => attributes.skip_deserialize,
                } {
                    continue;
                }
                let explicit = match direction {
                    SerdeDirection::Serialize => attributes.rename_serialize.as_deref(),
                    SerdeDirection::Deserialize => attributes.rename_deserialize.as_deref(),
                };
                let effective = explicit
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| apply_case(rust_name, case));
                names.entry(effective).or_default().push(rust_name.clone());
                if matches!(direction, SerdeDirection::Deserialize) {
                    for alias in attributes.aliases {
                        names.entry(alias).or_default().push(rust_name.clone());
                    }
                }
            }
            for (name, members) in names {
                let distinct = members.iter().collect::<HashSet<_>>();
                if distinct.len() > 1 {
                    self.candidates.push(Candidate {
                        definition: item.owner_id.def_id,
                        span: item.span,
                        direction,
                        name,
                        members,
                    });
                }
            }
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            let required = match candidate.direction {
                SerdeDirection::Serialize => "Serialize",
                SerdeDirection::Deserialize => "Deserialize",
            };
            if self
                .catalog
                .derived_type(candidate.definition, required)
                .is_none()
            {
                continue;
            }
            Violation {
                span: candidate.span,
                direction: candidate.direction,
                name: candidate.name,
                members: candidate.members,
            }
            .emit(cx);
        }
    }
}
