extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashMap;

use rustc_ast::ast::{AssocItemKind, Fn, Item, ItemKind, VariantData};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::attributes::{BonAttributeAnalysis, BuilderOption};
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Inconsistent conversion among same-typed members
// -----------------------------------------------------------------------------

/// Strict builder member whose peers accept values through `Into`.
struct Violation {
    /// Strict member type receiving the diagnostic.
    span: Span,
    /// Shared representation named in the diagnostic.
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

// -----------------------------------------------------------------------------
// ConversionMember: Comparable setter evidence
// -----------------------------------------------------------------------------

/// One same-typed member and whether it opts into `Into` conversion.
struct ConversionMember {
    /// Whether the member accepts values through `Into`.
    has_into: bool,
    /// Authored member type span.
    span: Span,
}

// -----------------------------------------------------------------------------
// BuilderMember: Authored member input
// -----------------------------------------------------------------------------

/// Authored builder member attributes paired with its type span.
struct BuilderMember<'member> {
    /// Attributes controlling conversion behavior.
    attributes: &'member [rustc_ast::Attribute],
    /// Authored member type used for grouping.
    type_span: Span,
}

// -----------------------------------------------------------------------------
// BonInconsistentBuilderConversions: Same-type conversion policy
// -----------------------------------------------------------------------------

/// Compares conversion behavior among members with the same representation.
struct BonInconsistentBuilderConversions;

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_INCONSISTENT_BUILDER_CONVERSIONS,
    Warn,
    "rejects inconsistent Into policy among same-typed Bon members",
    BonInconsistentBuilderConversions
}

impl BonInconsistentBuilderConversions {
    /// Minimum same-typed members needed to compare conversion policy.
    const MINIMUM_COMPARABLE_MEMBERS: usize = 2;

    /// Compares conversion policy for one collection of builder members.
    fn check_members<'member>(
        cx: &EarlyContext<'_>,
        members: impl IntoIterator<Item = BuilderMember<'member>>,
    ) {
        let source_map = cx.sess().source_map();
        let mut groups = HashMap::<String, Vec<ConversionMember>>::new();
        for BuilderMember {
            attributes,
            type_span,
        } in members
        {
            let Ok(ty) = source_map.span_to_snippet(type_span) else {
                continue;
            };
            if BonAttributeAnalysis::builder_has_option(cx, attributes, BuilderOption::WITH) {
                continue;
            }
            groups.entry(ty).or_default().push(ConversionMember {
                has_into: BonAttributeAnalysis::builder_has_option(
                    cx,
                    attributes,
                    BuilderOption::INTO,
                ),
                span: type_span,
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

    /// Checks parameters on one function builder unless an item-wide conversion policy applies.
    fn check_function(cx: &EarlyContext<'_>, attributes: &[rustc_ast::Attribute], function: &Fn) {
        // Functions without a directly inspectable builder expose no member conversion set.
        if BonAttributeAnalysis::builder(attributes).is_none()
            || BonAttributeAnalysis::builder_contains(cx, attributes, "on(")
        {
            return;
        }
        Self::check_members(
            cx,
            function
                .sig
                .decl
                .inputs
                .iter()
                .map(|parameter| BuilderMember {
                    attributes: parameter.attrs.as_slice(),
                    type_span: parameter.ty.span,
                }),
        );
    }

    /// Checks fields on one derived struct builder.
    fn check_struct(
        cx: &EarlyContext<'_>,
        attributes: &[rustc_ast::Attribute],
        data: &VariantData,
    ) {
        // Types without a directly inspectable derived builder expose no member conversion set.
        if !BonAttributeAnalysis::derives_builder(cx, attributes)
            || BonAttributeAnalysis::builder_contains(cx, attributes, "on(")
        {
            return;
        }
        Self::check_members(
            cx,
            data.fields().iter().map(|field| BuilderMember {
                attributes: field.attrs.as_slice(),
                type_span: field.ty.span,
            }),
        );
    }
}
impl EarlyLintPass for BonInconsistentBuilderConversions {
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
            ItemKind::Struct(_, _, data) => Self::check_struct(cx, &item.attrs, data),
            _ => {}
        }
    }
}
