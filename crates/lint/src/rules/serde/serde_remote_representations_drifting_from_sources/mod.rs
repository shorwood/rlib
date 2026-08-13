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

use super::contracts::{SerdeContractCatalog, serde_attributes};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct SourceSchema {
    fields: BTreeSet<String>,
}

struct RemoteCandidate {
    definition: LocalDefId,
    span: Span,
    source: String,
    fields: BTreeSet<String>,
}

struct Violation {
    span: Span,
    source: String,
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
struct SerdeRemoteRepresentationsDriftingFromSources {
    catalog: SerdeContractCatalog,
    sources: HashMap<String, Vec<SourceSchema>>,
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
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Struct(..)) {
            return;
        }
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
            return;
        };
        let fields = structure
            .fields
            .iter()
            .filter_map(|field| field.ident.as_ref().map(ToString::to_string))
            .collect::<BTreeSet<_>>();
        let attributes = serde_attributes(&structure.attrs);
        let Some(remote) = attributes.remote else {
            self.sources
                .entry(structure.ident.to_string())
                .or_default()
                .push(SourceSchema { fields });
            return;
        };
        if structure
            .attrs
            .iter()
            .any(|attribute| attribute.path().is_ident("doc"))
        {
            return;
        }
        self.candidates.push(RemoteCandidate {
            definition: item.owner_id.def_id,
            span: item.span,
            source: remote,
            fields,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if self
                .catalog
                .derived_type(candidate.definition, "Serialize")
                .is_none()
                && self
                    .catalog
                    .derived_type(candidate.definition, "Deserialize")
                    .is_none()
            {
                continue;
            }
            let source_name = candidate
                .source
                .rsplit("::")
                .next()
                .unwrap_or(&candidate.source);
            let Some([source]) = self.sources.get(source_name).map(Vec::as_slice) else {
                continue;
            };
            let missing = source
                .fields
                .difference(&candidate.fields)
                .map(|name| format!("`{name}`"))
                .collect::<Vec<_>>();
            if missing.is_empty() {
                continue;
            }
            Violation {
                span: candidate.span,
                source: candidate.source,
                missing,
            }
            .emit(cx);
        }
    }
}
