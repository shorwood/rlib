extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{AssocItemKind, Fn, Item, ItemKind, Param};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::attributes::{BonAttributeAnalysis, OptionType};
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Policy-bearing member with implicit omission
// -----------------------------------------------------------------------------

/// Optional builder member whose absence should be a conscious caller choice.
struct Violation {
    /// Optional type receiving the diagnostic.
    span: Span,
    /// Policy-bearing member named in the diagnostic.
    member: String,
}

impl Violation {
    /// Builds a violation for an undocumented policy-bearing optional parameter.
    fn from_parameter(cx: &EarlyContext<'_>, parameter: &Param) -> Option<Self> {
        let source_map = cx.sess().source_map();
        let ty = match source_map.span_to_snippet(parameter.ty.span) {
            Ok(ty) => ty,
            // Missing authored type text prevents reliable optional-member classification.
            Err(_error) => return None,
        };

        // An explicit requirement or documentation makes omission policy visible.
        if !OptionType::is_option(&ty)
            || BonAttributeAnalysis::contains_builder_value(cx, &parameter.attrs, "required")
            || BonAttributeAnalysis::has_attribute(&parameter.attrs, "doc")
        {
            return None;
        }

        let member = match source_map.span_to_snippet(parameter.pat.span) {
            Ok(member) => member,
            // Missing authored pattern text prevents recovery of the builder member name.
            Err(_error) => return None,
        }
        .trim()
        .trim_start_matches("mut ")
        .trim_start_matches("ref ")
        .to_owned();

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

// -----------------------------------------------------------------------------
// BonImplicitOptionalBuilderMembers: Explicit omission policy
// -----------------------------------------------------------------------------

/// Requires policy-bearing optional members to declare how omission is handled.
struct BonImplicitOptionalBuilderMembers;

impl BonImplicitOptionalBuilderMembers {
    /// Checks one free or associated function carrying Bon's builder attribute.
    fn check_function(cx: &EarlyContext<'_>, attributes: &[rustc_ast::Attribute], function: &Fn) {
        // Non-builder functions and explicitly required policies need no omission warning.
        if BonAttributeAnalysis::builder(attributes).is_none()
            || (BonAttributeAnalysis::contains_builder_value(cx, attributes, "on(")
                && BonAttributeAnalysis::contains_builder_value(cx, attributes, "required"))
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

crate::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_IMPLICIT_OPTIONAL_BUILDER_MEMBERS,
    Warn,
    "requires explicit omission policy for policy-bearing optional Bon members",
    BonImplicitOptionalBuilderMembers
}

impl EarlyLintPass for BonImplicitOptionalBuilderMembers {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        match &item.kind {
            ItemKind::Fn(function) => Self::check_function(cx, &item.attrs, function),
            ItemKind::Impl(implementation) => {
                for associated in &implementation.items {
                    let AssocItemKind::Fn(function) = &associated.kind else {
                        continue;
                    };
                    Self::check_function(cx, &associated.attrs, function);
                }
            }
            _ => {}
        }
    }
}
