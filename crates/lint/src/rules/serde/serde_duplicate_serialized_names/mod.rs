extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use convert_case::{Case, Casing};
use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::def_id::LocalDefId;
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

#[derive(Clone, Copy)]
enum Direction {
    Serialize,
    Deserialize,
}

impl Direction {
    const fn label(self) -> &'static str {
        match self {
            Self::Serialize => "serialization",
            Self::Deserialize => "deserialization",
        }
    }
}

struct Candidate {
    definition: LocalDefId,
    span: Span,
    direction: Direction,
    name: String,
    members: Vec<String>,
}

struct Violation {
    span: Span,
    direction: Direction,
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
    derives: HashMap<&'static str, HashSet<LocalDefId>>,
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
        if item.span.from_expansion() {
            self.record_derive(cx, item);
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
        for direction in [Direction::Serialize, Direction::Deserialize] {
            let case = match direction {
                Direction::Serialize => container.rename_all_serialize.as_deref(),
                Direction::Deserialize => container.rename_all_deserialize.as_deref(),
            };
            let mut names: HashMap<String, Vec<String>> = HashMap::new();
            for (rust_name, attributes) in &members {
                let attributes = serde_attributes(attributes);
                if match direction {
                    Direction::Serialize => attributes.skip_serialize,
                    Direction::Deserialize => attributes.skip_deserialize,
                } {
                    continue;
                }
                let explicit = match direction {
                    Direction::Serialize => attributes.rename_serialize.as_deref(),
                    Direction::Deserialize => attributes.rename_deserialize.as_deref(),
                };
                let effective = explicit
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| apply_case(rust_name, case));
                names.entry(effective).or_default().push(rust_name.clone());
                if matches!(direction, Direction::Deserialize) {
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
                Direction::Serialize => "Serialize",
                Direction::Deserialize => "Deserialize",
            };
            if !self
                .derives
                .get(required)
                .is_some_and(|definitions| definitions.contains(&candidate.definition))
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

impl SerdeDuplicateSerializedNames {
    fn record_derive(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if !matches!(item.kind, ItemKind::Impl(_)) {
            return;
        }
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
        let Some(definition) = cx
            .tcx
            .type_of(item.owner_id)
            .instantiate_identity()
            .ty_adt_def()
            .and_then(|definition| definition.did().as_local())
        else {
            return;
        };
        self.derives.entry(derive).or_default().insert(definition);
    }
}

#[derive(Default)]
struct SerdeAttributes {
    rename_serialize: Option<String>,
    rename_deserialize: Option<String>,
    rename_all_serialize: Option<String>,
    rename_all_deserialize: Option<String>,
    aliases: Vec<String>,
    skip_serialize: bool,
    skip_deserialize: bool,
}

fn serde_attributes(attributes: &[syn::Attribute]) -> SerdeAttributes {
    let mut result = SerdeAttributes::default();
    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("serde"))
    {
        let _ = attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") || meta.path.is_ident("rename_all") {
                let rename_all = meta.path.is_ident("rename_all");
                if meta.input.peek(syn::Token![=]) {
                    let value = meta.value()?.parse::<syn::LitStr>()?.value();
                    set_directional_name(&mut result, rename_all, None, value);
                } else {
                    meta.parse_nested_meta(|direction| {
                        let value = direction.value()?.parse::<syn::LitStr>()?.value();
                        let direction = if direction.path.is_ident("serialize") {
                            Some(Direction::Serialize)
                        } else if direction.path.is_ident("deserialize") {
                            Some(Direction::Deserialize)
                        } else {
                            None
                        };
                        set_directional_name(&mut result, rename_all, direction, value);
                        Ok(())
                    })?;
                }
            } else if meta.path.is_ident("alias") {
                result.aliases.push(meta.value()?.parse::<syn::LitStr>()?.value());
            } else if meta.path.is_ident("skip") {
                result.skip_serialize = true;
                result.skip_deserialize = true;
            } else if meta.path.is_ident("skip_serializing") {
                result.skip_serialize = true;
            } else if meta.path.is_ident("skip_deserializing") {
                result.skip_deserialize = true;
            }
            Ok(())
        });
    }
    result
}

fn set_directional_name(
    attributes: &mut SerdeAttributes,
    rename_all: bool,
    direction: Option<Direction>,
    value: String,
) {
    let (serialize, deserialize) = if rename_all {
        (
            &mut attributes.rename_all_serialize,
            &mut attributes.rename_all_deserialize,
        )
    } else {
        (
            &mut attributes.rename_serialize,
            &mut attributes.rename_deserialize,
        )
    };
    match direction {
        Some(Direction::Serialize) => *serialize = Some(value),
        Some(Direction::Deserialize) => *deserialize = Some(value),
        None => {
            *serialize = Some(value.clone());
            *deserialize = Some(value);
        }
    }
}

fn apply_case(name: &str, rule: Option<&str>) -> String {
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
