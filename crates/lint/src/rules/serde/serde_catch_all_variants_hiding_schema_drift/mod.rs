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

// -----------------------------------------------------------------------------
// Violation: Catch-all variant hiding schema drift
// -----------------------------------------------------------------------------

/// Catch-all variant awaiting external-schema and visibility checks.
struct Candidate {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Variants participating in the analyzed contract.
    variants: Vec<String>,
}

/// Catch-all variant that silently absorbs additions to an external schema.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Variants participating in the analyzed contract.
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
// -----------------------------------------------------------------------------
// SerdeCatchAllVariantsHidingSchemaDrift: Visible schema-change policy
// -----------------------------------------------------------------------------

/// Rejects catch-all variants that silently absorb additions to an external schema.
struct SerdeCatchAllVariantsHidingSchemaDrift {
    /// Effective Serde contracts consulted after all local declarations are known.
    catalog: SerdeContractCatalog,
    /// Authored Serde declarations awaiting crate-wide contract comparison.
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
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Enum(..)) {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        let Ok(enumeration) = syn::parse_str::<syn::ItemEnum>(&source) else {
            return;
        };
        if !matches!(enumeration.vis, syn::Visibility::Public(_)) {
            return;
        }

        let variants = enumeration
            .variants
            .iter()
            .filter(|variant| {
                SerdeAttributes::from_attributes(&variant.attrs).has(SerdeFlag::Other)
            })
            .map(|variant| format!("`{}`", variant.ident))
            .collect::<Vec<_>>();

        if variants.is_empty() {
            return;
        }

        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            variants,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            if self
                .catalog
                .derived_type(candidate.definition, "Deserialize")
                .is_none()
            {
                continue;
            }

            Violation {
                span: candidate.span,
                variants: candidate.variants,
            }
            .emit(cx);
        }
    }
}
