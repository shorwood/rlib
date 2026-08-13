extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::contracts::{SerdeAttributes, SerdeContractCatalog, SerdeFlag};
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

/// Carries the `Candidate` state used by this analysis.
struct Candidate {
    /// Stores the `definition` value used by this analysis.
    definition: LocalDefId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `variants` value used by this analysis.
    variants: Vec<String>,
}

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `variants` value used by this analysis.
    variants: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("public Serde deserialization hides unknown variants")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "{} {} the unknown wire spelling, making new or misspelled values indistinguishable",
            self.variants.join(", "),
            if self.variants.len() == 1 {
                "discards"
            } else {
                "discard"
            }
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "reject unknown variants at this boundary, or preserve their original value in an explicit representation",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            SERDE_CATCH_ALL_VARIANTS_HIDING_SCHEMA_DRIFT,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this public contract silently absorbs schema drift",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
/// Carries the `SerdeCatchAllVariantsHidingSchemaDrift` state used by this analysis.
struct SerdeCatchAllVariantsHidingSchemaDrift {
    /// Stores the `catalog` value used by this analysis.
    catalog: SerdeContractCatalog,
    /// Stores the `candidates` value used by this analysis.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SERDE_CATCH_ALL_VARIANTS_HIDING_SCHEMA_DRIFT,
    Warn,
    "finds public Serde catch-all variants that hide schema drift",
    SerdeCatchAllVariantsHidingSchemaDrift::default()
}

impl LateLintPass<'_> for SerdeCatchAllVariantsHidingSchemaDrift {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Update the accumulated analysis state.
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Enum(..)) {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        // Prepare the values used by this stage.
        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        if !matches!(enumeration.vis, syn::Visibility::Public(_)) {
            return;
        }

        // Prepare the values used by this stage.
        let variants = enumeration
            .variants
            .iter()
            .filter(|variant| {
                SerdeAttributes::analyze_serde_attributes(&variant.attrs).has(SerdeFlag::Other)
            })
            .map(|variant| format!("`{}`", variant.ident))
            .collect::<Vec<_>>();

        // Reject inputs that do not satisfy this stage.
        if variants.is_empty() {
            return;
        }

        // Update the accumulated analysis state.
        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            variants,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for analyze_candidate in self.candidates.drain(..) {
            // Reject inputs that do not satisfy this stage.
            if self
                .catalog
                .derived_type(analyze_candidate.definition, "Deserialize")
                .is_none()
            {
                continue;
            }

            // Perform the next step of the analysis.
            Violation {
                span: analyze_candidate.span,
                variants: analyze_candidate.variants,
            }
            .emit(cx);
        }
    }
}
