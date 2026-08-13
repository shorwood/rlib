extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{ThiserrorAttributes, ThiserrorContractCatalog};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Carries the `FieldCandidate` state used by this analysis.
struct FieldCandidate {
    /// Stores the `name` value used by this analysis.
    name: String,
    /// Stores the `target` value used by this analysis.
    target: LocalDefId,
    /// Stores the `is_marked_source` value used by this analysis.
    is_marked_source: bool,
}

/// Carries the `Candidate` state used by this analysis.
struct Candidate {
    /// Stores the `definition` value used by this analysis.
    definition: LocalDefId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `fields` value used by this analysis.
    fields: Vec<FieldCandidate>,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `field` value used by this analysis.
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

/// Performs the `causal_name` step of the lint analysis.
fn causal_name(name: &str) -> bool {
    matches!(name, "cause" | "error" | "source") || name.ends_with("_error")
}

#[derive(Default)]
/// Carries the `ThiserrorUnreportedErrorSources` state used by this analysis.
struct ThiserrorUnreportedErrorSources {
    /// Stores the `catalog` value used by this analysis.
    catalog: ThiserrorContractCatalog,
    /// Stores the `candidates` value used by this analysis.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_UNREPORTED_ERROR_SOURCES,
    Warn,
    "finds provable nested thiserror causes omitted from source chains",
    ThiserrorUnreportedErrorSources::default()
}

impl LateLintPass<'_> for ThiserrorUnreportedErrorSources {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Update the accumulated analysis state.
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            return;
        }

        // Prepare the values used by this stage.
        let hir_fields = match item.kind {
            ItemKind::Struct(_, _, data) => data.fields().iter().collect::<Vec<_>>(),
            ItemKind::Enum(_, _, definition) => definition
                .variants
                .iter()
                .flat_map(|variant| variant.data.fields())
                .collect(),
            _ => return,
        };

        // Prepare the values used by this stage.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Prepare the values used by this stage.
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

        // Prepare the values used by this stage.
        let fields = syn_fields
            .iter()
            .zip(hir_fields)
            .filter_map(|(field, hir_field)| {
                let name = field.ident.as_ref()?.to_string();
                if !causal_name(&name) {
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
                Some(FieldCandidate {
                    is_marked_source: attributes.is_source || name == "source",
                    name,
                    target,
                })
            })
            .collect();

        // Update the accumulated analysis state.
        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            fields,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for analyze_candidate in self.candidates.drain(..) {
            // Reject inputs that do not satisfy this stage.
            if self
                .catalog
                .derived_type(analyze_candidate.definition)
                .is_none()
            {
                continue;
            }

            // Prepare the values used by this stage.
            let nested = analyze_candidate
                .fields
                .into_iter()
                .filter(|field| self.catalog.derived_type(field.target).is_some())
                .collect::<Vec<_>>();

            // Reject inputs that do not satisfy this stage.
            if nested.iter().any(|field| field.is_marked_source) || nested.len() != 1 {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                field: nested[0].name.clone(),
            }
            .emit(cx);
        }
    }
}
