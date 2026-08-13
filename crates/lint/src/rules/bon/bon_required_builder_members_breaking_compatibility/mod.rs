extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{FieldDef, Item, ItemKind, Param, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::config::{BonApiBaselineConfig, BonMemberPath};
use super::utils::{BonAttributeAnalysis, builder_attribute, is_option_type};
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::EarlyViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `builder` value used by this analysis.
    builder: String,
    /// Stores the `member` value used by this analysis.
    member: String,
}

impl Violation {
    /// Performs the `member_violation` step of the lint analysis.
    fn member_violation(
        cx: &EarlyContext<'_>,
        baseline: &BonApiBaselineConfig,
        member_path: BonMemberPath<'_>,
        ty_span: Span,
        attributes: &[rustc_ast::Attribute],
    ) -> Option<Self> {
        // Reject inputs that do not satisfy this stage.
        if !baseline.contains_builder(member_path.builder)
            || baseline.contains_member(member_path)
            || BonAttributeAnalysis::builder_contains(cx, attributes, "default")
            || BonAttributeAnalysis::builder_contains(cx, attributes, "skip")
            || BonAttributeAnalysis::builder_contains(cx, attributes, "field")
        // Perform the next step of the analysis.
        {
            return None;
        }
        let ty = match cx.sess().source_map().span_to_snippet(ty_span) {
            Ok(ty) => ty,
            Err(_error) => return None,
        };

        // Prepare the values used by this stage.
        let required = !is_option_type(&ty)
            || BonAttributeAnalysis::builder_contains(cx, attributes, "required");
        required.then_some(Self {
            span: ty_span,
            builder: member_path.builder.to_owned(),
            member: member_path.member.to_owned(),
        })
    }
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

/// Performs the `parameter_violation` step of the lint analysis.
fn parameter_violation(
    cx: &EarlyContext<'_>,
    baseline: &BonApiBaselineConfig,
    builder: &str,
    parameter: &Param,
) -> Option<Violation> {
    // Prepare the values used by this stage.
    let member = match cx.sess().source_map().span_to_snippet(parameter.pat.span) {
        Ok(member) => member,
        Err(_error) => return None,
    };

    // Perform the next step of the analysis.
    Violation::member_violation(
        cx,
        baseline,
        BonMemberPath {
            builder,
            member: member.trim(),
        },
        parameter.ty.span,
        &parameter.attrs,
    )
}

/// Performs the `field_violation` step of the lint analysis.
fn field_violation(
    cx: &EarlyContext<'_>,
    baseline: &BonApiBaselineConfig,
    builder: &str,
    field: &FieldDef,
) -> Option<Violation> {
    // Perform the next step of the analysis.
    Violation::member_violation(
        cx,
        baseline,
        BonMemberPath {
            builder,
            member: field.ident?.name.as_str(),
        },
        field.ty.span,
        &field.attrs,
    )
}

/// Carries the `BonRequiredBuilderMembersBreakingCompatibility` state used by this analysis.
struct BonRequiredBuilderMembersBreakingCompatibility {
    /// Stores the `baseline` value used by this analysis.
    baseline: BonApiBaselineConfig,
}

impl BonRequiredBuilderMembersBreakingCompatibility {
    /// Performs the `new` operation for this value.
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
                    let Some(violation) =
                        parameter_violation(cx, &self.baseline, &builder, parameter)
                    else {
                        continue;
                    };
                    violation.emit(cx);
                }
            }
            ItemKind::Struct(identifier, _, data)
                if BonAttributeAnalysis::derives_builder(cx, &item.attrs) =>
            {
                let builder = identifier.name.to_string();
                for field in data.fields() {
                    let Some(violation) = field_violation(cx, &self.baseline, &builder, field)
                    else {
                        continue;
                    };
                    violation.emit(cx);
                }
            }
            _ => {}
        }
    }
}
