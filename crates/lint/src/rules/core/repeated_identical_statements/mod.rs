extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Block;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Repeated statement diagnostic
// -----------------------------------------------------------------------------

/// Adjacent authored statements with identical normalized source.
struct Violation {
    /// First occurrence that establishes the duplicated operation.
    first_span: Span,
    /// Later occurrence receiving the primary diagnostic.
    repeated_span: Span,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("this statement exactly repeats its predecessor")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "adjacent identical statements resemble an accidental copy, while intentional multiplicity remains invisible",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("remove the duplicate or express intentional multiplicity explicitly")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            REPEATED_IDENTICAL_STATEMENTS,
            self.repeated_span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.first_span, "the same statement first appears here");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// RepeatedIdenticalStatements: Explicit repetition policy
// -----------------------------------------------------------------------------

/// Compares neighboring statements within each authored block.
struct RepeatedIdenticalStatements;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub REPEATED_IDENTICAL_STATEMENTS,
    Warn,
    "rejects adjacent statements with identical authored source",
    RepeatedIdenticalStatements
}

impl LateLintPass<'_> for RepeatedIdenticalStatements {
    fn check_block(&mut self, cx: &LateContext<'_>, block: &Block<'_>) {
        for pair in block.stmts.windows(2) {
            // Retain adjacent authored statements only.
            let [first, repeated] = pair else { continue };
            if first.span.from_expansion() || repeated.span.from_expansion() {
                continue;
            }

            // Compare normalized source only when both snippets are available.
            let source_map = cx.sess().source_map();
            let (Ok(first_source), Ok(repeated_source)) = (
                source_map.span_to_snippet(first.span),
                source_map.span_to_snippet(repeated.span),
            ) else {
                continue;
            };
            if first_source.trim().is_empty() || first_source.trim() != repeated_source.trim() {
                continue;
            }

            // Preserve both occurrences for one duplicate-focused diagnostic.
            Violation {
                first_span: first.span,
                repeated_span: repeated.span,
            }
            .emit(cx);
        }
    }
}
