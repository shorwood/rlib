extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{AssocItemKind, FieldDef, Item, ItemKind, Param, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::attributes::{BonAttributeAnalysis, BuilderOption, OptionType};
use super::utils::config::{BonApiBaselineConfig, BonMemberPath};
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: New required member outside the API baseline
// -----------------------------------------------------------------------------

/// Required builder member that existing baseline call sequences cannot supply.
struct Violation {
    /// New member type receiving the diagnostic.
    span: Span,
    /// Public builder containing the incompatible addition.
    builder: String,
    /// Newly required member.
    member: String,
}

impl Violation {
    /// Compares one member with the configured public API baseline.
    fn member_violation(
        cx: &EarlyContext<'_>,
        baseline: &BonApiBaselineConfig,
        member_path: BonMemberPath<'_>,
        ty_span: Span,
        attributes: &[rustc_ast::Attribute],
    ) -> Option<Self> {
        // Existing, defaulted, skipped, and privately initialized members remain compatible.
        if !baseline.contains_builder(member_path.builder)
            || baseline.contains_member(member_path)
            || BonAttributeAnalysis::builder_has_option(cx, attributes, BuilderOption::DEFAULT)
            || BonAttributeAnalysis::builder_has_option(cx, attributes, BuilderOption::SKIP)
            || BonAttributeAnalysis::builder_has_option(cx, attributes, BuilderOption::FIELD)
        {
            return None;
        }
        let ty = match cx.sess().source_map().span_to_snippet(ty_span) {
            Ok(ty) => ty,
            Err(_error) => return None,
        };

        let required = !OptionType::is_option(&ty)
            || BonAttributeAnalysis::builder_has_option(cx, attributes, BuilderOption::REQUIRED);
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

// -----------------------------------------------------------------------------
// BonRequiredBuilderMembersBreakingCompatibility: Baseline compatibility policy
// -----------------------------------------------------------------------------

/// Protects configured public builders from newly mandatory call steps.
struct BonRequiredBuilderMembersBreakingCompatibility {
    /// Previously published builders and members used as the compatibility baseline.
    baseline: BonApiBaselineConfig,
}

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_REQUIRED_BUILDER_MEMBERS_BREAKING_COMPATIBILITY,
    Warn,
    "finds required Bon members added beyond a configured API baseline",
    BonRequiredBuilderMembersBreakingCompatibility::new()
}

impl BonRequiredBuilderMembersBreakingCompatibility {
    /// Loads the configured Bon API baseline.
    fn new() -> Self {
        Self {
            baseline: LibraryConfig::load().bon_api_baseline,
        }
    }

    /// Adapts a function parameter to the shared baseline comparison.
    fn parameter_violation(
        cx: &EarlyContext<'_>,
        baseline: &BonApiBaselineConfig,
        builder: &str,
        parameter: &Param,
    ) -> Option<Violation> {
        let member = match cx.sess().source_map().span_to_snippet(parameter.pat.span) {
            Ok(member) => member,
            Err(_error) => return None,
        };

        Violation::member_violation(
            cx,
            baseline,
            BonMemberPath {
                builder,
                member: member
                    .trim()
                    .trim_start_matches("mut ")
                    .trim_start_matches("ref "),
            },
            parameter.ty.span,
            &parameter.attrs,
        )
    }

    /// Adapts a struct field to the shared baseline comparison.
    fn field_violation(
        cx: &EarlyContext<'_>,
        baseline: &BonApiBaselineConfig,
        builder: &str,
        field: &FieldDef,
    ) -> Option<Violation> {
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

    /// Checks required parameters on one public builder function.
    fn check_function(&self, cx: &EarlyContext<'_>, builder: &str, function: &rustc_ast::Fn) {
        for parameter in &function.sig.decl.inputs {
            let Some(violation) = Self::parameter_violation(cx, &self.baseline, builder, parameter)
            else {
                continue;
            };
            violation.emit(cx);
        }
    }

    /// Checks public builder methods declared in one inherent implementation.
    fn check_implementation(&self, cx: &EarlyContext<'_>, implementation: &rustc_ast::Impl) {
        let Ok(owner) = cx
            .sess()
            .source_map()
            .span_to_snippet(implementation.self_ty.span)
        else {
            return;
        };
        for associated in &implementation.items {
            let AssocItemKind::Fn(function) = &associated.kind else {
                continue;
            };
            if !matches!(associated.vis.kind, VisibilityKind::Public)
                || BonAttributeAnalysis::builder(&associated.attrs).is_none()
            {
                continue;
            }
            let Some(identifier) = associated.kind.ident() else {
                continue;
            };
            let builder = format!("{}::{}", owner.trim(), identifier.name);
            self.check_function(cx, &builder, function);
        }
    }
}
impl EarlyLintPass for BonRequiredBuilderMembersBreakingCompatibility {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        match &item.kind {
            ItemKind::Fn(function)
                if matches!(item.vis.kind, VisibilityKind::Public)
                    && BonAttributeAnalysis::builder(&item.attrs).is_some() =>
            {
                let Some(identifier) = item.kind.ident() else {
                    return;
                };
                self.check_function(cx, identifier.name.as_str(), function);
            }
            ItemKind::Struct(identifier, _, data)
                if matches!(item.vis.kind, VisibilityKind::Public)
                    && BonAttributeAnalysis::derives_builder(cx, &item.attrs) =>
            {
                let builder = identifier.name.to_string();
                for field in data.fields() {
                    let Some(violation) =
                        Self::field_violation(cx, &self.baseline, &builder, field)
                    else {
                        continue;
                    };
                    violation.emit(cx);
                }
            }
            ItemKind::Impl(implementation) => self.check_implementation(cx, implementation),
            _ => {}
        }
    }
}
