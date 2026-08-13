extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Item, ItemKind, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::{BonAttributeAnalysis, builder_attribute};
use crate::utils::diagnostic::EarlyViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `positional` value used by this analysis.
    positional: usize,
    /// Stores the `total` value used by this analysis.
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

/// Carries the `BonRedundantPositionalAndBuilderApis` state used by this analysis.
struct BonRedundantPositionalAndBuilderApis;

impl BonRedundantPositionalAndBuilderApis {
    /// Minimum total members that make positional builder entry points hard to read.
    const MINIMUM_TOTAL_MEMBERS: usize = 5;

    /// Minimum positional members required before evaluating their share.
    const MINIMUM_POSITIONAL_MEMBERS: usize = 3;

    /// Denominator for the maximum one-half positional share.
    const POSITIONAL_SHARE_DENOMINATOR: usize = 2;
}

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_REDUNDANT_POSITIONAL_AND_BUILDER_APIS,
    Warn,
    "rejects positional-heavy public Bon builders",
    BonRedundantPositionalAndBuilderApis
}

impl EarlyLintPass for BonRedundantPositionalAndBuilderApis {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        // Prepare the values used by this stage.
        let ItemKind::Fn(function) = &item.kind else {
            return;
        };
        let Some(attribute) = builder_attribute(&item.attrs) else {
            return;
        };

        // Reject inputs that do not satisfy this stage.
        if !matches!(item.vis.kind, VisibilityKind::Public) {
            return;
        }
        let total = function.sig.decl.inputs.len();

        // Prepare the values used by this stage.
        let positional = function
            .sig
            .decl
            .inputs
            .iter()
            .filter(|parameter| {
                BonAttributeAnalysis::builder_contains(cx, &parameter.attrs, "start_fn")
                    || BonAttributeAnalysis::builder_contains(cx, &parameter.attrs, "finish_fn")
            })
            .count();

        // Reject inputs that do not satisfy this stage.
        if !(total >= Self::MINIMUM_TOTAL_MEMBERS
            && positional >= Self::MINIMUM_POSITIONAL_MEMBERS
            && positional * Self::POSITIONAL_SHARE_DENOMINATOR >= total)
        {
            return;
        }
        Violation {
            span: attribute.span,
            positional,
            total,
        }
        .emit(cx);
    }
}
