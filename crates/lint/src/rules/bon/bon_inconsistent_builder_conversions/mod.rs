extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashMap;

use rustc_ast::ast::{Item, ItemKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::{BonAttributeAnalysis, builder_attribute};
use crate::utils::diagnostic::EarlyViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `ty` value used by this analysis.
    ty: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Bon members of type `{}` use inconsistent conversion policy",
            self.ty
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "callers should not need to memorize which same-representation setters accept `Into` conversions",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "apply `#[builder(into)]` coherently or make the stricter member's distinct validation policy explicit",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            BON_INCONSISTENT_BUILDER_CONVERSIONS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this member remains strict while its peer accepts `Into`",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// One same-typed member and whether it opts into `Into` conversion.
struct ConversionMember {
    /// Whether the member accepts values through `Into`.
    has_into: bool,
    /// Authored member type span.
    span: Span,
}

/// Carries the `BonInconsistentBuilderConversions` state used by this analysis.
struct BonInconsistentBuilderConversions;

impl BonInconsistentBuilderConversions {
    /// Minimum same-typed members needed to compare conversion policy.
    const MINIMUM_COMPARABLE_MEMBERS: usize = 2;
}

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_INCONSISTENT_BUILDER_CONVERSIONS,
    Warn,
    "rejects inconsistent Into policy among same-typed Bon members",
    BonInconsistentBuilderConversions
}

impl EarlyLintPass for BonInconsistentBuilderConversions {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        let ItemKind::Fn(function) = &item.kind else {
            return;
        };
        if builder_attribute(&item.attrs).is_none()
            || BonAttributeAnalysis::builder_contains(cx, &item.attrs, "on(")
        {
            return;
        }
        let source_map = cx.sess().source_map();
        let mut groups = HashMap::<String, Vec<ConversionMember>>::new();
        for parameter in &function.sig.decl.inputs {
            // Prepare the values used by this stage.
            let Ok(ty) = source_map.span_to_snippet(parameter.ty.span) else {
                continue;
            };
            let has_custom_conversion = builder_attribute(&parameter.attrs).is_some()
                && !BonAttributeAnalysis::builder_contains(cx, &parameter.attrs, "into");

            // Reject inputs that do not satisfy this stage.
            if has_custom_conversion {
                continue;
            }
            groups.entry(ty).or_default().push(ConversionMember {
                has_into: BonAttributeAnalysis::builder_contains(cx, &parameter.attrs, "into"),
                span: parameter.ty.span,
            });
        }
        for (ty, members) in groups {
            let has_into = members.iter().any(|member| member.has_into);
            let strict = members.iter().find(|member| !member.has_into);
            if !has_into || members.len() < Self::MINIMUM_COMPARABLE_MEMBERS {
                continue;
            }
            let Some(strict) = strict else { continue };
            Violation {
                span: strict.span,
                ty,
            }
            .emit(cx);
        }
    }
}
