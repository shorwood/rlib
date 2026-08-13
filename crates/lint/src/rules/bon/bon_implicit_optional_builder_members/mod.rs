extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Item, ItemKind, Param};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::{BonAttributeAnalysis, builder_attribute, has_attribute, is_option_type};
use crate::utils::diagnostic::EarlyViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `member` value used by this analysis.
    member: String,
}

impl Violation {
    /// Builds a violation for an undocumented policy-bearing optional parameter.
    fn from_parameter(cx: &EarlyContext<'_>, parameter: &Param) -> Option<Self> {
        // Prepare the values used by this stage.
        let source_map = cx.sess().source_map();
        let ty = match source_map.span_to_snippet(parameter.ty.span) {
            Ok(ty) => ty,
            Err(_error) => return None,
        };

        // Reject inputs that do not satisfy this stage.
        if !is_option_type(&ty)
            || BonAttributeAnalysis::builder_contains(cx, &parameter.attrs, "required")
            || has_attribute(&parameter.attrs, "doc")
        {
            return None;
        }

        // Prepare the values used by this stage.
        let member = match source_map.span_to_snippet(parameter.pat.span) {
            Ok(member) => member,
            Err(_error) => return None,
        }
        .trim()
        .trim_start_matches("mut ")
        .trim_start_matches("ref ")
        .to_owned();

        // Perform the next step of the analysis.
        Self::is_policy_bearing(&member).then_some(Self {
            span: parameter.ty.span,
            member,
        })
    }

    /// Returns whether a member name carries caller-selected policy.
    fn is_policy_bearing(member: &str) -> bool {
        member.split('_').any(|word| {
            matches!(
                word,
                "authorization" | "callback" | "destination" | "limit" | "policy" | "timeout"
            )
        })
    }
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "policy-bearing Bon member `{}` is implicitly optional",
            self.member
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "Bon makes `Option<T>` members omittable, which can hide whether the caller consciously selected the absence policy",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "add `#[builder(required)]` to require an explicit `Option`, document why omission is intentional, or use a semantic policy enum",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            BON_IMPLICIT_OPTIONAL_BUILDER_MEMBERS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this `Option` member may be omitted");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Carries the `BonImplicitOptionalBuilderMembers` state used by this analysis.
struct BonImplicitOptionalBuilderMembers;

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_IMPLICIT_OPTIONAL_BUILDER_MEMBERS,
    Warn,
    "requires explicit omission policy for policy-bearing optional Bon members",
    BonImplicitOptionalBuilderMembers
}

impl EarlyLintPass for BonImplicitOptionalBuilderMembers {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        let ItemKind::Fn(function) = &item.kind else {
            return;
        };
        if builder_attribute(&item.attrs).is_none()
            || (BonAttributeAnalysis::builder_contains(cx, &item.attrs, "on(")
                && BonAttributeAnalysis::builder_contains(cx, &item.attrs, "required"))
        {
            return;
        }
        for parameter in &function.sig.decl.inputs {
            let Some(violation) = Violation::from_parameter(cx, parameter) else {
                continue;
            };
            violation.emit(cx);
        }
    }
}
