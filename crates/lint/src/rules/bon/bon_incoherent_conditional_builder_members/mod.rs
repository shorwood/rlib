extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{AssocItemKind, FieldDef, Fn, Item, ItemKind, Param};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::attributes::BonAttributeAnalysis;
use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Configuration-dependent member contract
// -----------------------------------------------------------------------------

/// Builder member whose requiredness or placement changes with configuration.
struct Violation {
    /// Conditional Bon attribute changing the contract.
    span: Span,
    /// Affected member named in the diagnostic.
    member: String,
}

impl Violation {
    /// Recognizes a struct field with conditional builder policy.
    fn field_violation(cx: &EarlyContext<'_>, field: &FieldDef) -> Option<Self> {
        Self::conditional_policy(cx, &field.attrs).map(|span| Self {
            span,
            member: field
                .ident
                .map_or_else(|| "field".to_owned(), |ident| ident.to_string()),
        })
    }

    /// Recognizes a function parameter with conditional builder policy.
    fn parameter_violation(cx: &EarlyContext<'_>, parameter: &Param) -> Option<Self> {
        let member = match cx.sess().source_map().span_to_snippet(parameter.pat.span) {
            Ok(member) => member,
            // Missing parameter source prevents a meaningful member name in the diagnostic.
            Err(_error) => return None,
        };
        Self::conditional_policy(cx, &parameter.attrs).map(|span| Self {
            span,
            member: member.trim().to_owned(),
        })
    }

    /// Finds a conditional attribute that changes a member's builder policy.
    fn conditional_policy(
        cx: &EarlyContext<'_>,
        attributes: &[rustc_ast::Attribute],
    ) -> Option<Span> {
        attributes.iter().find_map(|attribute| {
            // Attributes outside `cfg_attr` cannot vary builder policy by configuration.
            if BonAttributeAnalysis::name(attribute).is_none_or(|name| name.as_str() != "cfg_attr")
            {
                return None;
            }
            let source = match BonAttributeAnalysis::source(cx, attribute) {
                Ok(source) => source,
                // Missing attribute source prevents inspection of its conditional payload.
                Err(_error) => return None,
            };
            Self::builder_payload_changes_contract(&source).then_some(attribute.span)
        })
    }

    /// Returns whether a `builder(...)` payload contains a contract-changing policy token.
    fn builder_payload_changes_contract(source: &str) -> bool {
        source.match_indices("builder(").any(|(start, marker)| {
            let payload = &source[start + marker.len()..];
            let payload = payload.split(')').next().unwrap_or(payload);
            payload
                .split(|character: char| !character.is_alphanumeric() && character != '_')
                .any(|token| {
                    matches!(
                        token,
                        "default" | "required" | "skip" | "start_fn" | "finish_fn"
                    )
                })
        })
    }
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

// -----------------------------------------------------------------------------
// BonIncoherentConditionalBuilderMembers: Stable member contract policy
// -----------------------------------------------------------------------------

/// Rejects Bon member contracts that vary between build configurations.
struct BonIncoherentConditionalBuilderMembers;

impl BonIncoherentConditionalBuilderMembers {
    /// Checks every parameter on one free or associated builder function.
    fn check_function(cx: &EarlyContext<'_>, attributes: &[rustc_ast::Attribute], function: &Fn) {
        // Functions outside the Bon builder contract have no conditional builder members.
        if BonAttributeAnalysis::builder(attributes).is_none() {
            return;
        }
        for parameter in &function.sig.decl.inputs {
            let Some(violation) = Violation::parameter_violation(cx, parameter) else {
                continue;
            };
            violation.emit(cx);
        }
    }
}

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
            ItemKind::Fn(function) => Self::check_function(cx, &item.attrs, function),
            ItemKind::Impl(implementation) => {
                for associated in &implementation.items {
                    let AssocItemKind::Fn(function) = &associated.kind else {
                        continue;
                    };
                    Self::check_function(cx, &associated.attrs, function);
                }
            }
            ItemKind::Struct(_, _, data)
                if BonAttributeAnalysis::derives_builder(cx, &item.attrs) =>
            {
                for field in data.fields() {
                    let Some(violation) = Violation::field_violation(cx, field) else {
                        continue;
                    };
                    violation.emit(cx);
                }
            }
            _ => {}
        }
    }
}
