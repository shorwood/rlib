extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::contracts::DiagnosticCatalog;
use crate::utils::diagnostic::LateViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `code` value used by this analysis.
    code: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "diagnostic code `{}` has an unstable shape",
            self.code
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "codes should be stable machine identifiers, while mixed casing and presentation-like words tend to drift with user-facing text",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use a namespaced lowercase path such as `config::invalid_value` or a letter-number code",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MIETTE_MALFORMED_DIAGNOSTIC_CODES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this code does not use a stable accepted form");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Performs the `valid_code` step of the lint analysis.
fn valid_code(code: &str) -> bool {
    /// Namespace and local name required by a qualified diagnostic code.
    const MINIMUM_QUALIFIED_CODE_SEGMENTS: usize = 2;

    // Prepare the values used by this stage.
    let segments = code.split("::").collect::<Vec<_>>();

    // Reject inputs that do not satisfy this stage.
    if segments.len() >= MINIMUM_QUALIFIED_CODE_SEGMENTS {
        return segments.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_lowercase())
                && segment.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
                })
        });
    }

    // Prepare the values used by this stage.
    let letters = code.chars().take_while(char::is_ascii_alphabetic).count();

    // Perform the next step of the analysis.
    letters > 0
        && letters < code.len()
        && code[..letters]
            .chars()
            .all(|character| character.is_ascii_uppercase())
        && code[letters..]
            .chars()
            .all(|character| character.is_ascii_digit())
}

/// Performs the `check_code` step of the lint analysis.
fn check_code(cx: &LateContext<'_>, span: Span, code: Option<&str>) {
    // Prepare the values used by this stage.
    let Some(code) = code else {
        return;
    };
    if valid_code(code) {
        return;
    }

    // Perform the next step of the analysis.
    Violation {
        span,
        code: code.to_owned(),
    }
    .emit(cx);
}

#[derive(Default)]
/// Carries the `MietteMalformedDiagnosticCodes` state used by this analysis.
struct MietteMalformedDiagnosticCodes {
    /// Stores the `catalog` value used by this analysis.
    catalog: DiagnosticCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_MALFORMED_DIAGNOSTIC_CODES,
    Warn,
    "finds Miette diagnostic codes with unstable identifier forms",
    MietteMalformedDiagnosticCodes::default()
}

impl LateLintPass<'_> for MietteMalformedDiagnosticCodes {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            check_code(cx, contract.span, contract.metadata.code.as_deref());
            for member in &contract.members {
                check_code(cx, member.span, member.metadata.code.as_deref());
            }
        }
    }
}
