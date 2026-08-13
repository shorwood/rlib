extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{FieldDef, Item, ItemKind, Param, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::{
    attribute_source, builder_attribute, builder_attribute_contains, derives_bon_builder,
    has_attribute, is_option_type,
};
use crate::utils::diagnostic::EarlyViolation;

struct Violation {
    span: Span,
    member: String,
    behavior: &'static str,
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
                    if let Some(violation) = parameter_violation(cx, parameter) {
                        violation.emit(cx);
                    }
                }
            }
            ItemKind::Struct(_, _, data) if derives_bon_builder(cx, &item.attrs) => {
                for field in data.fields() {
                    if let Some(violation) = field_violation(cx, field) {
                        violation.emit(cx);
                    }
                }
            }
            _ => {}
        }
    }
}

fn parameter_violation(cx: &EarlyContext<'_>, parameter: &Param) -> Option<Violation> {
    let member = cx
        .sess()
        .source_map()
        .span_to_snippet(parameter.pat.span)
        .ok()?;
    member_violation(cx, &parameter.attrs, parameter.ty.span, member)
}

fn field_violation(cx: &EarlyContext<'_>, field: &FieldDef) -> Option<Violation> {
    member_violation(
        cx,
        &field.attrs,
        field.ty.span,
        field.ident?.name.to_string(),
    )
}

fn member_violation(
    cx: &EarlyContext<'_>,
    attributes: &[rustc_ast::Attribute],
    ty_span: Span,
    member: String,
) -> Option<Violation> {
    let builder_source = builder_attribute(attributes)
        .and_then(|attribute| attribute_source(cx, attribute))
        .unwrap_or_default();
    if has_attribute(attributes, "doc") || builder_source.contains("doc") {
        return None;
    }
    let ty = cx.sess().source_map().span_to_snippet(ty_span).ok()?;
    let behavior = if is_option_type(&ty) && !builder_attribute_contains(cx, attributes, "required")
    {
        "optional"
    } else if builder_source.contains("default") {
        "default"
    } else if builder_source.contains("into") || builder_source.contains("with") {
        "conversion"
    } else if builder_source.contains("skip") || builder_source.contains("field") {
        "hidden initialization"
    } else {
        return None;
    };
    Some(Violation {
        span: ty_span,
        member: member.trim().to_owned(),
        behavior,
    })
}
