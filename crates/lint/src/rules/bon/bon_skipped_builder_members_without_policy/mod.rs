extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{FieldDef, Item, ItemKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;

use super::utils::{BonAttributeAnalysis, builder_attribute, has_attribute};
use crate::utils::diagnostic::EarlyViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `member` value used by this analysis.
    member: String,
}

impl Violation {
    /// Builds a violation for a skipped field without an explicit policy.
    fn from_field(cx: &EarlyContext<'_>, field: &FieldDef) -> Option<Self> {
        // Prepare the values used by this stage.
        let attribute = builder_attribute(&field.attrs)?;
        let source = match BonAttributeAnalysis::source(cx, attribute) {
            Ok(source) => source,
            Err(_error) => return None,
        };

        // Reject inputs that do not satisfy this stage.
        if source.split_whitespace().collect::<String>() != "#[builder(skip)]"
            || has_attribute(&field.attrs, "doc")
            || cx
                .sess()
                .source_map()
                .span_to_snippet(field.ty.span)
                .is_ok_and(|ty| ty.contains("PhantomData"))
        // Perform the next step of the analysis.
        {
            return None;
        }
        Some(Self {
            span: attribute.span,
            member: field.ident?.name.to_string(),
        })
    }
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

/// Carries the `BonSkippedBuilderMembersWithoutPolicy` state used by this analysis.
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
        if !BonAttributeAnalysis::derives_builder(cx, &item.attrs) {
            return;
        }
        for field in data.fields() {
            let Some(violation) = Violation::from_field(cx, field) else {
                continue;
            };
            violation.emit(cx);
        }
    }
}
