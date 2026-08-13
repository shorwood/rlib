extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{FieldDef, Fn, Item, ItemKind, Param};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::{BonAttributeAnalysis, ConfiguredIdentifier, builder_attribute};
use crate::utils::diagnostic::EarlyViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `configured` value used by this analysis.
    configured: String,
    /// Stores the `established` value used by this analysis.
    established: String,
}

impl Violation {
    /// Performs the `member_violation` step of the lint analysis.
    fn member_violation(
        cx: &EarlyContext<'_>,
        attributes: &[rustc_ast::Attribute],
        established: String,
    ) -> Option<Self> {
        // Prepare the values used by this stage.
        let attribute = builder_attribute(attributes)?;
        let source = match BonAttributeAnalysis::source(cx, attribute) {
            Ok(source) => source,
            Err(_error) => return None,
        };
        let configured = ConfiguredIdentifier {
            source: &source,
            key: "name",
        }
        .parse()?;

        // Perform the next step of the analysis.
        (generic_member(&configured) && !generic_member(&established)).then_some(Self {
            span: attribute.span,
            configured,
            established,
        })
    }
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

/// Performs the `generic_operation` step of the lint analysis.
fn generic_operation(name: &str) -> bool {
    matches!(name, "done" | "execute" | "finish" | "process" | "run")
}

/// Performs the `generic_member` step of the lint analysis.
fn generic_member(name: &str) -> bool {
    matches!(name, "arg" | "data" | "item" | "param" | "thing" | "value")
}

/// Performs the `parameter_violation` step of the lint analysis.
fn parameter_violation(cx: &EarlyContext<'_>, parameter: &Param) -> Option<Violation> {
    let established = match cx.sess().source_map().span_to_snippet(parameter.pat.span) {
        Ok(established) => established,
        Err(_error) => return None,
    };
    Violation::member_violation(cx, &parameter.attrs, established.trim().to_owned())
}

/// Performs the `field_violation` step of the lint analysis.
fn field_violation(cx: &EarlyContext<'_>, field: &FieldDef) -> Option<Violation> {
    Violation::member_violation(cx, &field.attrs, field.ident?.name.to_string())
}

/// Carries the `BonIncoherentBuilderVocabulary` state used by this analysis.
struct BonIncoherentBuilderVocabulary;

impl BonIncoherentBuilderVocabulary {
    /// Checks the configured builder vocabulary and every function parameter.
    fn check_function(cx: &EarlyContext<'_>, item: &Item, function: &Fn) {
        let Some(attribute) = builder_attribute(&item.attrs) else {
            return;
        };
        let operation = item
            .kind
            .ident()
            .map(|ident| ident.name.to_string())
            .unwrap_or_default();

        // Diagnose generic entry and finish names that discard the operation vocabulary.
        if let Ok(source) = BonAttributeAnalysis::source(cx, attribute) {
            for key in ["start_fn", "finish_fn"] {
                if let Some(configured) = (ConfiguredIdentifier {
                    source: &source,
                    key,
                })
                .parse()
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

        // Diagnose parameter-level vocabulary independently.
        for parameter in &function.sig.decl.inputs {
            let Some(violation) = parameter_violation(cx, parameter) else {
                continue;
            };
            violation.emit(cx);
        }
    }
}

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
                Self::check_function(cx, item, function);
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
