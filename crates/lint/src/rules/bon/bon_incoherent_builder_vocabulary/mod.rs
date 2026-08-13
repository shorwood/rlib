extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{FieldDef, Item, ItemKind, Param};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::{
    attribute_source, builder_attribute, configured_identifier, derives_bon_builder,
};
use crate::utils::diagnostic::EarlyViolation;

struct Violation {
    span: Span,
    configured: String,
    established: String,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "generic Bon name `{}` replaces established `{}` vocabulary",
            self.configured, self.established
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "generated builder names should preserve the domain language already established by the function or member",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "remove the override to use Bon's conventional default or choose a name derived from the domain operation or member",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            BON_INCOHERENT_BUILDER_VOCABULARY,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.span, "this override weakens the API vocabulary");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct BonIncoherentBuilderVocabulary;

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_INCOHERENT_BUILDER_VOCABULARY,
    Warn,
    "rejects generic Bon names that erase established domain vocabulary",
    BonIncoherentBuilderVocabulary
}

impl EarlyLintPass for BonIncoherentBuilderVocabulary {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        match &item.kind {
            ItemKind::Fn(function) => {
                let Some(attribute) = builder_attribute(&item.attrs) else {
                    return;
                };
                let operation = item
                    .kind
                    .ident()
                    .map(|ident| ident.name.to_string())
                    .unwrap_or_default();
                if let Some(source) = attribute_source(cx, attribute) {
                    for key in ["start_fn", "finish_fn"] {
                        if let Some(configured) = configured_identifier(&source, key)
                            && generic_operation(&configured)
                            && !operation.split('_').any(|word| word == configured)
                        {
                            Violation {
                                span: attribute.span,
                                configured,
                                established: operation,
                            }
                            .emit(cx);
                            break;
                        }
                    }
                }
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
    let established = cx
        .sess()
        .source_map()
        .span_to_snippet(parameter.pat.span)
        .ok()?;
    member_violation(cx, &parameter.attrs, established.trim().to_owned())
}

fn field_violation(cx: &EarlyContext<'_>, field: &FieldDef) -> Option<Violation> {
    member_violation(cx, &field.attrs, field.ident?.name.to_string())
}

fn member_violation(
    cx: &EarlyContext<'_>,
    attributes: &[rustc_ast::Attribute],
    established: String,
) -> Option<Violation> {
    let attribute = builder_attribute(attributes)?;
    let source = attribute_source(cx, attribute)?;
    let configured = configured_identifier(&source, "name")?;
    (generic_member(&configured) && !generic_member(&established)).then_some(Violation {
        span: attribute.span,
        configured,
        established,
    })
}

fn generic_operation(name: &str) -> bool {
    matches!(name, "done" | "execute" | "finish" | "process" | "run")
}

fn generic_member(name: &str) -> bool {
    matches!(name, "arg" | "data" | "item" | "param" | "thing" | "value")
}
