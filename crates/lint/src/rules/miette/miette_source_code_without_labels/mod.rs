extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use super::utils::contracts::{
    DiagnosticCatalog, DiagnosticField, DiagnosticFieldRole, DiagnosticMetadata,
};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Transparency: Diagnostic label ownership
// -----------------------------------------------------------------------------

/// Whether a diagnostic delegates presentation to another diagnostic.
#[derive(Clone, Copy)]
enum Transparency {
    /// The diagnostic delegates its label contract.
    Transparent,
    /// The diagnostic owns its label contract.
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

    /// Returns whether the diagnostic delegates its label contract.
    const fn is_transparent(self) -> bool {
        matches!(self, Self::Transparent)
    }
}

// -----------------------------------------------------------------------------
// Violation: Source code without a diagnostic focus
// -----------------------------------------------------------------------------

/// Source-code fields retained without metadata that selects a relevant span.
struct Violation {
    /// Diagnostic declaration containing the source.
    span: Span,
    /// Source-code field names shown to the author.
    sources: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("diagnostic retains source code without a focus")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "source fields {} have no label, related diagnostic, or diagnostic source that can select a relevant location",
            self.sources.join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "add focused label metadata or remove source-code storage from this diagnostic",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MIETTE_SOURCE_CODE_WITHOUT_LABELS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this source has no diagnostic focus");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// MietteSourceCodeWithoutLabels: Focused source-retention policy
// -----------------------------------------------------------------------------

/// Collects diagnostic field roles before checking source focus.
#[derive(Default)]
struct MietteSourceCodeWithoutLabels {
    /// Derived diagnostic declarations in the crate.
    catalog: DiagnosticCatalog,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_SOURCE_CODE_WITHOUT_LABELS,
    Warn,
    "finds unfocused Miette source-code storage",
    MietteSourceCodeWithoutLabels::default()
}

impl MietteSourceCodeWithoutLabels {
    /// Returns whether a local nested diagnostic can select a source location.
    fn nested_focus(
        catalog: &DiagnosticCatalog,
        target: LocalDefId,
        visited: &mut HashSet<LocalDefId>,
    ) -> bool {
        // Revisiting a type closes a recursive related-diagnostic cycle without new evidence.
        if !visited.insert(target) {
            return false;
        }

        // Non-Miette related types cannot contribute a known label contract.
        let Some(contract) = catalog.derived_type(target) else {
            return true;
        };

        // Transparent diagnostics delegate source presentation beyond this contract.
        if contract.metadata.is_transparent {
            return true;
        }
        contract
            .fields
            .iter()
            .chain(contract.members.iter().flat_map(|member| &member.fields))
            .any(|field| {
                field.roles.contains(DiagnosticFieldRole::Label)
                    || ((field.roles.contains(DiagnosticFieldRole::Related)
                        || field.roles.contains(DiagnosticFieldRole::DiagnosticSource))
                        && field
                            .target
                            .is_none_or(|target| Self::nested_focus(catalog, target, visited)))
            })
    }

    /// Reports source storage with no label or nested diagnostic to focus it.
    fn check_fields(
        cx: &LateContext<'_>,
        catalog: &DiagnosticCatalog,
        span: Span,
        fields: &[DiagnosticField],
        transparency: Transparency,
    ) {
        let sources = fields
            .iter()
            .filter(|field| field.roles.contains(DiagnosticFieldRole::SourceCode))
            .collect::<Vec<_>>();

        // Delegation or any local focus makes the retained source useful.
        if sources.is_empty()
            || transparency.is_transparent()
            || fields.iter().any(|field| {
                field.roles.contains(DiagnosticFieldRole::Label)
                    || ((field.roles.contains(DiagnosticFieldRole::Related)
                        || field.roles.contains(DiagnosticFieldRole::DiagnosticSource))
                        && field.target.is_none_or(|target| {
                            Self::nested_focus(catalog, target, &mut HashSet::new())
                        }))
            })
        {
            return;
        }

        Violation {
            span,
            sources: sources
                .into_iter()
                .map(|field| format!("`{}`", field.name))
                .collect(),
        }
        .emit(cx);
    }
}

impl LateLintPass<'_> for MietteSourceCodeWithoutLabels {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            Self::check_fields(
                cx,
                &self.catalog,
                contract.span,
                &contract.fields,
                Transparency::for_contract(&contract.metadata),
            );
            for member in &contract.members {
                Self::check_fields(
                    cx,
                    &self.catalog,
                    member.span,
                    &member.fields,
                    Transparency::for_member(&member.metadata, &contract.metadata),
                );
            }
        }
    }
}
