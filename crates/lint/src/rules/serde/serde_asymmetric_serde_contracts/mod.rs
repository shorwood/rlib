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
    /// Stores the `declaration` value used by this analysis.
    declaration: String,
    /// Stores the `serialize` value used by this analysis.
    serialize: String,
    /// Stores the `deserialize` value used by this analysis.
    deserialize: String,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `declaration` value used by this analysis.
    declaration: String,
    /// Stores the `serialize` value used by this analysis.
    serialize: String,
    /// Stores the `deserialize` value used by this analysis.
    deserialize: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Serde names for `{}` differ by direction",
            self.declaration
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "serialization writes `{}` while deserialization accepts `{}` as the primary name, without an authored compatibility explanation",
            self.serialize, self.deserialize
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use one stable name or document the intentional one-way schema migration at this declaration",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_ASYMMETRIC_SERDE_CONTRACTS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this declaration has directional wire names");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
/// Carries the `SerdeAsymmetricSerdeContracts` state used by this analysis.
struct SerdeAsymmetricSerdeContracts {
    /// Stores the `catalog` value used by this analysis.
    catalog: SerdeContractCatalog,
    /// Stores the `candidates` value used by this analysis.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_ASYMMETRIC_SERDE_CONTRACTS,
    Warn,
    "finds unexplained directional Serde naming contracts",
    SerdeAsymmetricSerdeContracts::default()
}

impl LateLintPass<'_> for SerdeAsymmetricSerdeContracts {
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
        let members = match item.kind {
            ItemKind::Struct(..) => {
                // Prepare the values used by this stage.
                let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
                    return;
                };

                // Perform the next step of the analysis.
                structure
                    .fields
                    .iter()
                    .filter_map(|field| {
                        Some((field.ident.as_ref()?.to_string(), field.attrs.clone()))
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
                    .map(|variant| (variant.ident.to_string(), variant.attrs.clone()))
                    .collect::<Vec<_>>()
            }
            _ => return,
        };

        // Process the candidates handled by this stage.
        for (declaration, attributes) in members {
            // Reject inputs that do not satisfy this stage.
            if attributes
                .iter()
                .any(|attribute| attribute.path().is_ident("doc"))
            {
                continue;
            }
            let attributes = SerdeAttributes::analyze_serde_attributes(&attributes);

            // Prepare the values used by this stage.
            let (Some(serialize), Some(deserialize)) =
                (attributes.rename_serialize, attributes.rename_deserialize)
            else {
                continue;
            };

            // Reject inputs that do not satisfy this stage.
            if serialize == deserialize {
                continue;
            }

            // Update the accumulated analysis state.
            self.candidates.push(Candidate {
                definition: item.owner_id.def_id,
                span: item.span,
                declaration,
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
                declaration: analyze_candidate.declaration,
                serialize: analyze_candidate.serialize,
                deserialize: analyze_candidate.deserialize,
            }
            .emit(cx);
        }
    }
}
