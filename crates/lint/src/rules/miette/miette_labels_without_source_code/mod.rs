extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::utils::contracts::{
    DiagnosticCatalog, DiagnosticField, DiagnosticFieldRole, DiagnosticMetadata,
};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Transparency: Diagnostic source ownership
// -----------------------------------------------------------------------------

/// Whether a diagnostic delegates presentation to another diagnostic.
#[derive(Clone, Copy)]
enum Transparency {
    /// The diagnostic delegates its source contract.
    Transparent,
    /// The diagnostic owns its source contract.
    Opaque,
}

impl Transparency {
    /// Classifies a complete diagnostic contract.
    const fn for_contract(metadata: &DiagnosticMetadata) -> Self {
        if metadata.is_transparent {
            Self::Transparent
        } else {
            Self::Opaque
        }
    }

    /// Classifies a member together with its containing diagnostic contract.
    const fn for_member(member: &DiagnosticMetadata, contract: &DiagnosticMetadata) -> Self {
        if member.is_transparent || contract.is_transparent {
            Self::Transparent
        } else {
            Self::Opaque
        }
    }

    /// Returns whether the diagnostic delegates its source contract.
    const fn is_transparent(self) -> bool {
        matches!(self, Self::Transparent)
    }
}

// -----------------------------------------------------------------------------
// Violation: Labels without available source code
// -----------------------------------------------------------------------------

/// Label fields that cannot resolve against source text.
struct Violation {
    /// Diagnostic declaration containing the labels.
    span: Span,
    /// Label field names shown to the author.
    labels: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("diagnostic labels have no source code")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "label fields {} cannot render excerpts without a local or forwarded source",
            self.labels.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "add a `#[source_code]` field, forward a diagnostic source, or remove the unusable labels",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MIETTE_LABELS_WITHOUT_SOURCE_CODE,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "these labels cannot resolve against source text");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// MietteLabelsWithoutSourceCode: Renderable label policy
// -----------------------------------------------------------------------------

/// Collects diagnostic field roles before checking label-source pairing.
#[derive(Default)]
struct MietteLabelsWithoutSourceCode {
    /// Derived diagnostic declarations in the crate.
    catalog: DiagnosticCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_LABELS_WITHOUT_SOURCE_CODE,
    Warn,
    "finds Miette labels without an available source",
    MietteLabelsWithoutSourceCode::default()
}

impl MietteLabelsWithoutSourceCode {
    /// Reports label sets with neither local nor forwarded source code.
    fn check_fields(
        cx: &LateContext<'_>,
        span: Span,
        fields: &[DiagnosticField],
        transparency: Transparency,
    ) {
        let labels = fields
            .iter()
            .filter(|field| field.roles.contains(DiagnosticFieldRole::Label))
            .collect::<Vec<_>>();

        // Delegated diagnostics and locally sourced labels already have renderable text.
        if labels.is_empty()
            || transparency.is_transparent()
            || fields.iter().any(|field| {
                field.roles.contains(DiagnosticFieldRole::SourceCode)
                    || field.roles.contains(DiagnosticFieldRole::DiagnosticSource)
            })
        {
            return;
        }

        Violation {
            span,
            labels: labels
                .into_iter()
                .map(|field| format!("`{}`", field.name))
                .collect(),
        }
        .emit(cx);
    }
}
impl LateLintPass<'_> for MietteLabelsWithoutSourceCode {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            Self::check_fields(
                cx,
                contract.span,
                &contract.fields,
                Transparency::for_contract(&contract.metadata),
            );
            for member in &contract.members {
                Self::check_fields(
                    cx,
                    member.span,
                    &member.fields,
                    Transparency::for_member(&member.metadata, &contract.metadata),
                );
            }
        }
    }
}
