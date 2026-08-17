extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Crate, Item};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use crate::rules::leptos::utils::authored_files::AuthoredFiles;
use crate::rules::leptos::utils::component_architecture::{
    ArchitectureAnalysis, LeptosArchitectureConfig,
};
use crate::utils::diagnostic::EarlyViolation;

struct Violation {
    span: Span,
    statements: usize,
    statement_limit: usize,
    depth: usize,
    depth_limit: usize,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "event handler has {} statements and control-flow depth {}; limits are {} and {}",
            self.statements, self.depth, self.statement_limit, self.depth_limit
        ))
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "event bindings should declare intent while testable application logic lives behind a named operation",
        )
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "move the handler body into a named action or composable operation and keep the binding declarative",
        )
    }
    fn emit(self, cx: &EarlyContext<'_>) {
        let p = self.primary_message().into_owned();
        let r = self.rationale_message().into_owned();
        let h = self.remediation_message().into_owned();
        cx.emit_span_lint(
            LEPTOS_OVERSIZED_EVENT_HANDLERS,
            self.span,
            DiagDecorator(|d| {
                d.primary_message(p);
                d.note(r);
                d.help(h);
            }),
        );
    }
}

struct LeptosOversizedEventHandlers {
    config: LeptosArchitectureConfig,
    files: AuthoredFiles,
}

dylint_linting::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_OVERSIZED_EVENT_HANDLERS,Warn,
    "rejects complex Leptos event handlers",
    LeptosOversizedEventHandlers{config:LeptosArchitectureConfig::from_config(),files:AuthoredFiles::default()}
}
impl EarlyLintPass for LeptosOversizedEventHandlers {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        self.files.observe_item(cx, item);
    }
    fn check_crate_post(&mut self, cx: &EarlyContext<'_>, _: &Crate) {
        for f in ArchitectureAnalysis::analyze(self.files.documents(cx)).functions {
            if !f.is_reactive_owner() {
                continue;
            }
            for handler in f.handlers {
                if handler.statements > self.config.max_handler_statements
                    || handler.control_depth > self.config.max_handler_control_flow_depth
                {
                    Violation {
                        span: handler.span,
                        statements: handler.statements,
                        statement_limit: self.config.max_handler_statements,
                        depth: handler.control_depth,
                        depth_limit: self.config.max_handler_control_flow_depth,
                    }
                    .emit(cx);
                }
            }
        }
    }
}
