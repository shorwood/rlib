extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Item, ItemKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;
use serde::Deserialize;

use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// LeptosServerAuthorizationConfig: Application security vocabulary
// -----------------------------------------------------------------------------

#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "snake_case")]
pub struct LeptosServerAuthorizationConfig {
    sensitive_call_terms: Vec<String>,
    authorization_functions: Vec<String>,
    protected_endpoint_attributes: Vec<String>,
    public_endpoint_attributes: Vec<String>,
}

impl Default for LeptosServerAuthorizationConfig {
    fn default() -> Self {
        Self {
            sensitive_call_terms: [
                "create",
                "delete",
                "remove",
                "update",
                "upload",
                "credential",
                "private",
                "admin",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            authorization_functions: Vec::new(),
            protected_endpoint_attributes: Vec::new(),
            public_endpoint_attributes: Vec::new(),
        }
    }
}

impl LeptosServerAuthorizationConfig {
    fn from_config() -> Self {
        let config = LibraryConfig::load().leptos_server_authorization;
        config.validate().unwrap_or_else(|message| {
            panic!("invalid Leptos server authorization configuration: {message}")
        });
        config
    }

    fn validate(&self) -> Result<(), String> {
        let entries = self
            .sensitive_call_terms
            .iter()
            .chain(&self.authorization_functions)
            .chain(&self.protected_endpoint_attributes)
            .chain(&self.public_endpoint_attributes);
        if entries.clone().any(|entry| entry.trim().is_empty()) {
            return Err("authorization vocabulary entries must not be empty".to_owned());
        }
        if self.sensitive_call_terms.is_empty() {
            return Err("sensitive_call_terms must contain at least one call term".to_owned());
        }
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Violation: Sensitive public endpoint
// -----------------------------------------------------------------------------

struct Violation {
    span: Span,
    operation: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Leptos server endpoint reaches sensitive `{}` without an authorization boundary",
            self.operation
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "server functions are public API endpoints, so local-looking calls still require explicit application security policy",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "invoke a configured authorization helper first or add a configured protected/public endpoint marker",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            LEPTOS_SERVER_FUNCTIONS_WITHOUT_AUTHORIZATION_BOUNDARIES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this endpoint has no configured policy evidence");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosServerFunctionsWithoutAuthorizationBoundaries: Endpoint policy
// -----------------------------------------------------------------------------

struct LeptosServerFunctionsWithoutAuthorizationBoundaries {
    config: LeptosServerAuthorizationConfig,
}

impl LeptosServerFunctionsWithoutAuthorizationBoundaries {
    fn new() -> Self {
        Self {
            config: LeptosServerAuthorizationConfig::from_config(),
        }
    }

    fn attribute_name(attribute: &rustc_ast::Attribute) -> Option<rustc_span::Symbol> {
        attribute.path().last().copied()
    }

    fn is_server_function(item: &Item) -> bool {
        item.attrs.iter().any(|attribute| {
            Self::attribute_name(attribute).is_some_and(|name| name.as_str() == "server")
        })
    }

    fn has_marker(&self, item: &Item) -> bool {
        self.config
            .protected_endpoint_attributes
            .iter()
            .chain(&self.config.public_endpoint_attributes)
            .any(|marker| {
                item.attrs.iter().any(|attribute| {
                    Self::attribute_name(attribute).is_some_and(|name| name.as_str() == marker)
                })
            })
    }

    fn call_position(source: &str, term: &str) -> Option<usize> {
        source.match_indices(term).find_map(|(position, _)| {
            source[position + term.len()..]
                .trim_start()
                .starts_with('(')
                .then_some(position)
        })
    }

    fn first_sensitive_call(&self, body: &str) -> Option<(usize, String)> {
        self.config
            .sensitive_call_terms
            .iter()
            .filter_map(|term| Self::call_position(body, term).map(|position| (position, term)))
            .min_by_key(|(position, _)| *position)
            .map(|(position, term)| (position, term.clone()))
    }

    fn authorization_precedes(&self, body: &str, sensitive: usize) -> bool {
        self.config.authorization_functions.iter().any(|helper| {
            Self::call_position(body, helper).is_some_and(|position| {
                position < sensitive && Self::brace_depth(&body[..position]) == 1
            })
        })
    }

    fn brace_depth(source: &str) -> usize {
        source.bytes().fold(0_usize, |depth, byte| match byte {
            b'{' => depth + 1,
            b'}' => depth.saturating_sub(1),
            _ => depth,
        })
    }
}

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_SERVER_FUNCTIONS_WITHOUT_AUTHORIZATION_BOUNDARIES,
    Warn,
    "requires configured authorization evidence for sensitive Leptos server endpoints",
    LeptosServerFunctionsWithoutAuthorizationBoundaries::new()
}

impl EarlyLintPass for LeptosServerFunctionsWithoutAuthorizationBoundaries {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if !matches!(item.kind, ItemKind::Fn { .. }) {
            return;
        }
        let Ok(source) = cx.sess().source_map().span_to_snippet(item.span) else {
            return;
        };
        if !Self::is_server_function(item) {
            return;
        }
        if self.has_marker(item) {
            return;
        }
        let Some(body_start) = source.find('{') else {
            return;
        };
        let body = &source[body_start..];
        let Some((position, operation)) = self.first_sensitive_call(body) else {
            return;
        };
        if self.authorization_precedes(body, position) {
            return;
        }
        Violation {
            span: item.span,
            operation,
        }
        .emit(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::LeptosServerAuthorizationConfig;

    #[test]
    fn rejects_empty_security_vocabulary() {
        let config = LeptosServerAuthorizationConfig {
            sensitive_call_terms: Vec::new(),
            ..LeptosServerAuthorizationConfig::default()
        };
        assert!(config.validate().is_err());
    }
}
