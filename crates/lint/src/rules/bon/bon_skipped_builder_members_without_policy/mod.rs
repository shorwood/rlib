extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{FieldDef, Item, ItemKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::{attribute_source, builder_attribute, derives_bon_builder, has_attribute};
use crate::utils::diagnostic::EarlyViolation;

struct Violation {
    span: Span,
    member: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "skipped Bon member `{}` has only an implicit default policy",
            self.member
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "bare `#[builder(skip)]` initializes the field with `Default::default()`, hiding construction policy from readers and callers",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use `#[builder(skip = expression)]` to state the initialization policy or document why the type's default is the intended invariant",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            BON_SKIPPED_BUILDER_MEMBERS_WITHOUT_POLICY,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this skip silently uses `Default::default()`");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct BonSkippedBuilderMembersWithoutPolicy;

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_SKIPPED_BUILDER_MEMBERS_WITHOUT_POLICY,
    Warn,
    "requires visible initialization policy for skipped Bon fields",
    BonSkippedBuilderMembersWithoutPolicy
}

impl EarlyLintPass for BonSkippedBuilderMembersWithoutPolicy {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        let ItemKind::Struct(_, _, data) = &item.kind else {
            return;
        };
        if !derives_bon_builder(cx, &item.attrs) {
            return;
        }
        for field in data.fields() {
            if let Some(violation) = violation(cx, field) {
                violation.emit(cx);
            }
        }
    }
}

fn violation(cx: &EarlyContext<'_>, field: &FieldDef) -> Option<Violation> {
    let attribute = builder_attribute(&field.attrs)?;
    let source = attribute_source(cx, attribute)?;
    if source.split_whitespace().collect::<String>() != "#[builder(skip)]"
        || has_attribute(&field.attrs, "doc")
        || cx
            .sess()
            .source_map()
            .span_to_snippet(field.ty.span)
            .is_ok_and(|ty| ty.contains("PhantomData"))
    {
        return None;
    }
    Some(Violation {
        span: attribute.span,
        member: field.ident?.name.to_string(),
    })
}
