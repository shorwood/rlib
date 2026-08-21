extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::utils::contracts::DiagnosticCatalog;
use crate::utils::diagnostic::LateViolation;

/// Namespace and local name required by a qualified diagnostic code.
const MINIMUM_QUALIFIED_CODE_SEGMENTS: usize = 2;

// -----------------------------------------------------------------------------
// Violation: Malformed diagnostic code
// -----------------------------------------------------------------------------

/// Diagnostic code whose spelling is unsuitable as a stable identifier.
struct Violation {
    /// Diagnostic declaration carrying the code.
    span: Span,
    /// Authored code quoted in the diagnostic.
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

// -----------------------------------------------------------------------------
// MietteMalformedDiagnosticCodes: Stable code spelling policy
// -----------------------------------------------------------------------------

/// Collects derived diagnostic codes before validating their spelling.
#[derive(Default)]
struct MietteMalformedDiagnosticCodes {
    /// Derived diagnostic declarations in the crate.
    catalog: DiagnosticCatalog,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_MALFORMED_DIAGNOSTIC_CODES,
    Warn,
    "finds Miette diagnostic codes with unstable identifier forms",
    MietteMalformedDiagnosticCodes::default()
}

impl MietteMalformedDiagnosticCodes {
    /// Accepts namespaced snake-case codes and conventional letter-number codes.
    fn is_valid_code(code: &str) -> bool {
        let segments = code.split("::").collect::<Vec<_>>();

        // Qualified codes reserve each segment for a stable machine-oriented name.
        if segments.len() >= MINIMUM_QUALIFIED_CODE_SEGMENTS {
            return segments.iter().all(|segment| {
                !segment.is_empty()
                    && !segment.ends_with('_')
                    && !segment.contains("__")
                    && segment
                        .chars()
                        .next()
                        .is_some_and(|character| character.is_ascii_lowercase())
                    && segment.chars().all(|character| {
                        character.is_ascii_lowercase()
                            || character.is_ascii_digit()
                            || character == '_'
                    })
            });
        }

        let letters = code.chars().take_while(char::is_ascii_alphabetic).count();

        letters > 0
            && letters < code.len()
            && code[..letters]
                .chars()
                .all(|character| character.is_ascii_uppercase())
            && code[letters..]
                .chars()
                .all(|character| character.is_ascii_digit())
    }

    /// Reports a present code that does not use either accepted shape.
    fn check_code(cx: &LateContext<'_>, span: Span, code: Option<&str>) {
        // Diagnostics without a code have no code syntax to validate.
        let Some(code) = code else {
            return;
        };

        // Codes matching an accepted shape require no diagnostic.
        if Self::is_valid_code(code) {
            return;
        }

        Violation {
            span,
            code: code.to_owned(),
        }
        .emit(cx);
    }
}

impl LateLintPass<'_> for MietteMalformedDiagnosticCodes {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            Self::check_code(cx, contract.span, contract.metadata.code.as_deref());
            for member in &contract.members {
                Self::check_code(cx, member.span, member.metadata.code.as_deref());
            }
        }
    }
}
