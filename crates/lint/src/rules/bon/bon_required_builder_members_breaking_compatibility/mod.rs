extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{FieldDef, Item, ItemKind, Param, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::config::BonApiBaselineConfig;
use super::utils::{
    builder_attribute, builder_attribute_contains, derives_bon_builder, is_option_type,
};
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::EarlyViolation;

struct Violation {
    span: Span,
    builder: String,
    member: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "new required Bon member `{}.{}` breaks baseline call sequences",
            self.builder, self.member
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "existing callers represented by the configured API snapshot cannot finish this builder without setting the new member",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "provide a meaningful default or optional contract, version the builder API, or update the baseline as part of an intentional breaking release",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            BON_REQUIRED_BUILDER_MEMBERS_BREAKING_COMPATIBILITY,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this required member is absent from the baseline",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct BonRequiredBuilderMembersBreakingCompatibility {
    baseline: BonApiBaselineConfig,
}

impl BonRequiredBuilderMembersBreakingCompatibility {
    fn new() -> Self {
        Self {
            baseline: LibraryConfig::load().bon_api_baseline,
        }
    }
}

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_REQUIRED_BUILDER_MEMBERS_BREAKING_COMPATIBILITY,
    Warn,
    "finds required Bon members added beyond a configured API baseline",
    BonRequiredBuilderMembersBreakingCompatibility::new()
}

impl EarlyLintPass for BonRequiredBuilderMembersBreakingCompatibility {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if !matches!(item.vis.kind, VisibilityKind::Public) {
            return;
        }
        match &item.kind {
            ItemKind::Fn(function) if builder_attribute(&item.attrs).is_some() => {
                let Some(identifier) = item.kind.ident() else {
                    return;
                };
                let builder = identifier.name.to_string();
                for parameter in &function.sig.decl.inputs {
                    if let Some(violation) =
                        parameter_violation(cx, &self.baseline, &builder, parameter)
                    {
                        violation.emit(cx);
                    }
                }
            }
            ItemKind::Struct(identifier, _, data) if derives_bon_builder(cx, &item.attrs) => {
                let builder = identifier.name.to_string();
                for field in data.fields() {
                    if let Some(violation) = field_violation(cx, &self.baseline, &builder, field) {
                        violation.emit(cx);
                    }
                }
            }
            _ => {}
        }
    }
}

fn parameter_violation(
    cx: &EarlyContext<'_>,
    baseline: &BonApiBaselineConfig,
    builder: &str,
    parameter: &Param,
) -> Option<Violation> {
    let member = cx
        .sess()
        .source_map()
        .span_to_snippet(parameter.pat.span)
        .ok()?;
    member_violation(
        cx,
        baseline,
        builder,
        member.trim(),
        parameter.ty.span,
        &parameter.attrs,
    )
}

fn field_violation(
    cx: &EarlyContext<'_>,
    baseline: &BonApiBaselineConfig,
    builder: &str,
    field: &FieldDef,
) -> Option<Violation> {
    member_violation(
        cx,
        baseline,
        builder,
        field.ident?.name.as_str(),
        field.ty.span,
        &field.attrs,
    )
}

fn member_violation(
    cx: &EarlyContext<'_>,
    baseline: &BonApiBaselineConfig,
    builder: &str,
    member: &str,
    ty_span: Span,
    attributes: &[rustc_ast::Attribute],
) -> Option<Violation> {
    if !baseline.contains_builder(builder)
        || baseline.contains_member(builder, member)
        || builder_attribute_contains(cx, attributes, "default")
        || builder_attribute_contains(cx, attributes, "skip")
        || builder_attribute_contains(cx, attributes, "field")
    {
        return None;
    }
    let ty = cx.sess().source_map().span_to_snippet(ty_span).ok()?;
    let required = !is_option_type(&ty) || builder_attribute_contains(cx, attributes, "required");
    required.then_some(Violation {
        span: ty_span,
        builder: builder.to_owned(),
        member: member.to_owned(),
    })
}
