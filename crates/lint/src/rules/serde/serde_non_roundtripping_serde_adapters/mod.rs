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
// Violation: Adapter pair that cannot round-trip
// -----------------------------------------------------------------------------

/// Adapted field awaiting comparison of its read and write functions.
struct Candidate {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Field name quoted in the diagnostic.
    field: String,
    /// Effective name written during serialization.
    serialize: String,
    /// Primary name accepted during deserialization.
    deserialize: String,
}

/// Separates an adapter family from its unit or encoding contract.
struct NamedContract {
    /// Shared adapter family name.
    family: String,
    /// Unit or encoding selected by the adapter.
    contract: &'static str,
}

impl NamedContract {
    /// Adapter contract suffixes understood by this analysis.
    const CONTRACTS: &[&str] = &[
        "milliseconds",
        "microseconds",
        "nanoseconds",
        "seconds",
        "base64",
        "hex",
    ];

    /// Extracts a known unit or encoding contract from an adapter path.
    fn from_path(path: &str) -> Option<Self> {
        path.rsplit("::").find_map(|name| {
            Self::CONTRACTS.iter().find_map(|&contract| {
                let parts = name.split('_').collect::<Vec<_>>();
                parts.contains(&contract).then(|| {
                    let family = parts
                        .into_iter()
                        .filter(|part| {
                            *part != contract
                                && !matches!(
                                    *part,
                                    "adapter"
                                        | "helper"
                                        | "as"
                                        | "decode"
                                        | "deserialize"
                                        | "encode"
                                        | "from"
                                        | "serialize"
                                        | "to"
                                )
                        })
                        .collect::<Vec<_>>()
                        .join("_");
                    Self { family, contract }
                })
            })
        })
    }
}

/// Names the directional adapters whose contracts must agree.
struct AdapterPair<'path> {
    /// Serialization adapter path.
    serialize: &'path str,
    /// Deserialization adapter path.
    deserialize: &'path str,
}

impl AdapterPair<'_> {
    /// Returns whether the directional adapter contracts provably disagree.
    fn is_provably_incompatible(&self) -> bool {
        let Some(serialize) = NamedContract::from_path(self.serialize) else {
            return false;
        };
        let Some(deserialize) = NamedContract::from_path(self.deserialize) else {
            return false;
        };
        serialize.family == deserialize.family && serialize.contract != deserialize.contract
    }
}

/// Field whose serialization and deserialization adapters do not form one contract.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Field name quoted in the diagnostic.
    field: String,
    /// Effective name written during serialization.
    serialize: String,
    /// Primary name accepted during deserialization.
    deserialize: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Serde adapters for `{}` do not round-trip",
            self.field
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "serialization uses `{}` while deserialization uses the incompatible `{}` contract",
            self.serialize, self.deserialize
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("pair helpers with the same unit or encoding, or use one `with` module")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_NON_ROUNDTRIPPING_SERDE_ADAPTERS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "these directional adapters disagree");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
// -----------------------------------------------------------------------------
// SerdeNonRoundtrippingSerdeAdapters: Symmetric adapter policy
// -----------------------------------------------------------------------------

/// Rejects mismatched serialization and deserialization adapters on one value.
struct SerdeNonRoundtrippingSerdeAdapters {
    /// Effective Serde contracts consulted after all local declarations are known.
    catalog: SerdeContractCatalog,
    /// Authored Serde declarations awaiting crate-wide contract comparison.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_NON_ROUNDTRIPPING_SERDE_ADAPTERS,
    Warn,
    "finds provably incompatible directional Serde adapters",
    SerdeNonRoundtrippingSerdeAdapters::default()
}

impl LateLintPass<'_> for SerdeNonRoundtrippingSerdeAdapters {
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
        let fields = match item.kind {
            ItemKind::Struct(..) => {
                let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
                    return;
                };
                structure
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        (
                            field
                                .ident
                                .as_ref()
                                .map_or_else(|| format!("field {index}"), ToString::to_string),
                            field.attrs.clone(),
                        )
                    })
                    .collect::<Vec<_>>()
            }
            ItemKind::Enum(..) => {
                let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
                    return;
                };
                enumeration
                    .variants
                    .iter()
                    .flat_map(|variant| {
                        variant
                            .fields
                            .iter()
                            .enumerate()
                            .map(move |(index, field)| {
                                let field_name = field
                                    .ident
                                    .as_ref()
                                    .map_or_else(|| index.to_string(), ToString::to_string);
                                (
                                    format!("{}.{field_name}", variant.ident),
                                    field.attrs.clone(),
                                )
                            })
                    })
                    .collect::<Vec<_>>()
            }
            _ => return,
        };
        for (field, authored_attributes) in fields {
            let attributes = SerdeAttributes::from_attributes(&authored_attributes);
            if attributes.has(SerdeFlag::SkipSerialize)
                || attributes.has(SerdeFlag::SkipDeserialize)
            {
                continue;
            }
            let (Some(serialize), Some(deserialize)) =
                (attributes.serialize_with, attributes.deserialize_with)
            else {
                continue;
            };
            if !(AdapterPair {
                serialize: &serialize,
                deserialize: &deserialize,
            })
            .is_provably_incompatible()
            {
                continue;
            }

            self.candidates.push(Candidate {
                definition: item.owner_id.def_id,
                span: item.span,
                field,
                serialize,
                deserialize,
            });
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if self
                .catalog
                .derived_type(candidate.definition, "Serialize")
                .is_none()
                || self
                    .catalog
                    .derived_type(candidate.definition, "Deserialize")
                    .is_none()
            {
                continue;
            }

            Violation {
                span: candidate.span,
                field: candidate.field,
                serialize: candidate.serialize,
                deserialize: candidate.deserialize,
            }
            .emit(cx);
        }
    }
}
