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
// Violation: Foreign error representation in a public enum
// -----------------------------------------------------------------------------

/// Public error enum whose variants expose dependency-specific source types.
struct Violation {
    /// Public enum receiving the diagnostic.
    span: Span,
    /// Variants and foreign source types exposed to downstream patterns.
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

// -----------------------------------------------------------------------------
// Candidate: Public foreign-source evidence
// -----------------------------------------------------------------------------

/// Foreign source exposures retained until thiserror derivation is confirmed.
struct Candidate {
    /// Candidate public error enum definition.
    definition: LocalDefId,
    /// Enum declaration receiving a later diagnostic.
    span: Span,
    /// Variant and foreign-type descriptions.
    exposures: Vec<String>,
}

// -----------------------------------------------------------------------------
// ThiserrorOpaqueErrorsExposingRepresentations: Stable public shape policy
// -----------------------------------------------------------------------------

/// Correlates public source-bearing variants with local thiserror contracts.
#[derive(Default)]
struct ThiserrorOpaqueErrorsExposingRepresentations {
    /// Local derived error contracts.
    catalog: ThiserrorContractCatalog,
    /// Public enums awaiting derive confirmation.
    candidates: Vec<Candidate>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub THISERROR_OPAQUE_ERRORS_EXPOSING_REPRESENTATIONS,
    Warn,
    "finds foreign error representations exposed by public thiserror enums",
    ThiserrorOpaqueErrorsExposingRepresentations::default()
}

impl LateLintPass<'_> for ThiserrorOpaqueErrorsExposingRepresentations {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);

        // Only enums can expose variant-specific foreign error representations.
        let ItemKind::Enum(_, _, definition) = item.kind else {
            return;
        };

        // Generated or non-exported enums do not define the public authored contract.
        if item.span.from_expansion()
            || !cx
                .tcx
                .effective_visibilities(())
                .is_exported(item.owner_id.def_id)
        {
            return;
        }

        // Missing authored source prevents recovery of thiserror field roles.
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Unparseable enum source cannot be aligned with HIR variants and fields.
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        let mut exposures = Vec::new();

        // Inspect only fields that participate in the public causal chain.
        for (variant, hir_variant) in enumeration.variants.iter().zip(definition.variants) {
            let is_transparent = variant.attrs.iter().any(|attribute| {
                attribute.path().is_ident("error")
                    && attribute
                        .parse_args::<syn::Path>()
                        .is_ok_and(|path| path.is_ident("transparent"))
            });
            for (field, hir_field) in variant.fields.iter().zip(hir_variant.data.fields()) {
                let attributes = ThiserrorAttributes::from_attributes(&field.attrs);
                let has_conventional_source = field
                    .ident
                    .as_ref()
                    .is_some_and(|identifier| identifier == "source");

                if !(attributes.is_source
                    || has_conventional_source
                    || (is_transparent && variant.fields.len() == 1))
                {
                    continue;
                }

                let Some(target) = cx
                    .tcx
                    .type_of(hir_field.def_id)
                    .instantiate_identity()
                    .ty_adt_def()
                else {
                    continue;
                };
                if target.did().is_local() {
                    continue;
                }

                exposures.push(format!(
                    "`{}({})`",
                    variant.ident,
                    cx.tcx.def_path_str(target.did())
                ));
            }
        }

        // Enums without a foreign causal field expose no representation through their variants.
        if exposures.is_empty() {
            return;
        }

        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            exposures,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if self.catalog.derived_type(candidate.definition).is_none() {
                continue;
            }

            Violation {
                span: candidate.span,
                exposures: candidate.exposures,
            }
            .emit(cx);
        }
    }
}
