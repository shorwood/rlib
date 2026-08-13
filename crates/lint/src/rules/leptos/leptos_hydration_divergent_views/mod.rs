extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ExprKind, HirId};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Hydration shape mismatch
// -----------------------------------------------------------------------------

struct Violation {
    owner: HirId,
    span: Span,
    server_shape: Vec<String>,
    browser_shape: Vec<String>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("server and browser branches produce different initial view shapes")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "hydration expects the server DOM shape {:?} to match the browser shape {:?}",
            self.server_shape, self.browser_shape
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "render one stable initial element shape and defer browser-only changes until after hydration",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_HYDRATION_DIVERGENT_VIEWS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this environment branch changes the authored node shape");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosHydrationDivergentViews: Initial-view parity policy
// -----------------------------------------------------------------------------

struct LeptosHydrationDivergentViews;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_HYDRATION_DIVERGENT_VIEWS,
    Warn,
    "rejects environment-dependent initial Leptos view shapes",
    LeptosHydrationDivergentViews
}

impl LeptosHydrationDivergentViews {
    fn environment_condition(source: &str) -> bool {
        let compact = source.split_ascii_whitespace().collect::<String>();
        compact.contains("cfg!(target_arch=\"wasm32\")")
            || compact.contains("cfg!(feature=\"ssr\")")
            || compact.contains("cfg!(feature=\"hydrate\")")
    }

    fn view_shape(source: &str) -> Option<Vec<String>> {
        if !source.contains("view!") {
            return None;
        }
        let bytes = source.as_bytes();
        let mut shape = Vec::new();
        let mut cursor = 0;
        while cursor < bytes.len() {
            let Some(relative) = source[cursor..].find('<') else {
                break;
            };
            cursor += relative + 1;
            if cursor >= bytes.len() || matches!(bytes[cursor], b'/' | b'>' | b'!') {
                continue;
            }
            let start = cursor;
            while cursor < bytes.len()
                && (bytes[cursor].is_ascii_alphanumeric()
                    || matches!(bytes[cursor], b'_' | b'-' | b':'))
            {
                cursor += 1;
            }
            if cursor > start {
                shape.push(source[start..cursor].to_owned());
            }
        }
        (!shape.is_empty()).then_some(shape)
    }
}

impl LateLintPass<'_> for LeptosHydrationDivergentViews {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        if expression.span.from_expansion() {
            return;
        }
        let ExprKind::If(condition, then_branch, Some(else_branch)) = expression.kind else {
            return;
        };
        let source_map = cx.sess().source_map();
        let Ok(condition_source) = source_map.span_to_snippet(condition.span) else {
            return;
        };
        if !Self::environment_condition(&condition_source) {
            return;
        }
        let Ok(then_source) = source_map.span_to_snippet(then_branch.span) else {
            return;
        };
        let Ok(else_source) = source_map.span_to_snippet(else_branch.span) else {
            return;
        };
        let (Some(then_shape), Some(else_shape)) = (
            Self::view_shape(&then_source),
            Self::view_shape(&else_source),
        ) else {
            return;
        };
        if then_shape == else_shape {
            return;
        }
        Violation {
            owner: expression.hir_id,
            span: expression.span,
            server_shape: else_shape,
            browser_shape: then_shape,
        }
        .emit(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::LeptosHydrationDivergentViews;

    #[test]
    fn compares_element_identity_without_text_content() {
        assert_eq!(
            LeptosHydrationDivergentViews::view_shape("view! { <main><ClientToolbar/></main> }")
                .unwrap(),
            ["main", "ClientToolbar"]
        );
    }
}
