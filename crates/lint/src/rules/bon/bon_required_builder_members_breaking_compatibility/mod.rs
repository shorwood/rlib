extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{FieldDef, Item, ItemKind, Param, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::attributes::{BonAttributeAnalysis, OptionType};
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
            || BonAttributeAnalysis::builder_contains(cx, attributes, "default")
            || BonAttributeAnalysis::builder_contains(cx, attributes, "skip")
            || BonAttributeAnalysis::builder_contains(cx, attributes, "field")
        {
            return None;
        }
        let ty = match cx.sess().source_map().span_to_snippet(ty_span) {
            Ok(ty) => ty,
            Err(_error) => return None,
        };

        let required = !OptionType::is_option(&ty)
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
                member: member.trim(),
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
    fn check_function(&self, cx: &EarlyContext<'_>, item: &Item, function: &rustc_ast::Fn) {
        let Some(identifier) = item.kind.ident() else {
            return;
        };
        let builder = identifier.name.to_string();
        for parameter in &function.sig.decl.inputs {
            let Some(violation) =
                Self::parameter_violation(cx, &self.baseline, &builder, parameter)
            else {
                continue;
            };
            violation.emit(cx);
        }
    }
}
impl EarlyLintPass for BonRequiredBuilderMembersBreakingCompatibility {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if !matches!(item.vis.kind, VisibilityKind::Public) {
            return;
        }
        match &item.kind {
            ItemKind::Fn(function) if BonAttributeAnalysis::builder(&item.attrs).is_some() => {
                self.check_function(cx, item, function);
            }
            ItemKind::Struct(identifier, _, data)
                if BonAttributeAnalysis::derives_builder(cx, &item.attrs) =>
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
            _ => {}
        }
    }
}
