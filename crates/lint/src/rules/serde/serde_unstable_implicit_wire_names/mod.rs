extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{SerdeAttributes, SerdeContractCatalog, SerdeFlag};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Carries the `Candidate` state used by this analysis.
struct Candidate {
    /// Stores the `definition` value used by this analysis.
    definition: LocalDefId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `serialize_names` value used by this analysis.
    serialize_names: Vec<String>,
    /// Stores the `deserialize_names` value used by this analysis.
    deserialize_names: Vec<String>,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `directions` value used by this analysis.
    directions: String,
    /// Stores the `names` value used by this analysis.
    names: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "public Serde {} contract relies on implicit wire names",
            self.directions
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "renaming the Rust identifiers {} would silently change externally observed data",
            self.names.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "declare `#[serde(rename_all = \"...\")]` or explicit member renames for each wire direction",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_UNSTABLE_IMPLICIT_WIRE_NAMES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this public contract inherits Rust identifiers");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// One named Serde member and its authored attributes.
struct ImplicitMember {
    /// Rust member name.
    name: String,
    /// Parsed Serde attributes for the member.
    attributes: super::contracts::SerdeAttributes,
}

/// Implicit names retained for each wire direction.
struct ImplicitNames {
    /// Names inherited by serialization.
    serialize: Vec<String>,
    /// Names inherited by deserialization.
    deserialize: Vec<String>,
}

impl ImplicitNames {
    /// Finds member names left implicit in each wire direction.
    fn analyze(
        members: impl Iterator<Item = ImplicitMember>,
        serialize_rule: Option<&str>,
        deserialize_rule: Option<&str>,
    ) -> Self {
        let mut serialize = Vec::new();
        let mut deserialize = Vec::new();
        for ImplicitMember { name, attributes } in members {
            if serialize_rule.is_none()
                && attributes.rename_serialize.is_none()
                && !attributes.has(SerdeFlag::SkipSerialize)
            {
                serialize.push(name.clone());
            }
            if !(deserialize_rule.is_none()
                && attributes.rename_deserialize.is_none()
                && !attributes.has(SerdeFlag::SkipDeserialize))
            {
                continue;
            }
            deserialize.push(name);
        }
        Self {
            serialize,
            deserialize,
        }
    }
}

/// Visibility and implicit wire names recovered from one Serde contract.
struct ContractNames {
    /// Whether the contract is public.
    is_public: bool,
    /// Names inherited by serialization.
    serialize: Vec<String>,
    /// Names inherited by deserialization.
    deserialize: Vec<String>,
}

impl ContractNames {
    /// Recovers visibility and implicit names from a Serde struct or enum.
    fn analyze(source: &str) -> Option<Self> {
        // Reject inputs that do not satisfy this stage.
        if let Ok(structure) = syn::parse_str::<syn::ItemStruct>(source) {
            // Prepare the values used by this stage.
            let container = SerdeAttributes::analyze_serde_attributes(&structure.attrs);
            let fields = structure.fields.iter().filter_map(|field| {
                field.ident.as_ref().map(|name| ImplicitMember {
                    name: name.to_string(),
                    attributes: SerdeAttributes::analyze_serde_attributes(&field.attrs),
                })
            });

            // Prepare the values used by this stage.
            let ImplicitNames {
                serialize,
                deserialize,
            } = ImplicitNames::analyze(
                fields,
                container.rename_all_serialize.as_deref(),
                container.rename_all_deserialize.as_deref(),
            );

            // Return the completed analysis result.
            return Some(Self {
                is_public: matches!(structure.vis, syn::Visibility::Public(_)),
                serialize,
                deserialize,
            });
        }

        // Prepare the values used by this stage.
        let enumeration = match syn::parse_str::<syn::ItemEnum>(source) {
            Ok(enumeration) => enumeration,
            Err(_error) => return None,
        };
        let container = SerdeAttributes::analyze_serde_attributes(&enumeration.attrs);

        // Reject inputs that do not satisfy this stage.
        if container.has(SerdeFlag::Untagged) {
            return None;
        }
        let variants = enumeration.variants.iter().map(|variant| ImplicitMember {
            name: variant.ident.to_string(),
            attributes: SerdeAttributes::analyze_serde_attributes(&variant.attrs),
        });

        // Prepare the values used by this stage.
        let ImplicitNames {
            serialize,
            deserialize,
        } = ImplicitNames::analyze(
            variants,
            container.rename_all_serialize.as_deref(),
            container.rename_all_deserialize.as_deref(),
        );

        // Return the completed analysis result.
        Some(Self {
            is_public: matches!(enumeration.vis, syn::Visibility::Public(_)),
            serialize,
            deserialize,
        })
    }
}

#[derive(Default)]
/// Carries the `SerdeUnstableImplicitWireNames` state used by this analysis.
struct SerdeUnstableImplicitWireNames {
    /// Stores the `catalog` value used by this analysis.
    catalog: SerdeContractCatalog,
    /// Stores the `candidates` value used by this analysis.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_UNSTABLE_IMPLICIT_WIRE_NAMES,
    Warn,
    "finds public Serde contracts with implicit wire names",
    SerdeUnstableImplicitWireNames::default()
}

impl LateLintPass<'_> for SerdeUnstableImplicitWireNames {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Update the accumulated analysis state.
        self.catalog.check_item(cx, item);
        if item.span.from_expansion()
            || !matches!(item.kind, ItemKind::Struct(..) | ItemKind::Enum(..))
        {
            return;
        }

        // Prepare the values used by this stage.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };
        let Some(ContractNames {
            is_public,
            serialize: serialize_names,
            deserialize: deserialize_names,
        }) = ContractNames::analyze(&source)
        else {
            return;
        };

        // Reject inputs that do not satisfy this stage.
        if !is_public || (serialize_names.is_empty() && deserialize_names.is_empty()) {
            return;
        }

        // Update the accumulated analysis state.
        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            serialize_names,
            deserialize_names,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for analyze_candidate in self.candidates.drain(..) {
            // Prepare the values used by this stage.
            let serializes = self
                .catalog
                .derived_type(analyze_candidate.definition, "Serialize")
                .is_some();

            // Prepare the values used by this stage.
            let deserializes = self
                .catalog
                .derived_type(analyze_candidate.definition, "Deserialize")
                .is_some();
            let mut names = Vec::new();
            let mut directions = Vec::new();

            // Reject inputs that do not satisfy this stage.
            if serializes && !analyze_candidate.serialize_names.is_empty() {
                directions.push("serialization");
                names.extend(analyze_candidate.serialize_names);
            }

            // Reject inputs that do not satisfy this stage.
            if deserializes && !analyze_candidate.deserialize_names.is_empty() {
                directions.push("deserialization");
                names.extend(analyze_candidate.deserialize_names);
            }
            names.sort();
            names.dedup();

            // Reject inputs that do not satisfy this stage.
            if directions.is_empty() {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                directions: directions.join(" and "),
                names: names.into_iter().map(|name| format!("`{name}`")).collect(),
            }
            .emit(cx);
        }
    }
}
