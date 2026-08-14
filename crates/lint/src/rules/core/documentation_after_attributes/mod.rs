extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{AssocItem, Attribute, FieldDef, Item, ItemKind, Variant};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::{Span, sym};

use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Misordered documentation attribute diagnostic
// -----------------------------------------------------------------------------

/// Documentation preceded by another outer attribute on the same declaration.
struct Violation {
    /// First non-documentation attribute appearing before documentation.
    preceding: Span,
    /// First misplaced documentation attribute.
    documentation: Span,
}

impl Violation {
    /// Finds the first documentation attribute appearing after a non-documentation attribute.
    fn from_attributes(attributes: &[Attribute]) -> Option<Self> {
        let mut preceding = None;
        for attribute in attributes {
            let is_documentation = attribute.is_doc_comment() || attribute.has_name(sym::doc);
            if is_documentation {
                if let Some(preceding) = preceding {
                    return Some(Self {
                        preceding,
                        documentation: attribute.span,
                    });
                }
            } else {
                preceding.get_or_insert(attribute.span);
            }
        }
        None
    }
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("declaration documentation appears after another attribute")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "documentation should introduce the declaration before attributes modify how it is compiled",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("move the complete documentation group above every other outer attribute")
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            DOCUMENTATION_AFTER_ATTRIBUTES,
            self.documentation,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.documentation, "this documentation is misplaced");
                diag.span_label(self.preceding, "this attribute appears first");
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// DocumentationAfterAttributes: Leading declaration contract policy
// -----------------------------------------------------------------------------

/// Pre-expansion pass preserving the authored order around procedural attributes.
struct DocumentationAfterAttributes;

dylint_linting::impl_pre_expansion_lint! {
    #[doc = include_str!("README.md")]
    pub DOCUMENTATION_AFTER_ATTRIBUTES,
    Warn,
    "requires declaration documentation to lead the outer attribute stack",
    DocumentationAfterAttributes
}

impl DocumentationAfterAttributes {
    /// Emits the first ordering failure in one declaration's attribute stack.
    fn check_attributes(cx: &EarlyContext<'_>, attributes: &[Attribute]) {
        let Some(violation) = Violation::from_attributes(attributes) else {
            return;
        };
        if violation.documentation.from_expansion() || violation.preceding.from_expansion() {
            return;
        }
        violation.emit(cx);
    }

    /// Extends declaration ordering to every directly authored aggregate field.
    fn check_fields(cx: &EarlyContext<'_>, fields: &[FieldDef]) {
        for field in fields {
            Self::check_attributes(cx, &field.attrs);
        }
    }
}

impl EarlyLintPass for DocumentationAfterAttributes {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        Self::check_attributes(cx, &item.attrs);
        match &item.kind {
            ItemKind::Struct(_, _, data) | ItemKind::Union(_, _, data) => {
                Self::check_fields(cx, data.fields());
            }
            _ => {}
        }
    }

    fn check_variant(&mut self, cx: &EarlyContext<'_>, variant: &Variant) {
        Self::check_attributes(cx, &variant.attrs);
        Self::check_fields(cx, variant.data.fields());
    }

    fn check_trait_item(&mut self, cx: &EarlyContext<'_>, item: &AssocItem) {
        Self::check_attributes(cx, &item.attrs);
    }

    fn check_impl_item(&mut self, cx: &EarlyContext<'_>, item: &AssocItem) {
        Self::check_attributes(cx, &item.attrs);
    }
}
