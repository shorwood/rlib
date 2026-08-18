extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{AssocItemKind, FieldDef, Fn, Item, ItemKind, Param};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::attributes::{BonAttributeAnalysis, ConfiguredIdentifier};
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Generated name erasing domain vocabulary
// -----------------------------------------------------------------------------

/// Generic Bon name replacing a more specific authored name.
struct Violation {
    /// Bon attribute containing the generic override.
    span: Span,
    /// Generated name selected by the override.
    configured: String,
    /// Domain name already established by the declaration.
    established: String,
}

impl Violation {
    /// Builds a violation when a member override discards specific vocabulary.
    fn member_violation(
        cx: &EarlyContext<'_>,
        attributes: &[rustc_ast::Attribute],
        established: String,
    ) -> Option<Self> {
        let attribute = BonAttributeAnalysis::builder(attributes)?;
        let source = match BonAttributeAnalysis::source(cx, attribute) {
            Ok(source) => source,
            // Missing attribute source prevents comparison with established vocabulary.
            Err(_error) => return None,
        };
        let configured = ConfiguredIdentifier {
            source: &source,
            key: "name",
        }
        .parse()?;

        (BonIncoherentBuilderVocabulary::generic_member_name(&configured)
            && !BonIncoherentBuilderVocabulary::generic_member_name(&established))
        .then_some(Self {
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

// -----------------------------------------------------------------------------
// BonIncoherentBuilderVocabulary: Domain naming policy
// -----------------------------------------------------------------------------

/// Preserves authored domain vocabulary in Bon-generated entry points and setters.
struct BonIncoherentBuilderVocabulary;

crate::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub BON_INCOHERENT_BUILDER_VOCABULARY,
    Warn,
    "rejects generic Bon names that erase established domain vocabulary",
    BonIncoherentBuilderVocabulary
}

impl BonIncoherentBuilderVocabulary {
    /// Returns whether a generated operation name lacks domain meaning.
    fn generic_operation_name(name: &str) -> bool {
        matches!(name, "done" | "execute" | "finish" | "process" | "run")
    }

    /// Returns whether a generated member name lacks domain meaning.
    fn generic_member_name(name: &str) -> bool {
        matches!(name, "arg" | "data" | "item" | "param" | "thing" | "value")
    }

    /// Checks a function parameter's configured setter name.
    fn parameter_violation(cx: &EarlyContext<'_>, parameter: &Param) -> Option<Violation> {
        let established = match cx.sess().source_map().span_to_snippet(parameter.pat.span) {
            Ok(established) => established,
            // Missing parameter source prevents recovery of its established vocabulary.
            Err(_error) => return None,
        };
        Violation::member_violation(cx, &parameter.attrs, established.trim().to_owned())
    }

    /// Checks a struct field's configured setter name.
    fn field_violation(cx: &EarlyContext<'_>, field: &FieldDef) -> Option<Violation> {
        Violation::member_violation(cx, &field.attrs, field.ident?.name.to_string())
    }

    /// Checks configured start and finish vocabulary against an established operation name.
    fn check_operation(
        cx: &EarlyContext<'_>,
        attributes: &[rustc_ast::Attribute],
        operation: &str,
    ) {
        // Functions outside the Bon builder contract expose no configured operation vocabulary.
        let Some(attribute) = BonAttributeAnalysis::builder(attributes) else {
            return;
        };

        // Diagnose generic entry and finish names that discard the operation vocabulary.
        let Ok(source) = BonAttributeAnalysis::source(cx, attribute) else {
            return;
        };
        for key in ["start_fn", "finish_fn"] {
            if let Some(configured) = (ConfiguredIdentifier {
                source: &source,
                key,
            })
            .parse()
                && Self::generic_operation_name(&configured)
                && !operation.split('_').any(|word| word == configured)
            {
                Violation {
                    span: attribute.span,
                    configured,
                    established: operation.to_owned(),
                }
                .emit(cx);
                break;
            }
        }
    }

    /// Checks the configured builder vocabulary and every function parameter.
    fn check_function(
        cx: &EarlyContext<'_>,
        attributes: &[rustc_ast::Attribute],
        operation: &str,
        function: &Fn,
    ) {
        // Functions outside the Bon builder contract expose no member vocabulary to compare.
        if BonAttributeAnalysis::builder(attributes).is_none() {
            return;
        }
        Self::check_operation(cx, attributes, operation);

        // Diagnose parameter-level vocabulary independently.
        for parameter in &function.sig.decl.inputs {
            let Some(violation) = Self::parameter_violation(cx, parameter) else {
                continue;
            };
            violation.emit(cx);
        }
    }
}

impl EarlyLintPass for BonIncoherentBuilderVocabulary {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        match &item.kind {
            ItemKind::Fn(function) => {
                let operation = item
                    .kind
                    .ident()
                    .map(|ident| ident.name.to_string())
                    .unwrap_or_default();
                Self::check_function(cx, &item.attrs, &operation, function);
            }
            ItemKind::Impl(implementation) => {
                for associated in &implementation.items {
                    let AssocItemKind::Fn(function) = &associated.kind else {
                        continue;
                    };
                    let operation = associated
                        .kind
                        .ident()
                        .map(|ident| ident.name.to_string())
                        .unwrap_or_default();
                    Self::check_function(cx, &associated.attrs, &operation, function);
                }
            }
            ItemKind::Struct(_, _, data)
                if BonAttributeAnalysis::derives_builder(cx, &item.attrs) =>
            {
                let operation = item
                    .kind
                    .ident()
                    .map(|ident| ident.name.to_string())
                    .unwrap_or_default();
                Self::check_operation(cx, &item.attrs, &operation);
                for field in data.fields() {
                    let Some(violation) = Self::field_violation(cx, field) else {
                        continue;
                    };
                    violation.emit(cx);
                }
            }
            _ => {}
        }
    }
}
