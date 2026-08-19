extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, HirId};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::rules::leptos::utils::view_structure::{LiteralContext, ViewCallSites};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Presentation literal bypassing localization
// -----------------------------------------------------------------------------

/// User-visible literal recovered from authored Leptos markup.
struct Violation {
    /// Expanded expression used to honor local lint attributes.
    owner: HirId,
    /// Exact authored literal or attribute span.
    span: Span,
    /// Decoded literal retained as concrete diagnostic evidence.
    value: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("user-visible view literal bypasses localization")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "even initially fixed interface text must react to the user's selected locale",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "move this phrase into a locale catalog and render it through the localization runtime",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_UNLOCALIZED_VIEW_LITERALS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    format!("literal `{}` is presentation text", self.value),
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosUnlocalizedViewLiterals: Locale catalog boundary policy
// -----------------------------------------------------------------------------

/// Late lint pass rejecting directly authored presentation text in Leptos views.
struct LeptosUnlocalizedViewLiterals {
    /// Deduplicated rstml-backed authored view analysis.
    views: ViewCallSites,
}

impl LeptosUnlocalizedViewLiterals {
    /// Returns whether a literal contains words rather than formatting separators.
    fn is_meaningful(value: &str) -> bool {
        value.chars().any(char::is_alphabetic)
    }

    /// Recognizes attributes and component props whose values are presentation text.
    fn is_translatable_attribute(name: &str) -> bool {
        let terminal = name.rsplit([':', '-']).next().unwrap_or(name);
        matches!(
            name,
            "aria-label" | "alt" | "placeholder" | "title" | "data-label"
        ) || matches!(
            terminal,
            "label" | "title" | "description" | "eyebrow" | "empty" | "hint"
        )
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosUnlocalizedViewLiterals {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Expressions outside authored view macros contain no presentation literals.
        let Some(view) = self.views.analyze(cx, expression) else {
            return;
        };

        // Report visible words and known presentation attributes, not technical markup data.
        for literal in view.literals {
            let translatable = match &literal.context {
                LiteralContext::Text => true,
                LiteralContext::Attribute(name) => Self::is_translatable_attribute(name),
            };
            if !translatable || !Self::is_meaningful(&literal.value) {
                continue;
            }
            Violation {
                owner: view.owner,
                span: literal.span,
                value: literal.value,
            }
            .emit(cx);
        }
    }
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_UNLOCALIZED_VIEW_LITERALS,
    Warn,
    "rejects unlocalized user-visible literals in Leptos views",
    LeptosUnlocalizedViewLiterals {
        views: ViewCallSites::default(),
    }
}

// -----------------------------------------------------------------------------
// Tests: Literal classification
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::LeptosUnlocalizedViewLiterals;

    #[test]
    fn distinguishes_presentation_attributes_and_words_from_technical_values() {
        assert!(LeptosUnlocalizedViewLiterals::is_meaningful("Save"));
        assert!(!LeptosUnlocalizedViewLiterals::is_meaningful(" · "));
        assert!(LeptosUnlocalizedViewLiterals::is_translatable_attribute(
            "aria-label"
        ));
        assert!(LeptosUnlocalizedViewLiterals::is_translatable_attribute(
            "description"
        ));
        assert!(!LeptosUnlocalizedViewLiterals::is_translatable_attribute(
            "aria-controls"
        ));
        assert!(!LeptosUnlocalizedViewLiterals::is_translatable_attribute(
            "href"
        ));
    }
}
