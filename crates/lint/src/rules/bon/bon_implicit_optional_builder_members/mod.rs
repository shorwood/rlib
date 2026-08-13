extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Item, ItemKind, Param};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::{builder_attribute, builder_attribute_contains, has_attribute};
use crate::utils::diagnostic::EarlyViolation;

struct Violation {
    span: Span,
    member: String,
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
            || (builder_attribute_contains(cx, &item.attrs, "on(")
                && builder_attribute_contains(cx, &item.attrs, "required"))
        {
            return;
        }
        for parameter in &function.sig.decl.inputs {
            if let Some(violation) = violation(cx, parameter) {
                violation.emit(cx);
            }
        }
    }
}

fn violation(cx: &EarlyContext<'_>, parameter: &Param) -> Option<Violation> {
    let source_map = cx.sess().source_map();
    let ty = source_map.span_to_snippet(parameter.ty.span).ok()?;
    if !is_option(&ty)
        || builder_attribute_contains(cx, &parameter.attrs, "required")
        || has_attribute(&parameter.attrs, "doc")
    {
        return None;
    }
    let member = source_map
        .span_to_snippet(parameter.pat.span)
        .ok()?
        .trim()
        .trim_start_matches("mut ")
        .trim_start_matches("ref ")
        .to_owned();
    is_policy_bearing(&member).then_some(Violation {
        span: parameter.ty.span,
        member,
    })
}

fn is_option(ty: &str) -> bool {
    let compact: String = ty
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    compact.starts_with("Option<")
        || compact.starts_with("std::option::Option<")
        || compact.starts_with("core::option::Option<")
}

fn is_policy_bearing(member: &str) -> bool {
    member.split('_').any(|word| {
        matches!(
            word,
            "authorization" | "callback" | "destination" | "limit" | "policy" | "timeout"
        )
    })
}
