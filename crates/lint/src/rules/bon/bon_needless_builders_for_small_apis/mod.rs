extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Item, ItemKind, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::plain_builder_attribute;
use crate::utils::diagnostic::EarlyViolation;

struct Violation {
    span: Span,
    parameter_count: usize,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "private Bon builder wraps only {} required parameter{}",
            self.parameter_count,
            if self.parameter_count == 1 { "" } else { "s" }
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "this small internal API gains no optionality, default, conversion, or staging benefit from generated typestate",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("remove `#[bon::builder]` and retain the direct function call")
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            BON_NEEDLESS_BUILDERS_FOR_SMALL_APIS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct BonNeedlessBuildersForSmallApis;

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_NEEDLESS_BUILDERS_FOR_SMALL_APIS,
    Warn,
    "rejects Bon builders that add no value to small private functions",
    BonNeedlessBuildersForSmallApis
}

impl BonNeedlessBuildersForSmallApis {
    fn required_distinct_parameters(cx: &EarlyContext<'_>, item: &Item) -> Option<usize> {
        let ItemKind::Fn(function) = &item.kind else {
            return None;
        };
        let inputs = &function.sig.decl.inputs;
        if inputs.is_empty() || inputs.len() > 2 {
            return None;
        }
        let source_map = cx.sess().source_map();
        let mut types = Vec::with_capacity(inputs.len());
        for parameter in inputs {
            types.push(source_map.span_to_snippet(parameter.ty.span).ok()?);
        }
        if types
            .iter()
            .any(|ty| ty.trim_start().starts_with("Option<"))
            || (types.len() == 2 && types[0] == types[1])
            || inputs.iter().any(|parameter| !parameter.attrs.is_empty())
        {
            return None;
        }
        Some(inputs.len())
    }
}

impl EarlyLintPass for BonNeedlessBuildersForSmallApis {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if !matches!(item.vis.kind, VisibilityKind::Inherited) {
            return;
        }
        let Some(span) = plain_builder_attribute(cx, &item.attrs) else {
            return;
        };
        let Some(parameter_count) = Self::required_distinct_parameters(cx, item) else {
            return;
        };
        Violation {
            span,
            parameter_count,
        }
        .emit(cx);
    }
}
