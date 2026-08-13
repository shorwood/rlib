extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{FieldDef, Item, ItemKind, Param, VisibilityKind};
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
    /// Stores the `behavior` value used by this analysis.
    behavior: &'static str,
}

impl Violation {
    /// Performs the `member_violation` step of the lint analysis.
    fn member_violation(
        cx: &EarlyContext<'_>,
        attributes: &[rustc_ast::Attribute],
        ty_span: Span,
        member: &str,
    ) -> Option<Self> {
        // Prepare the values used by this stage.
        let builder_documents = builder_attribute(attributes).is_some_and(|attribute| {
            BonAttributeAnalysis::source(cx, attribute).is_ok_and(|source| source.contains("doc"))
        });
        if has_attribute(attributes, "doc") || builder_documents {
            return None;
        }

        // Prepare the values used by this stage.
        let ty = match cx.sess().source_map().span_to_snippet(ty_span) {
            Ok(ty) => ty,
            Err(_error) => return None,
        };
        let behavior = if is_option_type(&ty)
            && !BonAttributeAnalysis::builder_contains(cx, attributes, "required")
        // Perform the next step of the analysis.
        {
            "optional"
        } else if BonAttributeAnalysis::builder_contains(cx, attributes, "default") {
            "default"
        } else if BonAttributeAnalysis::builder_contains(cx, attributes, "into")
            || BonAttributeAnalysis::builder_contains(cx, attributes, "with")
        {
            "conversion"
        } else {
            if !BonAttributeAnalysis::builder_contains(cx, attributes, "skip")
                && !BonAttributeAnalysis::builder_contains(cx, attributes, "field")
            {
                return None;
            }
            "hidden initialization"
        };

        // Return the completed analysis result.
        Some(Self {
            span: ty_span,
            member: member.trim().to_owned(),
            behavior,
        })
    }
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "public Bon member `{}` has undocumented {} behavior",
            self.member, self.behavior
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "generated setter signatures do not explain the domain meaning of optionality, defaults, conversions, or hidden initialization",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "document the member's caller-visible policy with a doc comment or explicit Bon setter documentation",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            BON_UNDOCUMENTED_BUILDER_MEMBERS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this non-obvious builder policy is undocumented");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Performs the `parameter_violation` step of the lint analysis.
fn parameter_violation(cx: &EarlyContext<'_>, parameter: &Param) -> Option<Violation> {
    let member = match cx.sess().source_map().span_to_snippet(parameter.pat.span) {
        Ok(member) => member,
        Err(_error) => return None,
    };
    Violation::member_violation(cx, &parameter.attrs, parameter.ty.span, &member)
}

/// Performs the `field_violation` step of the lint analysis.
fn field_violation(cx: &EarlyContext<'_>, field: &FieldDef) -> Option<Violation> {
    Violation::member_violation(
        cx,
        &field.attrs,
        field.ty.span,
        &field.ident?.name.to_string(),
    )
}

/// Carries the `BonUndocumentedBuilderMembers` state used by this analysis.
struct BonUndocumentedBuilderMembers;

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_UNDOCUMENTED_BUILDER_MEMBERS,
    Warn,
    "requires documentation for non-obvious public Bon member policy",
    BonUndocumentedBuilderMembers
}

impl EarlyLintPass for BonUndocumentedBuilderMembers {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        if !matches!(item.vis.kind, VisibilityKind::Public) {
            return;
        }
        match &item.kind {
            ItemKind::Fn(function) if builder_attribute(&item.attrs).is_some() => {
                for parameter in &function.sig.decl.inputs {
                    let Some(violation) = parameter_violation(cx, parameter) else {
                        continue;
                    };
                    violation.emit(cx);
                }
            }
            ItemKind::Struct(_, _, data)
                if BonAttributeAnalysis::derives_builder(cx, &item.attrs) =>
            {
                for field in data.fields() {
                    let Some(violation) = field_violation(cx, field) else {
                        continue;
                    };
                    violation.emit(cx);
                }
            }
            _ => {}
        }
    }
}
