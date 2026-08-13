extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Item, ItemKind, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::{builder_attribute, builder_attribute_contains};
use crate::utils::diagnostic::EarlyViolation;

struct Violation {
    span: Span,
    positional: usize,
    total: usize,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Bon builder keeps {} of {} members positional",
            self.positional, self.total
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "a parameter-heavy builder loses its named-argument value when most inputs remain positional at its start or finish boundary",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "keep only a small identity-bearing prefix or terminal value positional and expose the remaining inputs as named setters",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            BON_REDUNDANT_POSITIONAL_AND_BUILDER_APIS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this builder retains a positional-heavy call contract",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct BonRedundantPositionalAndBuilderApis;

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_REDUNDANT_POSITIONAL_AND_BUILDER_APIS,
    Warn,
    "rejects positional-heavy public Bon builders",
    BonRedundantPositionalAndBuilderApis
}

impl EarlyLintPass for BonRedundantPositionalAndBuilderApis {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        let ItemKind::Fn(function) = &item.kind else {
            return;
        };
        let Some(attribute) = builder_attribute(&item.attrs) else {
            return;
        };
        if !matches!(item.vis.kind, VisibilityKind::Public) {
            return;
        }
        let total = function.sig.decl.inputs.len();
        let positional = function
            .sig
            .decl
            .inputs
            .iter()
            .filter(|parameter| {
                builder_attribute_contains(cx, &parameter.attrs, "start_fn")
                    || builder_attribute_contains(cx, &parameter.attrs, "finish_fn")
            })
            .count();
        if total >= 5 && positional >= 3 && positional * 2 >= total {
            Violation {
                span: attribute.span,
                positional,
                total,
            }
            .emit(cx);
        }
    }
}
