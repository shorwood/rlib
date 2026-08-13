extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{ThiserrorContractCatalog, thiserror_attributes};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::authored_item_source;

struct Candidate {
    definition: LocalDefId,
    span: Span,
    exposures: Vec<String>,
}

struct Violation {
    span: Span,
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
struct ThiserrorOpaqueErrorsExposingRepresentations {
    catalog: ThiserrorContractCatalog,
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
        self.catalog.check_item(cx, item);
        let ItemKind::Enum(_, _, definition) = item.kind else {
            return;
        };
        if item.span.from_expansion() || !cx.tcx.visibility(item.owner_id.def_id).is_public() {
            return;
        }
        let Some(source) = authored_item_source(cx, item) else {
            return;
        };
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        let mut exposures = Vec::new();
        for (variant, hir_variant) in enumeration.variants.iter().zip(definition.variants) {
            for (field, hir_field) in variant.fields.iter().zip(hir_variant.data.fields()) {
                let attributes = thiserror_attributes(&field.attrs);
                let conventional_source = field
                    .ident
                    .as_ref()
                    .is_some_and(|identifier| identifier == "source");
                if !attributes.source && !conventional_source {
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
