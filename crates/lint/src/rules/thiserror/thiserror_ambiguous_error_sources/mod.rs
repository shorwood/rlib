extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::{ThiserrorAttributes, ThiserrorContractCatalog};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Ambiguous primary error source
// -----------------------------------------------------------------------------

/// Error type with several plausible causes but no clear causal policy.
struct Violation {
    /// Error declaration receiving the diagnostic.
    span: Span,
    /// Plausible source fields named in the rationale.
    fields: Vec<String>,
    /// Plausible fields selected for the standard source chain.
    selected_sources: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("thiserror type stores several plausible primary sources")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        let policy = if self.selected_sources.is_empty() {
            "none is selected for the standard source chain".to_owned()
        } else {
            format!(
                "only {} enters the standard source chain",
                self.selected_sources.join(", ")
            )
        };
        Cow::Owned(format!(
            "fields {} all contain local thiserror types, but {policy}",
            self.fields.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "select and name one primary source, then classify other failures as related, suppressed, or aggregate errors",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            THISERROR_AMBIGUOUS_ERROR_SOURCES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "the causal policy among these fields is ambiguous",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// Candidate: Plausible causal field evidence
// -----------------------------------------------------------------------------

/// Field whose name and resolved type make it a plausible primary source.
struct CandidateField {
    /// Authored field name.
    name: String,
    /// Local thiserror type stored by the field.
    target: LocalDefId,
    /// Whether the field is selected for `Error::source`.
    is_primary_source: bool,
}

/// Error declaration retained until all local thiserror types are known.
struct Candidate {
    /// Candidate error definition.
    definition: LocalDefId,
    /// Error declaration receiving a later diagnostic.
    span: Span,
    /// Fields whose names suggest a causal role.
    fields: Vec<CandidateField>,
}

// -----------------------------------------------------------------------------
// ThiserrorAmbiguousErrorSources: Primary source policy
// -----------------------------------------------------------------------------

/// Correlates causal-looking fields with local derived error types.
#[derive(Default)]
struct ThiserrorAmbiguousErrorSources {
    /// Local thiserror contracts used to validate field targets.
    catalog: ThiserrorContractCatalog,
    /// Error declarations awaiting complete contract information.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_AMBIGUOUS_ERROR_SOURCES,
    Warn,
    "finds derived errors with multiple plausible causal sources",
    ThiserrorAmbiguousErrorSources::default()
}

impl ThiserrorAmbiguousErrorSources {
    /// Smallest field count that makes error-source selection ambiguous.
    const MINIMUM_CAUSAL_FIELD_CANDIDATES: usize = 2;

    /// Returns whether a field name denotes a primary causal role.
    fn has_causal_name(name: &str) -> bool {
        if ["related", "suppressed", "fallback", "retry"]
            .iter()
            .any(|role| name.contains(role))
        {
            return false;
        }
        matches!(name, "cause" | "error" | "source") || name.ends_with("_error")
    }
}
impl LateLintPass<'_> for ThiserrorAmbiguousErrorSources {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }

        let hir_fields = match item.kind {
            ItemKind::Struct(_, _, data) => data.fields().iter().collect::<Vec<_>>(),
            ItemKind::Enum(_, _, definition) => definition
                .variants
                .iter()
                .flat_map(|variant| variant.data.fields())
                .collect(),
            _ => return,
        };

        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        let syn_fields = match syn::parse_str::<syn::ItemStruct>(&source) {
            Ok(structure) => structure.fields.into_iter().collect::<Vec<_>>(),
            Err(_error) => {
                let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
                    return;
                };
                enumeration
                    .variants
                    .into_iter()
                    .flat_map(|variant| variant.fields)
                    .collect()
            }
        };

        let fields = syn_fields
            .iter()
            .zip(hir_fields)
            .filter_map(|(field, hir_field)| {
                let name = field.ident.as_ref()?.to_string();
                if !Self::has_causal_name(&name) {
                    return None;
                }
                let target = cx
                    .tcx
                    .type_of(hir_field.def_id)
                    .instantiate_identity()
                    .ty_adt_def()?
                    .did()
                    .as_local()?;
                let attributes = ThiserrorAttributes::from_attributes(&field.attrs);
                Some(CandidateField {
                    is_primary_source: attributes.is_source || name == "source",
                    name,
                    target,
                })
            })
            .collect();

        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            fields,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if self.catalog.derived_type(candidate.definition).is_none() {
                continue;
            }

            let fields = candidate
                .fields
                .into_iter()
                .filter(|field| self.catalog.derived_type(field.target).is_some())
                .collect::<Vec<_>>();

            if fields.len() < Self::MINIMUM_CAUSAL_FIELD_CANDIDATES {
                continue;
            }

            Violation {
                span: candidate.span,
                selected_sources: fields
                    .iter()
                    .filter(|field| field.is_primary_source)
                    .map(|field| format!("`{}`", field.name))
                    .collect(),
                fields: fields
                    .into_iter()
                    .map(|field| format!("`{}`", field.name))
                    .collect(),
            }
            .emit(cx);
        }
    }
}
