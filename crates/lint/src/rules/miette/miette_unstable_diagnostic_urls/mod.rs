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

struct Violation {
    span: Span,
    url: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "diagnostic URL `{}` is not a stable external link",
            self.url
        ))
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "local, insecure, relative, or presentation-derived links are unsuitable as durable diagnostic metadata",
        )
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("use a static HTTPS documentation URL with a stable path")
    }
    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            MIETTE_UNSTABLE_DIAGNOSTIC_URLS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this diagnostic exposes an unstable URL");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct MietteUnstableDiagnosticUrls {
    catalog: DiagnosticCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_UNSTABLE_DIAGNOSTIC_URLS,
    Warn,
    "finds unstable static Miette diagnostic URLs",
    MietteUnstableDiagnosticUrls::default()
}

impl LateLintPass<'_> for MietteUnstableDiagnosticUrls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }
    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            check_url(cx, contract.span, contract.metadata.url.as_deref());
            for member in &contract.members {
                check_url(cx, member.span, member.metadata.url.as_deref());
            }
        }
    }
}

fn check_url(cx: &LateContext<'_>, span: Span, url: Option<&str>) {
    let Some(url) = url else {
        return;
    };
    if !stable_url(url) {
        Violation {
            span,
            url: url.to_owned(),
        }
        .emit(cx);
    }
}

fn stable_url(url: &str) -> bool {
    url.starts_with("https://")
        && !url.contains('{')
        && !url.contains('}')
        && !url.contains("localhost")
        && !url.contains("127.0.0.1")
        && url[8..].contains('.')
}
