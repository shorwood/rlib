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

/// One Rust declaration name and the Serde attributes governing its wire name.
struct WireMember {
    /// Authored Rust field or variant name.
    rust_name: String,
    /// Effective local Serde attributes.
    attributes: Vec<syn::Attribute>,
}

/// Named fields governed by one enum variant's rename-all contract.
struct WireFieldGroup {
    /// Variant attributes controlling its fields.
    attributes: Vec<syn::Attribute>,
    /// Named fields declared by the variant.
    members: Vec<WireMember>,
}

/// Container members and nested field groups recovered from authored syntax.
struct WireGroups {
    /// Container-level Serde attributes.
    attributes: Vec<syn::Attribute>,
    /// Direct struct fields or enum variants.
    members: Vec<WireMember>,
    /// Per-variant named field groups.
    field_groups: Vec<WireFieldGroup>,
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
    /// Recovers direct named fields from one authored struct.
    fn struct_groups(source: &str) -> Option<WireGroups> {
        // Invalid authored struct syntax cannot yield trustworthy wire-name groups.
        let structure = match syn::parse_str::<syn::ItemStruct>(source) {
            Ok(structure) => structure,
            // Without a parsed struct, field identities and attributes are unavailable.
            Err(_error) => return None,
        };
        let members = structure
            .fields
            .iter()
            .filter_map(|field| {
                Some(WireMember {
                    rust_name: field.ident.as_ref()?.to_string(),
                    attributes: field.attrs.clone(),
                })
            })
            .collect();
        Some(WireGroups {
            attributes: structure.attrs,
            members,
            field_groups: Vec::new(),
        })
    }

    /// Recovers named field groups nested under authored enum variants.
    fn enum_field_groups(enumeration: &syn::ItemEnum) -> Vec<WireFieldGroup> {
        enumeration
            .variants
            .iter()
            .filter_map(|variant| {
                let members = variant
                    .fields
                    .iter()
                    .filter_map(|field| {
                        Some(WireMember {
                            rust_name: field.ident.as_ref()?.to_string(),
                            attributes: field.attrs.clone(),
                        })
                    })
                    .collect::<Vec<_>>();
                (!members.is_empty()).then(|| WireFieldGroup {
                    attributes: variant.attrs.clone(),
                    members,
                })
            })
            .collect()
    }

    /// Recovers variants and per-variant named fields from one authored enum.
    fn enum_groups(source: &str) -> Option<WireGroups> {
        // Invalid authored enum syntax cannot yield trustworthy wire-name groups.
        let enumeration = match syn::parse_str::<syn::ItemEnum>(source) {
            Ok(enumeration) => enumeration,
            // Without a parsed enum, variant identities and attributes are unavailable.
            Err(_error) => return None,
        };
        let members = enumeration
            .variants
            .iter()
            .map(|variant| WireMember {
                rust_name: variant.ident.to_string(),
                attributes: variant.attrs.clone(),
            })
            .collect();
        Some(WireGroups {
            field_groups: Self::enum_field_groups(&enumeration),
            attributes: enumeration.attrs,
            members,
        })
    }

    /// Records wire-name collisions for both serialization directions.
    fn record_group(
        &mut self,
        definition: LocalDefId,
        span: Span,
        cases: [Option<&str>; 2],
        members: &[WireMember],
    ) {
        for (direction, case) in [SerdeDirection::Serialize, SerdeDirection::Deserialize]
            .into_iter()
            .zip(cases)
        {
            let mut names: HashMap<String, Vec<String>> = HashMap::new();
            for member in members {
                let rust_name = &member.rust_name;
                let attributes = SerdeAttributes::from_attributes(&member.attributes);
                if match direction {
                    SerdeDirection::Serialize => attributes.has_flag(SerdeFlag::SkipSerialize),
                    SerdeDirection::Deserialize => attributes.has_flag(SerdeFlag::SkipDeserialize),
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

                if !matches!(direction, SerdeDirection::Deserialize) {
                    continue;
                }
                for alias in attributes.aliases {
                    names.entry(alias).or_default().push(rust_name.clone());
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

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_DUPLICATE_SERIALIZED_NAMES,
    Warn,
    "finds colliding effective Serde field and variant names",
    SerdeDuplicateSerializedNames::default()
}

impl LateLintPass<'_> for SerdeDuplicateSerializedNames {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);

        // Generated declarations do not define authored Serde wire-name policy.
        if item.span.from_expansion() {
            return;
        }

        // Missing authored source prevents recovery of direction-specific Serde attributes.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        let WireGroups {
            attributes,
            members,
            field_groups,
        } = match item.kind {
            ItemKind::Struct(..) => {
                // Unparseable struct source cannot produce reliable field wire groups.
                let Some(groups) = Self::struct_groups(&source) else {
                    return;
                };
                groups
            }
            ItemKind::Enum(..) => {
                // Unparseable enum source cannot produce reliable variant wire groups.
                let Some(groups) = Self::enum_groups(&source) else {
                    return;
                };
                groups
            }
            // Other item kinds do not define struct-field or enum-variant wire groups.
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
        for WireFieldGroup {
            attributes,
            members,
        } in field_groups
        {
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
