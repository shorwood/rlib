extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::net::IpAddr;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::utils::contracts::DiagnosticCatalog;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Unstable diagnostic documentation URL
// -----------------------------------------------------------------------------

/// Diagnostic metadata that cannot serve as a durable external link.
struct Violation {
    /// Diagnostic declaration carrying the URL.
    span: Span,
    /// Authored URL quoted in the diagnostic.
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

// -----------------------------------------------------------------------------
// MietteUnstableDiagnosticUrls: Durable documentation links
// -----------------------------------------------------------------------------

/// Collects derived diagnostic URLs before validating their stability.
#[derive(Default)]
struct MietteUnstableDiagnosticUrls {
    /// Derived diagnostic declarations in the crate.
    catalog: DiagnosticCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_UNSTABLE_DIAGNOSTIC_URLS,
    Warn,
    "finds unstable static Miette diagnostic URLs",
    MietteUnstableDiagnosticUrls::default()
}

impl MietteUnstableDiagnosticUrls {
    /// Returns whether a host is externally routable enough for durable documentation.
    fn public_host(authority: &str) -> bool {
        // Empty authorities and embedded credentials are not stable public hosts.
        if authority.is_empty() || authority.contains('@') {
            return false;
        }
        let host = if let Some(bracketed) = authority.strip_prefix('[') {
            // Bracketed IPv6 authorities require a complete closing delimiter.
            let Some((host, suffix)) = bracketed.split_once(']') else {
                return false;
            };

            // Any suffix must be a nonempty numeric port declaration.
            if !suffix.is_empty()
                && !suffix.strip_prefix(':').is_some_and(|port| {
                    !port.is_empty() && port.chars().all(|c| c.is_ascii_digit())
                })
            {
                return false;
            }
            host
        } else if let Some((host, port)) = authority.rsplit_once(':') {
            // Unbracketed port suffixes must contain only a nonempty numeric port.
            if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
                return false;
            }
            host
        } else {
            authority
        };

        // IP literals can be classified directly by their routability properties.
        if let Ok(address) = host.parse::<IpAddr>() {
            return match address {
                IpAddr::V4(address) => {
                    !(address.is_loopback()
                        || address.is_private()
                        || address.is_link_local()
                        || address.is_unspecified())
                }
                IpAddr::V6(address) => {
                    !(address.is_loopback()
                        || address.is_unspecified()
                        || address.is_unique_local()
                        || address.is_unicast_link_local())
                }
            };
        }

        let host = host.to_ascii_lowercase();
        let labels = host.split('.').collect::<Vec<_>>();
        labels.len() >= 2
            && host != "localhost"
            && !host.ends_with(".localhost")
            && labels.iter().all(|label| {
                !label.is_empty()
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '-')
            })
    }

    /// Returns whether a URL is a static public HTTPS address.
    fn stable_url(url: &str) -> bool {
        // Durable diagnostic documentation must use an encrypted absolute URL.
        let Some(remainder) = url.strip_prefix("https://") else {
            return false;
        };

        // Interpolation markers make the metadata dynamic rather than a stable link.
        if url.contains('{') || url.contains('}') {
            return false;
        }
        let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
        let (authority, location) = remainder.split_at(authority_end);
        let path = location.split(['?', '#']).next().unwrap_or_default();
        Self::public_host(authority)
            && path.starts_with('/')
            && path.chars().any(|character| character != '/')
    }

    /// Reports a present URL that is unsuitable for durable documentation.
    fn check_url(cx: &LateContext<'_>, span: Span, url: Option<&str>) {
        // Diagnostics without URL metadata have no link stability contract.
        let Some(url) = url else {
            return;
        };

        // Stable public HTTPS links already satisfy the documentation contract.
        if Self::stable_url(url) {
            return;
        }

        Violation {
            span,
            url: url.to_owned(),
        }
        .emit(cx);
    }
}
impl LateLintPass<'_> for MietteUnstableDiagnosticUrls {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.derived_contracts() {
            Self::check_url(cx, contract.span, contract.metadata.url.as_deref());
            for member in &contract.members {
                Self::check_url(cx, member.span, member.metadata.url.as_deref());
            }
        }
    }
}
