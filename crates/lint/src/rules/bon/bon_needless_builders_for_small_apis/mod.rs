extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Item, ItemKind, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::BonAttributeAnalysis;
use crate::utils::diagnostic::EarlyViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `parameter_count` value used by this analysis.
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

/// Carries the `BonNeedlessBuildersForSmallApis` state used by this analysis.
struct BonNeedlessBuildersForSmallApis;

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_NEEDLESS_BUILDERS_FOR_SMALL_APIS,
    Warn,
    "rejects Bon builders that add no value to small private functions",
    BonNeedlessBuildersForSmallApis
}

impl BonNeedlessBuildersForSmallApis {
    /// Largest private required-only API for which a builder adds no naming value.
    const MAXIMUM_SIMPLE_PARAMETERS: usize = 2;

    /// Performs the `required_distinct_parameters` operation for this value.
    fn required_distinct_parameters(cx: &EarlyContext<'_>, item: &Item) -> Option<usize> {
        // Prepare the values used by this stage.
        let ItemKind::Fn(function) = &item.kind else {
            return None;
        };
        let inputs = &function.sig.decl.inputs;
        if inputs.is_empty() || inputs.len() > Self::MAXIMUM_SIMPLE_PARAMETERS {
            return None;
        }

        // Prepare the values used by this stage.
        let source_map = cx.sess().source_map();
        let mut types = Vec::with_capacity(inputs.len());

        // Process the candidates handled by this stage.
        for parameter in inputs {
            let ty = match source_map.span_to_snippet(parameter.ty.span) {
                Ok(ty) => ty,
                Err(_error) => return None,
            };
            types.push(ty);
        }

        // Reject inputs that do not satisfy this stage.
        if types
            .iter()
            .any(|ty| ty.trim_start().starts_with("Option<"))
            || (types.len() == Self::MAXIMUM_SIMPLE_PARAMETERS && types[0] == types[1])
            || inputs.iter().any(|parameter| !parameter.attrs.is_empty())
        {
            return None;
        }
        Some(inputs.len())
    }
}

impl EarlyLintPass for BonNeedlessBuildersForSmallApis {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        // Reject inputs that do not satisfy this stage.
        if !matches!(item.vis.kind, VisibilityKind::Inherited) {
            return;
        }
        let Some(span) = BonAttributeAnalysis::plain_builder(cx, &item.attrs) else {
            return;
        };

        // Prepare the values used by this stage.
        let Some(parameter_count) = Self::required_distinct_parameters(cx, item) else {
            return;
        };

        // Perform the next step of the analysis.
        Violation {
            span,
            parameter_count,
        }
        .emit(cx);
    }
}
