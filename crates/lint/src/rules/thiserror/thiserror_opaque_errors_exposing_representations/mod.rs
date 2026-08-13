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

/// Carries the `Candidate` state used by this analysis.
struct Candidate {
    /// Stores the `definition` value used by this analysis.
    definition: LocalDefId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `exposures` value used by this analysis.
    exposures: Vec<String>,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `exposures` value used by this analysis.
    exposures: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("public thiserror enum exposes foreign error representations")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "source-bearing {} {} part of downstream pattern matching: {}",
            if self.exposures.len() == 1 {
                "variant"
            } else {
                "variants"
            },
            if self.exposures.len() == 1 {
                "becomes"
            } else {
                "become"
            },
            self.exposures.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use an opaque public wrapper or stable owned variants, keeping foreign errors behind a private source boundary",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            THISERROR_OPAQUE_ERRORS_EXPOSING_REPRESENTATIONS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this public enum leaks dependency-specific error shapes",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
/// Carries the `ThiserrorOpaqueErrorsExposingRepresentations` state used by this analysis.
struct ThiserrorOpaqueErrorsExposingRepresentations {
    /// Stores the `catalog` value used by this analysis.
    catalog: ThiserrorContractCatalog,
    /// Stores the `candidates` value used by this analysis.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_OPAQUE_ERRORS_EXPOSING_REPRESENTATIONS,
    Warn,
    "finds foreign error representations exposed by public thiserror enums",
    ThiserrorOpaqueErrorsExposingRepresentations::default()
}

impl LateLintPass<'_> for ThiserrorOpaqueErrorsExposingRepresentations {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Update the accumulated analysis state.
        self.catalog.check_item(cx, item);
        let ItemKind::Enum(_, _, definition) = item.kind else {
            return;
        };
        if item.span.from_expansion() || !cx.tcx.visibility(item.owner_id.def_id).is_public() {
            return;
        }

        // Prepare the values used by this stage.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        let mut exposures = Vec::new();

        // Process the candidates handled by this stage.
        for (variant, hir_variant) in enumeration.variants.iter().zip(definition.variants) {
            for (field, hir_field) in variant.fields.iter().zip(hir_variant.data.fields()) {
                // Prepare the values used by this stage.
                let attributes = ThiserrorAttributes::from_attributes(&field.attrs);
                let conventional_source = field
                    .ident
                    .as_ref()
                    .is_some_and(|identifier| identifier == "source");

                // Reject inputs that do not satisfy this stage.
                if !attributes.is_source && !conventional_source {
                    continue;
                }

                // Prepare the values used by this stage.
                let Some(target) = cx
                    .tcx
                    .type_of(hir_field.def_id)
                    .instantiate_identity()
                    .ty_adt_def()
                // Perform the next step of the analysis.
                else {
                    continue;
                };
                if target.did().is_local() {
                    continue;
                }

                // Perform the next step of the analysis.
                exposures.push(format!(
                    "`{}({})`",
                    variant.ident,
                    cx.tcx.def_path_str(target.did())
                ));
            }
        }

        // Reject inputs that do not satisfy this stage.
        if exposures.is_empty() {
            return;
        }

        // Update the accumulated analysis state.
        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            exposures,
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

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                exposures: analyze_candidate.exposures,
            }
            .emit(cx);
        }
    }
}
