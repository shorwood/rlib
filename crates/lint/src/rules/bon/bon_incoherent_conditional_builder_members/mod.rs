extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{FieldDef, Item, ItemKind, Param};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::{attribute_name, attribute_source, builder_attribute, derives_bon_builder};
use crate::utils::diagnostic::EarlyViolation;

struct Violation {
    span: Span,
    member: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "Bon member `{}` changes construction policy conditionally",
            self.member
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "changing requiredness, defaults, or positional placement across configurations gives one domain member incompatible call contracts",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "keep the member's Bon policy stable, gate the whole capability explicitly, or expose distinct configuration-specific APIs",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            BON_INCOHERENT_CONDITIONAL_BUILDER_MEMBERS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this `cfg_attr` changes the builder contract");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct BonIncoherentConditionalBuilderMembers;

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_INCOHERENT_CONDITIONAL_BUILDER_MEMBERS,
    Warn,
    "rejects configuration-dependent Bon member policy",
    BonIncoherentConditionalBuilderMembers
}

impl EarlyLintPass for BonIncoherentConditionalBuilderMembers {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
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
    conditional_policy(cx, &parameter.attrs).map(|span| Violation {
        span,
        member: member.trim().to_owned(),
    })
}

fn field_violation(cx: &EarlyContext<'_>, field: &FieldDef) -> Option<Violation> {
    conditional_policy(cx, &field.attrs).map(|span| Violation {
        span,
        member: field
            .ident
            .map_or_else(|| "field".to_owned(), |ident| ident.to_string()),
    })
}

fn conditional_policy(cx: &EarlyContext<'_>, attributes: &[rustc_ast::Attribute]) -> Option<Span> {
    attributes.iter().find_map(|attribute| {
        if attribute_name(attribute).is_none_or(|name| name.as_str() != "cfg_attr") {
            return None;
        }
        let source = attribute_source(cx, attribute)?;
        (source.contains("builder(")
            && ["default", "required", "skip", "start_fn", "finish_fn"]
                .iter()
                .any(|policy| source.contains(policy)))
        .then_some(attribute.span)
    })
}
