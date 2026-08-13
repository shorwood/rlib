extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Item, ItemKind, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::attributes::BonAttributeAnalysis;
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Builder without a naming or staging benefit
// -----------------------------------------------------------------------------

/// Small private API wrapped in unnecessary generated typestate.
struct Violation {
    /// Bon attribute that enables the unnecessary builder.
    span: Span,
    /// Number of required parameters already clear at the call site.
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

// -----------------------------------------------------------------------------
// BonNeedlessBuildersForSmallApis: Small private API policy
// -----------------------------------------------------------------------------

/// Rejects builders that add no optionality or argument disambiguation.
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

    /// Counts required parameters when their direct call remains unambiguous.
    fn required_distinct_parameters(cx: &EarlyContext<'_>, item: &Item) -> Option<usize> {
        let ItemKind::Fn(function) = &item.kind else {
            return None;
        };
        let inputs = &function.sig.decl.inputs;
        if inputs.is_empty() || inputs.len() > Self::MAXIMUM_SIMPLE_PARAMETERS {
            return None;
        }

        let source_map = cx.sess().source_map();
        let mut types = Vec::with_capacity(inputs.len());
        for parameter in inputs {
            let ty = match source_map.span_to_snippet(parameter.ty.span) {
                Ok(ty) => ty,
                Err(_error) => return None,
            };
            types.push(ty);
        }

        // Optional, repeated, or configured parameters still benefit from named setters.
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
        if !matches!(item.vis.kind, VisibilityKind::Inherited) {
            return;
        }
        let Some(span) = BonAttributeAnalysis::plain_builder(cx, &item.attrs) else {
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
