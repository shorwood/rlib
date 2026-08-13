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

use super::contracts::{
    SerdeAttributes, SerdeCase, SerdeContractCatalog, SerdeDirection, SerdeFlag,
};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Carries the `Candidate` state used by this analysis.
struct Candidate {
    /// Stores the `definition` value used by this analysis.
    definition: LocalDefId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `direction` value used by this analysis.
    direction: SerdeDirection,
    /// Stores the `name` value used by this analysis.
    name: String,
    /// Stores the `members` value used by this analysis.
    members: Vec<String>,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `direction` value used by this analysis.
    direction: SerdeDirection,
    /// Stores the `name` value used by this analysis.
    name: String,
    /// Stores the `members` value used by this analysis.
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
/// Carries the `SerdeDuplicateSerializedNames` state used by this analysis.
struct SerdeDuplicateSerializedNames {
    /// Stores the `catalog` value used by this analysis.
    catalog: SerdeContractCatalog,
    /// Stores the `candidates` value used by this analysis.
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
        // Update the accumulated analysis state.
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Prepare the values used by this stage.
        let (attributes, members) = match item.kind {
            ItemKind::Struct(..) => {
                // Prepare the values used by this stage.
                let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
                    return;
                };

                // Perform the next step of the analysis.
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
                // Prepare the values used by this stage.
                let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
                    return;
                };

                // Perform the next step of the analysis.
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

        // Prepare the values used by this stage.
        let container = SerdeAttributes::analyze_serde_attributes(&attributes);

        // Process the candidates handled by this stage.
        for direction in [SerdeDirection::Serialize, SerdeDirection::Deserialize] {
            let case = match direction {
                SerdeDirection::Serialize => container.rename_all_serialize.as_deref(),
                SerdeDirection::Deserialize => container.rename_all_deserialize.as_deref(),
            };
            let mut names: HashMap<String, Vec<String>> = HashMap::new();
            for (rust_name, attributes) in &members {
                // Prepare the values used by this stage.
                let attributes = SerdeAttributes::analyze_serde_attributes(attributes);
                if match direction {
                    SerdeDirection::Serialize => attributes.has(SerdeFlag::SkipSerialize),
                    SerdeDirection::Deserialize => attributes.has(SerdeFlag::SkipDeserialize),
                } {
                    continue;
                }

                // Prepare the values used by this stage.
                let explicit = match direction {
                    SerdeDirection::Serialize => attributes.rename_serialize.as_deref(),
                    SerdeDirection::Deserialize => attributes.rename_deserialize.as_deref(),
                };
                let effective =
                    explicit.map_or_else(|| SerdeCase::apply(rust_name, case), ToOwned::to_owned);
                names.entry(effective).or_default().push(rust_name.clone());

                // Reject inputs that do not satisfy this stage.
                if !(matches!(direction, SerdeDirection::Deserialize)) {
                    continue;
                }
                for alias in attributes.aliases {
                    names.entry(alias).or_default().push(rust_name.clone());
                }
            }
            for (name, members) in names {
                // Prepare the values used by this stage.
                let distinct = members.iter().collect::<HashSet<_>>();
                if distinct.len() <= 1 {
                    continue;
                }

                // Update the accumulated analysis state.
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

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for analyze_candidate in self.candidates.drain(..) {
            // Prepare the values used by this stage.
            let required = match analyze_candidate.direction {
                SerdeDirection::Serialize => "Serialize",
                SerdeDirection::Deserialize => "Deserialize",
            };

            // Reject inputs that do not satisfy this stage.
            if self
                .catalog
                .derived_type(analyze_candidate.definition, required)
                .is_none()
            {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                direction: analyze_candidate.direction,
                name: analyze_candidate.name,
                members: analyze_candidate.members,
            }
            .emit(cx);
        }
    }
}
