extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Item, ItemKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;
use syn::visit::{self, Visit};

use crate::config::leptos::LeptosServerAuthorizationConfig;
use crate::config::store::ConfigStore;
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Sensitive server endpoint without authorization evidence
// -----------------------------------------------------------------------------

/// One parsed function or method call in authored order.
struct CallObservation {
    /// Terminal callable name.
    name: String,
    /// Lexical block depth, with the function body at depth one.
    block_depth: usize,
}

/// Sensitive call selected from an ordered function-body traversal.
struct SensitiveCall<'calls> {
    /// Position used to inspect preceding authorization calls.
    position: usize,
    /// Parsed call carrying the operation name and lexical depth.
    call: &'calls CallObservation,
}

/// Collects actual Rust calls without confusing comments or literals for executable code.
#[derive(Default)]
struct CallCollector {
    /// Current lexical block depth.
    block_depth: usize,
    /// Calls in authored traversal order.
    calls: Vec<CallObservation>,
}

impl CallCollector {
    /// Records one terminal callable name.
    fn record(&mut self, name: &impl ToString) {
        self.calls.push(CallObservation {
            name: name.to_string(),
            block_depth: self.block_depth,
        });
    }
}

impl<'ast> Visit<'ast> for CallCollector {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.block_depth += 1;
        visit::visit_block(self, block);
        self.block_depth -= 1;
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*call.func
            && let Some(segment) = path.path.segments.last()
        {
            self.record(&segment.ident);
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.record(&call.method);
        visit::visit_expr_method_call(self, call);
    }
}

/// Sensitive server operation reached before an authorization boundary.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Sensitive operation reached before a proven authorization boundary.
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

/// Verifies that sensitive server-function work follows an explicit authorization boundary.
struct LeptosServerFunctionsWithoutAuthorizationBoundaries {
    /// Validated project policy applied by this lint pass.
    config: LeptosServerAuthorizationConfig,
}

impl LeptosServerFunctionsWithoutAuthorizationBoundaries {
    /// Starts endpoint analysis with the project's authorization vocabulary.
    fn new() -> Self {
        Self {
            config: ConfigStore::get().leptos_server_authorization.clone(),
        }
    }

    /// Recovers the final path segment of an authored attribute.
    fn attribute_name(attribute: &rustc_ast::Attribute) -> Option<rustc_span::Symbol> {
        attribute.path().last().copied()
    }

    /// Returns whether a function carries Leptos's server endpoint attribute.
    fn is_server_function(item: &Item) -> bool {
        item.attrs.iter().any(|attribute| {
            Self::attribute_name(attribute).is_some_and(|name| name.as_str() == "server")
        })
    }

    /// Returns whether an endpoint carries one configured policy attribute.
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

    /// Finds the earliest configured sensitive operation among parsed calls.
    fn first_sensitive_call<'calls>(
        &self,
        calls: &'calls [CallObservation],
    ) -> Option<SensitiveCall<'calls>> {
        calls
            .iter()
            .enumerate()
            .find(|(_, call)| {
                self.config
                    .sensitive_call_terms
                    .iter()
                    .any(|term| term == &call.name)
            })
            .map(|(position, call)| SensitiveCall { position, call })
    }

    /// Proves that an authorization call occurs earlier in the same lexical scope.
    fn has_preceding_authorization(&self, calls: &[CallObservation], sensitive: usize) -> bool {
        calls[..sensitive].iter().any(|call| {
            call.block_depth == 1
                && self
                    .config
                    .authorization_functions
                    .iter()
                    .any(|helper| helper == &call.name)
        })
    }
}

crate::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_SERVER_FUNCTIONS_WITHOUT_AUTHORIZATION_BOUNDARIES,
    Warn,
    "requires configured authorization evidence for sensitive Leptos server endpoints",
    LeptosServerFunctionsWithoutAuthorizationBoundaries::new()
}

impl EarlyLintPass for LeptosServerFunctionsWithoutAuthorizationBoundaries {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        // Only functions can define callable server endpoints.
        if !matches!(item.kind, ItemKind::Fn { .. }) {
            return;
        }

        // Missing authored source prevents call-order and scope analysis.
        let Ok(source) = cx.sess().source_map().span_to_snippet(item.span) else {
            return;
        };

        // Ordinary functions do not expose the Leptos server endpoint boundary.
        if !Self::is_server_function(item) {
            return;
        }

        // An explicit configured marker already declares the endpoint's access policy.
        if self.has_marker(item) {
            return;
        }

        // Unparseable function text cannot provide reliable call-order evidence.
        let Ok(function) = syn::parse_str::<syn::ItemFn>(&source) else {
            return;
        };
        let mut collector = CallCollector::default();
        collector.visit_block(&function.block);

        // Endpoints without configured sensitive operations require no authorization proof.
        let Some(SensitiveCall {
            position,
            call: sensitive,
        }) = self.first_sensitive_call(&collector.calls)
        else {
            return;
        };

        // An earlier same-scope authorization call establishes the required boundary.
        if self.has_preceding_authorization(&collector.calls, position) {
            return;
        }

        Violation {
            span: item.span,
            operation: sensitive.name.clone(),
        }
        .emit(cx);
    }
}
