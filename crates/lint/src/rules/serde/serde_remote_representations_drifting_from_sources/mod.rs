extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{SerdeAttributes, SerdeContractCatalog};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Carries the `SourceSchema` state used by this analysis.
struct SourceSchema {
    /// Stores the `fields` value used by this analysis.
    fields: BTreeSet<String>,
}

/// Carries the `RemoteCandidate` state used by this analysis.
struct RemoteCandidate {
    /// Stores the `definition` value used by this analysis.
    definition: LocalDefId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `source` value used by this analysis.
    source: String,
    /// Stores the `fields` value used by this analysis.
    fields: BTreeSet<String>,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `source` value used by this analysis.
    source: String,
    /// Stores the `missing` value used by this analysis.
    missing: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Serde remote representation drifts from `{}`",
            self.source
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the undocumented remote projection omits visible source {} {}",
            if self.missing.len() == 1 {
                "field"
            } else {
                "fields"
            },
            self.missing.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "mirror the missing fields, or document this as a versioned projection and its reconstruction policy",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_REMOTE_REPRESENTATIONS_DRIFTING_FROM_SOURCES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this remote schema is an implicit subset of its source",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
/// Carries the `SerdeRemoteRepresentationsDriftingFromSources` state used by this analysis.
struct SerdeRemoteRepresentationsDriftingFromSources {
    /// Stores the `catalog` value used by this analysis.
    catalog: SerdeContractCatalog,
    /// Stores the `sources` value used by this analysis.
    sources: HashMap<String, Vec<SourceSchema>>,
    /// Stores the `candidates` value used by this analysis.
    candidates: Vec<RemoteCandidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_REMOTE_REPRESENTATIONS_DRIFTING_FROM_SOURCES,
    Warn,
    "finds undocumented drift in local Serde remote representations",
    SerdeRemoteRepresentationsDriftingFromSources::default()
}

impl LateLintPass<'_> for SerdeRemoteRepresentationsDriftingFromSources {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Update the accumulated analysis state.
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Struct(..)) {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Prepare the values used by this stage.
        let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
            return;
        };

        // Prepare the values used by this stage.
        let fields = structure
            .fields
            .iter()
            .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
            .collect::<BTreeSet<_>>();
        let attributes = SerdeAttributes::analyze_serde_attributes(&structure.attrs);

        // Prepare the values used by this stage.
        let Some(remote) = attributes.remote else {
            self.sources
                .entry(structure.ident.to_string())
                .or_default()
                .push(SourceSchema { fields });
            return;
        };

        // Reject inputs that do not satisfy this stage.
        if structure
            .attrs
            .iter()
            .any(|attribute| attribute.path().is_ident("doc"))
        {
            return;
        }

        // Update the accumulated analysis state.
        self.candidates.push(RemoteCandidate {
            definition: item.owner_id.def_id,
            span: item.span,
            source: remote,
            fields,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for analyze_candidate in self.candidates.drain(..) {
            // Reject inputs that do not satisfy this stage.
            if self
                .catalog
                .derived_type(analyze_candidate.definition, "Serialize")
                .is_none()
                && self
                    .catalog
                    .derived_type(analyze_candidate.definition, "Deserialize")
                    .is_none()
            // Perform the next step of the analysis.
            {
                continue;
            }

            // Prepare the values used by this stage.
            let source_name = analyze_candidate
                .source
                .rsplit("::")
                .next()
                .unwrap_or(&analyze_candidate.source);

            // Prepare the values used by this stage.
            let Some([source]) = self.sources.get(source_name).map(Vec::as_slice) else {
                continue;
            };

            // Prepare the values used by this stage.
            let missing = source
                .fields
                .difference(&analyze_candidate.fields)
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>();

            // Reject inputs that do not satisfy this stage.
            if missing.is_empty() {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                source: analyze_candidate.source,
                missing,
            }
            .emit(cx);
        }
    }
}
