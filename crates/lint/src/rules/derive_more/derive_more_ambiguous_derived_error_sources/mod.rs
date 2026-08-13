extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::DeriveMoreContractCatalog;
use crate::utils::diagnostic::LateViolation;
use crate::utils::source_provenance::AuthoredItemSource;

// -----------------------------------------------------------------------------
// Violation: Competing derived error sources
// -----------------------------------------------------------------------------

/// Derived error type awaiting identification of plausible causal fields.
struct Candidate {
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Additional field competing for the same semantic role.
    competing_field: String,
}

/// Derived error whose source field cannot be selected unambiguously.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Local type name used to identify the affected derive contract.
    name: String,
    /// Additional field competing for the same semantic role.
    competing_field: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "derived error source for `{}` is ambiguous with `{}`",
            self.name, self.competing_field
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "derive_more selects the field named `source` by convention even though another field is named like an error cause",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "mark the intended field with `#[error(source)]` and classify the other cause-like field explicitly",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_AMBIGUOUS_DERIVED_ERROR_SOURCES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this type relies on implicit source selection");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// DeriveMoreAmbiguousDerivedErrorSources: Unambiguous source policy
// -----------------------------------------------------------------------------

/// Rejects generated error sources when several fields plausibly own the causal chain.
#[derive(Default)]
struct DeriveMoreAmbiguousDerivedErrorSources {
    /// Authored type contracts and `derive_more` expansions consulted by this rule.
    catalog: DeriveMoreContractCatalog,
    /// Authored declarations awaiting association with `derive_more` expansions.
    candidates: Vec<Candidate>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_AMBIGUOUS_DERIVED_ERROR_SOURCES,
    Warn,
    "requires explicit derive_more Error source selection when field roles compete",
    DeriveMoreAmbiguousDerivedErrorSources::default()
}

impl LateLintPass<'_> for DeriveMoreAmbiguousDerivedErrorSources {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() || !matches!(item.kind, ItemKind::Struct(..)) {
            return;
        }
        let Some(source) = AuthoredItemSource::for_item(cx, item) else {
            return;
        };

        let Ok(structure) = syn::parse_str::<syn::ItemStruct>(&source) else {
            return;
        };

        if structure.fields.iter().any(|field| {
            field
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("error"))
        }) {
            return;
        }

        let has_implicit_source = structure
            .fields
            .iter()
            .any(|field| field.ident.as_ref().is_some_and(|name| name == "source"));
        if !has_implicit_source {
            return;
        }

        let Some(competing_field) = structure.fields.iter().find_map(|field| {
            let name = field.ident.as_ref()?;
            let normalized = name.to_string().to_ascii_lowercase();
            (name != "source" && (normalized.ends_with("error") || normalized.ends_with("cause")))
                .then(|| name.to_string())
        }) else {
            return;
        };

        self.candidates.push(Candidate {
            definition: item.owner_id.def_id,
            span: item.span,
            competing_field,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for candidate in self.candidates.drain(..) {
            let Some(contract) = self.catalog.derived_type(candidate.definition, "Error") else {
                continue;
            };

            Violation {
                span: candidate.span,
                name: contract.name.to_string(),
                competing_field: candidate.competing_field,
            }
            .emit(cx);
        }
    }
}
