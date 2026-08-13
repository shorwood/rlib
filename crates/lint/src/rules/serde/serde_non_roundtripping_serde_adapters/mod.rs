extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{SerdeAttributes, SerdeContractCatalog};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Carries the `Candidate` state used by this analysis.
struct Candidate {
    /// Stores the `definition` value used by this analysis.
    definition: LocalDefId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `field` value used by this analysis.
    field: String,
    /// Stores the `serialize` value used by this analysis.
    serialize: String,
    /// Stores the `deserialize` value used by this analysis.
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
        let name = path.rsplit("::").next()?;
        Self::CONTRACTS.iter().find_map(|&contract| {
            name.strip_suffix(contract).map(|family| Self {
                family: family.trim_end_matches('_').to_owned(),
                contract,
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

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `field` value used by this analysis.
    field: String,
    /// Stores the `serialize` value used by this analysis.
    serialize: String,
    /// Stores the `deserialize` value used by this analysis.
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
/// Carries the `SerdeNonRoundtrippingSerdeAdapters` state used by this analysis.
struct SerdeNonRoundtrippingSerdeAdapters {
    /// Stores the `catalog` value used by this analysis.
    catalog: SerdeContractCatalog,
    /// Stores the `candidates` value used by this analysis.
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
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Struct(..)) {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };
        let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
            return;
        };
        for field in &structure.fields {
            // Prepare the values used by this stage.
            let Some(name) = field.ident.as_ref() else {
                continue;
            };
            let attributes = SerdeAttributes::analyze_serde_attributes(&field.attrs);
            let (Some(serialize), Some(deserialize)) =
                (attributes.serialize_with, attributes.deserialize_with)
            // Perform the next step of the analysis.
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

            // Update the accumulated analysis state.
            self.candidates.push(Candidate {
                definition: item.owner_id.def_id,
                span: item.span,
                field: name.to_string(),
                serialize,
                deserialize,
            });
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for analyze_candidate in self.candidates.drain(..) {
            // Reject inputs that do not satisfy this stage.
            if self
                .catalog
                .derived_type(analyze_candidate.definition, "Serialize")
                .is_none()
                || self
                    .catalog
                    .derived_type(analyze_candidate.definition, "Deserialize")
                    .is_none()
            // Perform the next step of the analysis.
            {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                field: analyze_candidate.field,
                serialize: analyze_candidate.serialize,
                deserialize: analyze_candidate.deserialize,
            }
            .emit(cx);
        }
    }
}
