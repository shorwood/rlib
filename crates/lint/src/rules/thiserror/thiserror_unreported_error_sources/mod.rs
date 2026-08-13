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
// Violation: Sole nested cause omitted from the source chain
// -----------------------------------------------------------------------------

/// Derived error with one provable local cause that `Error::source` omits.
struct Violation {
    /// Error declaration receiving the diagnostic.
    span: Span,
    /// Sole causal field named in the diagnostic.
    field: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "nested error `{}` is omitted from the thiserror source chain",
            self.field
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "this is the sole causal-looking field with a local thiserror type, but `Error::source` will not expose it",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "mark the field `#[source]`, or use `#[from]` when automatic conversion is intended",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            THISERROR_UNREPORTED_ERROR_SOURCES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this derived error drops a provable cause from its chain",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// Candidate: Causal field evidence
// -----------------------------------------------------------------------------

/// Causal-looking field and the local type it stores.
struct CandidateField {
    /// Authored field name.
    name: String,
    /// Local error type stored by the field.
    target: LocalDefId,
    /// Whether thiserror places the field in the source chain.
    is_marked_source: bool,
}

/// Error declaration retained until all local thiserror contracts are known.
struct Candidate {
    /// Candidate error definition.
    definition: LocalDefId,
    /// Error declaration receiving a later diagnostic.
    span: Span,
    /// Fields whose names suggest a causal role.
    fields: Vec<CandidateField>,
}

// -----------------------------------------------------------------------------
// ThiserrorUnreportedErrorSources: Complete source-chain policy
// -----------------------------------------------------------------------------

/// Correlates causal-looking fields with local derived error contracts.
#[derive(Default)]
struct ThiserrorUnreportedErrorSources {
    /// Local derived error contracts used to validate nested field types.
    catalog: ThiserrorContractCatalog,
    /// Error declarations awaiting complete type information.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_UNREPORTED_ERROR_SOURCES,
    Warn,
    "finds provable nested thiserror causes omitted from source chains",
    ThiserrorUnreportedErrorSources::default()
}

impl ThiserrorUnreportedErrorSources {
    /// Returns whether a field name conventionally denotes a cause.
    fn has_causal_name(name: &str) -> bool {
        matches!(name, "cause" | "error" | "source") || name.ends_with("_error")
    }
}
impl LateLintPass<'_> for ThiserrorUnreportedErrorSources {
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
                    is_marked_source: attributes.is_source || name == "source",
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

            let nested = candidate
                .fields
                .into_iter()
                .filter(|field| self.catalog.derived_type(field.target).is_some())
                .collect::<Vec<_>>();

            if nested.iter().any(|field| field.is_marked_source) || nested.len() != 1 {
                continue;
            }

            Violation {
                span: candidate.span,
                field: nested[0].name.clone(),
            }
            .emit(cx);
        }
    }
}
