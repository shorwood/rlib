extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::{SerdeAttributes, SerdeContractCatalog, SerdeFlag};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Public wire names inferred from Rust identifiers
// -----------------------------------------------------------------------------

/// Public Serde declaration awaiting effective wire-name calculation.
struct Candidate {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Effective serialized names whose spelling depends on Rust identifiers.
    serialize_names: Vec<String>,
    /// Effective accepted names whose spelling depends on Rust identifiers.
    deserialize_names: Vec<String>,
}

/// Public wire contract whose spelling still depends on a Rust identifier.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Serialization directions whose wire names remain implicit.
    directions: String,
    /// Effective external names keyed by their declarations.
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
    attributes: super::utils::contracts::SerdeAttributes,
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
            let name = name.replace("r#", "");
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
        if let Ok(structure) = syn::parse_str::<syn::ItemStruct>(source) {
            let container = SerdeAttributes::from_attributes(&structure.attrs);
            let fields = structure.fields.iter().filter_map(|field| {
                field.ident.as_ref().map(|name| ImplicitMember {
                    name: name.to_string(),
                    attributes: SerdeAttributes::from_attributes(&field.attrs),
                })
            });

            let ImplicitNames {
                serialize,
                deserialize,
            } = ImplicitNames::analyze(
                fields,
                container.rename_all_serialize.as_deref(),
                container.rename_all_deserialize.as_deref(),
            );

            return Some(Self {
                is_public: matches!(structure.vis, syn::Visibility::Public(_)),
                serialize,
                deserialize,
            });
        }

        let enumeration = match syn::parse_str::<syn::ItemEnum>(source) {
            Ok(enumeration) => enumeration,
            Err(_error) => return None,
        };
        let container = SerdeAttributes::from_attributes(&enumeration.attrs);

        let mut serialize = Vec::new();
        let mut deserialize = Vec::new();
        if !container.has(SerdeFlag::Untagged) {
            let variants = enumeration.variants.iter().filter_map(|variant| {
                let attributes = SerdeAttributes::from_attributes(&variant.attrs);
                (!attributes.has(SerdeFlag::Untagged)).then(|| ImplicitMember {
                    name: variant.ident.to_string(),
                    attributes,
                })
            });
            let names = ImplicitNames::analyze(
                variants,
                container.rename_all_serialize.as_deref(),
                container.rename_all_deserialize.as_deref(),
            );
            serialize.extend(names.serialize);
            deserialize.extend(names.deserialize);
        }

        for variant in &enumeration.variants {
            let variant_attributes = SerdeAttributes::from_attributes(&variant.attrs);
            let fields = variant.fields.iter().filter_map(|field| {
                field.ident.as_ref().map(|name| ImplicitMember {
                    name: format!("{}.{}", variant.ident, name),
                    attributes: SerdeAttributes::from_attributes(&field.attrs),
                })
            });
            let names = ImplicitNames::analyze(
                fields,
                variant_attributes
                    .rename_all_serialize
                    .as_deref()
                    .or(container.rename_all_fields_serialize.as_deref()),
                variant_attributes
                    .rename_all_deserialize
                    .as_deref()
                    .or(container.rename_all_fields_deserialize.as_deref()),
            );
            serialize.extend(names.serialize);
            deserialize.extend(names.deserialize);
        }

        Some(Self {
            is_public: matches!(enumeration.vis, syn::Visibility::Public(_)),
            serialize,
            deserialize,
        })
    }
}

// -----------------------------------------------------------------------------
// SerdeUnstableImplicitWireNames: Stable explicit wire-name policy
// -----------------------------------------------------------------------------

/// Rejects externally visible wire names that change when Rust identifiers are renamed.
#[derive(Default)]
struct SerdeUnstableImplicitWireNames {
    /// Effective Serde contracts consulted after all local declarations are known.
    catalog: SerdeContractCatalog,
    /// Authored Serde declarations awaiting crate-wide contract comparison.
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
        self.catalog.check_item(cx, item);
        if item.span.from_expansion()
            || !matches!(item.kind, ItemKind::Struct(..) | ItemKind::Enum(..))
        {
            return;
        }

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

        if !is_public || (serialize_names.is_empty() && deserialize_names.is_empty()) {
            return;
        }

        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            serialize_names,
            deserialize_names,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if !cx
                .tcx
                .effective_visibilities(())
                .is_exported(candidate.definition)
            {
                continue;
            }
            let serializes = self
                .catalog
                .derived_type(candidate.definition, "Serialize")
                .is_some();

            let deserializes = self
                .catalog
                .derived_type(candidate.definition, "Deserialize")
                .is_some();
            let mut names = Vec::new();
            let mut directions = Vec::new();

            if serializes && !candidate.serialize_names.is_empty() {
                directions.push("serialization");
                names.extend(candidate.serialize_names);
            }

            if deserializes && !candidate.deserialize_names.is_empty() {
                directions.push("deserialization");
                names.extend(candidate.deserialize_names);
            }
            names.sort();
            names.dedup();

            if directions.is_empty() {
                continue;
            }

            Violation {
                span: candidate.span,
                directions: directions.join(" and "),
                names: names.into_iter().map(|name| format!("`{name}`")).collect(),
            }
            .emit(cx);
        }
    }
}
