extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rustc_errors::DiagDecorator;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::hygiene::{ExpnKind, MacroKind};
use rustc_span::{BytePos, Span};

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Unstable key diagnostic
// -----------------------------------------------------------------------------

/// Key expression that cannot retain stable item identity.
struct Violation {
    /// Function body used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored key expression highlighted by the diagnostic.
    span: Span,
    /// Concise reason the key is unstable.
    reason: &'static str,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("`<For>` key does not represent stable item identity")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "keys preserve the relationship between data and rendered rows when a collection changes",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("use a unique domain identifier that remains stable across inserts and reordering")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_UNSTABLE_FOR_KEYS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, self.reason);
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosUnstableForKeys: Key identity policy
// -----------------------------------------------------------------------------

/// Late lint pass that rejects constant and position-based keyed iteration.
#[derive(Default)]
struct LeptosUnstableForKeys {
    /// Absolute source ranges already diagnosed while visiting one expanded view.
    reported: HashSet<(u32, u32)>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_UNSTABLE_FOR_KEYS,
    Warn,
    "rejects constant and position-based Leptos For keys",
    LeptosUnstableForKeys::default()
}

impl LeptosUnstableForKeys {
    /// Classifies one authored key closure prefix and returns its diagnostic width.
    fn unstable_key(source: &str) -> Option<(usize, &'static str)> {
        for constant in ["|_| ()", "|_| true", "|_| false"] {
            if source.starts_with(constant) {
                return Some((constant.len(), "every row receives the same key"));
            }
        }
        if let Some(rest) = source.strip_prefix("|_| ") {
            let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
            if digits > 0 {
                return Some((4 + digits, "every row receives the same key"));
            }
        }
        for name in ["i", "idx", "index", "position"] {
            for key in [
                format!("|{name}| {name}"),
                format!("|({name}, _)| *{name}"),
                format!("|({name}, _)| {name}"),
            ] {
                if source.starts_with(&key) {
                    return Some((
                        key.len(),
                        "the row position changes when the collection is reordered",
                    ));
                }
            }
        }
        None
    }

    /// Finds unstable key closures inside authored `<For>` markup.
    fn findings(source: &str) -> Vec<(usize, usize, &'static str)> {
        let mut findings = Vec::new();
        let mut cursor = 0;
        while let Some(relative) = source[cursor..].find("key=") {
            let attribute = cursor + relative;
            let before = &source[..attribute];
            let is_for = before.rsplit_once('<').is_some_and(|(_, tag)| {
                matches!(tag.split_ascii_whitespace().next(), Some("For" | "ForEnumerate"))
            });
            let value = attribute + "key=".len();
            let whitespace = source[value..]
                .bytes()
                .take_while(u8::is_ascii_whitespace)
                .count();
            let start = value + whitespace;
            if is_for
                && let Some((length, reason)) = Self::unstable_key(&source[start..])
            {
                findings.push((start, length, reason));
            }
            cursor = value;
        }
        findings
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosUnstableForKeys {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        let Some(view_span) = expression.span.macro_backtrace().find_map(|expansion| {
            matches!(expansion.kind, ExpnKind::Macro(MacroKind::Bang, name) if name.as_str() == "view")
                .then_some(expansion.call_site)
        }) else {
            return;
        };
        let Ok(source) = cx.sess().source_map().span_to_snippet(view_span) else {
            return;
        };
        for (offset, length, reason) in Self::findings(&source) {
            let lo = view_span.lo() + BytePos(offset as u32);
            let hi = lo + BytePos(length as u32);
            if !self.reported.insert((lo.0, hi.0)) {
                continue;
            }
            Violation {
                owner: expression.hir_id,
                span: Span::with_root_ctxt(lo, hi),
                reason,
            }
            .emit(cx);
        }
    }
}

// -----------------------------------------------------------------------------
// Tests: Key source classification
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::LeptosUnstableForKeys;

    #[test]
    fn distinguishes_position_keys_from_domain_keys() {
        assert!(LeptosUnstableForKeys::unstable_key("|_| 0").is_some());
        assert!(LeptosUnstableForKeys::unstable_key("|(index, _)| *index").is_some());
        assert!(LeptosUnstableForKeys::unstable_key("|item| item.id").is_none());
    }
}
