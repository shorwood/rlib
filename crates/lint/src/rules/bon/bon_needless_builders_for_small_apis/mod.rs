extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{AssocItemKind, Fn, Item, ItemKind, Param, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::attributes::{BonAttributeAnalysis, OptionType};
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

/// Callable ownership used to account for an associated receiver parameter.
#[derive(Clone, Copy)]
enum CallableOwner {
    /// Free function with no receiver convention.
    Free,
    /// Associated function whose first parameter may be a receiver.
    Associated,
}

/// Rejects builders that add no optionality or argument disambiguation.
struct BonNeedlessBuildersForSmallApis;

crate::impl_pre_expansion_lint! {
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
    fn required_distinct_parameters(cx: &EarlyContext<'_>, inputs: &[Param]) -> Option<usize> {
        // Empty and larger signatures are respectively trivial or benefit from named setters.
        if inputs.is_empty() || inputs.len() > Self::MAXIMUM_SIMPLE_PARAMETERS {
            return None;
        }

        let source_map = cx.sess().source_map();
        let mut types = Vec::with_capacity(inputs.len());
        for parameter in inputs {
            // Unavailable type syntax prevents comparison of the direct call's parameter roles.
            let ty = match source_map.span_to_snippet(parameter.ty.span) {
                Ok(ty) => ty,
                // A snippet failure leaves no authored type spelling to compare.
                Err(_error) => return None,
            };
            types.push(ty);
        }

        // Optional, repeated, or configured parameters still benefit from named setters.
        if types.iter().any(|ty| OptionType::is_option(ty))
            || (types.len() == Self::MAXIMUM_SIMPLE_PARAMETERS
                && Self::normalized_type(&types[0]) == Self::normalized_type(&types[1]))
            || inputs
                .iter()
                .any(|parameter| BonAttributeAnalysis::builder(&parameter.attrs).is_some())
        {
            return None;
        }
        Some(inputs.len())
    }

    /// Removes insignificant whitespace before comparing authored type spellings.
    fn normalized_type(ty: &str) -> String {
        ty.chars()
            .filter(|character| !character.is_whitespace())
            .collect()
    }

    /// Returns whether the first associated-function parameter is a receiver.
    fn is_receiver(cx: &EarlyContext<'_>, parameter: &Param) -> bool {
        cx.sess()
            .source_map()
            .span_to_snippet(parameter.pat.span)
            .is_ok_and(|pattern| {
                pattern
                    .split(|character: char| character != '_' && !character.is_alphanumeric())
                    .any(|token| token == "self")
            })
    }

    /// Checks one private free or associated function.
    fn check_function(
        cx: &EarlyContext<'_>,
        visibility: &rustc_ast::Visibility,
        attributes: &[rustc_ast::Attribute],
        function: &Fn,
        owner: CallableOwner,
    ) {
        // Any explicit visibility places the callable outside this private-API policy.
        if !matches!(visibility.kind, VisibilityKind::Inherited) {
            return;
        }

        // Functions without a plain Bon builder attribute expose no removable builder layer.
        let Some(span) = BonAttributeAnalysis::plain_builder(cx, attributes) else {
            return;
        };
        let inputs = &function.sig.decl.inputs;
        let inputs = if matches!(owner, CallableOwner::Associated)
            && inputs
                .first()
                .is_some_and(|input| Self::is_receiver(cx, input))
        {
            &inputs[1..]
        } else {
            inputs
        };

        // Optional, repeated, ambiguous, or larger inputs still benefit from named setters.
        let Some(parameter_count) = Self::required_distinct_parameters(cx, inputs) else {
            return;
        };
        Violation {
            span,
            parameter_count,
        }
        .emit(cx);
    }
}

impl EarlyLintPass for BonNeedlessBuildersForSmallApis {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        match &item.kind {
            ItemKind::Fn(function) => {
                Self::check_function(cx, &item.vis, &item.attrs, function, CallableOwner::Free);
            }
            ItemKind::Impl(implementation) => {
                for associated in &implementation.items {
                    let AssocItemKind::Fn(function) = &associated.kind else {
                        continue;
                    };
                    Self::check_function(
                        cx,
                        &associated.vis,
                        &associated.attrs,
                        function,
                        CallableOwner::Associated,
                    );
                }
            }
            _ => {}
        }
    }
}
