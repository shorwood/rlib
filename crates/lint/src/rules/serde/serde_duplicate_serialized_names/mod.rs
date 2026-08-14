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

use super::utils::contracts::{
    SerdeAttributes, SerdeCase, SerdeContractCatalog, SerdeDirection, SerdeFlag,
};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Duplicate serialized member names
// -----------------------------------------------------------------------------

/// Serde container awaiting effective member-name comparison.
struct Candidate {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Serialization direction in which the behavior applies.
    direction: SerdeDirection,
    /// Authored type or member name involved in the wire contract.
    name: String,
    /// Member declarations participating in the contract.
    members: Vec<String>,
}

/// Two members that resolve to the same serialized wire name.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Serialization direction in which the behavior applies.
    direction: SerdeDirection,
    /// Authored type or member name involved in the wire contract.
    name: String,
    /// Member declarations participating in the contract.
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

// -----------------------------------------------------------------------------
// SerdeDuplicateSerializedNames: Unique wire-name policy
// -----------------------------------------------------------------------------

/// Finds fields or variants that resolve to the same serialized wire name.
#[derive(Default)]
struct SerdeDuplicateSerializedNames {
    /// Effective Serde contracts consulted after all local declarations are known.
    catalog: SerdeContractCatalog,
    /// Authored Serde declarations awaiting crate-wide contract comparison.
    candidates: Vec<Candidate>,
}

impl SerdeDuplicateSerializedNames {
    fn record_group(
        &mut self,
        definition: LocalDefId,
        span: Span,
        cases: [Option<&str>; 2],
        members: &[(String, Vec<syn::Attribute>)],
    ) {
        for (direction, case) in [SerdeDirection::Serialize, SerdeDirection::Deserialize]
            .into_iter()
            .zip(cases)
        {
            let mut names: HashMap<String, Vec<String>> = HashMap::new();
            for (rust_name, attributes) in members {
                let attributes = SerdeAttributes::from_attributes(attributes);
                if match direction {
                    SerdeDirection::Serialize => attributes.has(SerdeFlag::SkipSerialize),
                    SerdeDirection::Deserialize => attributes.has(SerdeFlag::SkipDeserialize),
                } {
                    continue;
                }

                let explicit = match direction {
                    SerdeDirection::Serialize => attributes.rename_serialize.as_deref(),
                    SerdeDirection::Deserialize => attributes.rename_deserialize.as_deref(),
                };
                let effective =
                    explicit.map_or_else(|| SerdeCase::apply(rust_name, case), ToOwned::to_owned);
                names.entry(effective).or_default().push(rust_name.clone());

                if matches!(direction, SerdeDirection::Deserialize) {
                    for alias in attributes.aliases {
                        names.entry(alias).or_default().push(rust_name.clone());
                    }
                }
            }

            let mut collisions = names
                .into_iter()
                .filter(|(_, members)| members.iter().collect::<HashSet<_>>().len() > 1)
                .collect::<Vec<_>>();
            collisions.sort_by(|(left, _), (right, _)| left.cmp(right));
            self.candidates
                .extend(collisions.into_iter().map(|(name, members)| Candidate {
                    definition,
                    span,
                    direction,
                    name,
                    members,
                }));
        }
    }
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
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        let (attributes, members, field_groups) = match item.kind {
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
                    Vec::new(),
                )
            }
            ItemKind::Enum(..) => {
                let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
                    return;
                };

                let field_groups = enumeration
                    .variants
                    .iter()
                    .filter_map(|variant| {
                        let members = variant
                            .fields
                            .iter()
                            .filter_map(|field| {
                                Some((field.ident.as_ref()?.to_string(), field.attrs.clone()))
                            })
                            .collect::<Vec<_>>();
                        (!members.is_empty()).then(|| (variant.attrs.clone(), members))
                    })
                    .collect::<Vec<_>>();
                (
                    enumeration.attrs,
                    enumeration
                        .variants
                        .iter()
                        .map(|variant| (variant.ident.to_string(), variant.attrs.clone()))
                        .collect::<Vec<_>>(),
                    field_groups,
                )
            }
            _ => return,
        };

        let container = SerdeAttributes::from_attributes(&attributes);

        self.record_group(
            item.owner_id.def_id,
            item.span,
            [
                container.rename_all_serialize.as_deref(),
                container.rename_all_deserialize.as_deref(),
            ],
            &members,
        );
        for (attributes, members) in field_groups {
            let variant = SerdeAttributes::from_attributes(&attributes);
            self.record_group(
                item.owner_id.def_id,
                item.span,
                [
                    variant
                        .rename_all_serialize
                        .as_deref()
                        .or(container.rename_all_fields_serialize.as_deref()),
                    variant
                        .rename_all_deserialize
                        .as_deref()
                        .or(container.rename_all_fields_deserialize.as_deref()),
                ],
                &members,
            );
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
