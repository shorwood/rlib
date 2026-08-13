extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{AssocItem, FieldDef, Item, ItemKind, VariantData, Visibility};
use rustc_errors::{Applicability, DiagDecorator};
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::{BytePos, Span};

use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// CanonicalRemediation: Restricted syntax classification
// -----------------------------------------------------------------------------
#[derive(Clone, Copy)]
/// Semantics-preserving canonical replacement, or architectural guidance for a path restriction.
enum CanonicalRemediation {
    /// Exact source replacement safe to apply without usage analysis.
    Replacement(
        /// Canonical source text replacing the complete visibility span.
        &'static str,
    ),
    /// Arbitrary ancestor-path restriction requiring an ownership decision.
    Reorganize,
}

impl CanonicalRemediation {
    /// Classifies noncanonical authored syntax while retaining the accepted vocabulary.
    fn from_source(source: &str) -> Option<Self> {
        match source.trim() {
            "pub(self)" | "pub(in self)" => Some(Self::Replacement("")),
            "pub(in super)" => Some(Self::Replacement("pub(super)")),
            "pub(in crate)" => Some(Self::Replacement("pub(crate)")),
            source if source.starts_with("pub(in ") => Some(Self::Reorganize),
            _ => None,
        }
    }
}

// -----------------------------------------------------------------------------
// Violation: Noncanonical restricted visibility diagnostic
// -----------------------------------------------------------------------------

/// Authored restricted visibility with exact syntax-preserving remediation context.
struct Violation {
    /// Visibility token range highlighted and, when safe, replaced.
    span: Span,
    /// Exact range replaced by syntax-only remediation.
    suggestion_span: Span,
    /// Declaration identity shown to the author.
    declaration: String,
    /// Original authored spelling.
    source: String,
    /// Canonical replacement or ownership-level remediation.
    remediation: CanonicalRemediation,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "{} uses noncanonical restricted visibility `{}`",
            self.declaration, self.source
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "a small visibility vocabulary makes module boundaries immediately comparable and prevents arbitrary ancestor paths from encoding fragile file topology",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        // Explain exact aliases before falling back to architectural remediation.
        if let CanonicalRemediation::Replacement(replacement) = self.remediation {
            if replacement.is_empty() {
                return Cow::Borrowed(
                    "remove the explicit self-only visibility and keep the declaration private",
                );
            }
            return Cow::Owned(format!(
                "write the equivalent canonical `{replacement}` visibility"
            ));
        }
        Cow::Borrowed(
            "move the declaration to the module that owns its consumers, or deliberately use `pub(crate)` when the capability belongs to the whole crate",
        )
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        // Render stable diagnostic layers before moving the remediation category.
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Emit a syntax fix only for exact semantic aliases.
        cx.emit_span_lint(
            NONCANONICAL_RESTRICTED_VISIBILITY,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.note(rationale);
                match self.remediation {
                    CanonicalRemediation::Replacement(replacement) => diag.span_suggestion(
                        self.suggestion_span,
                        remediation,
                        replacement,
                        Applicability::MachineApplicable,
                    ),
                    CanonicalRemediation::Reorganize => diag.help(remediation),
                };
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// NoncanonicalRestrictedVisibility: Canonical syntax policy
// -----------------------------------------------------------------------------

/// Early lint pass over authored visibility-bearing declarations.
struct NoncanonicalRestrictedVisibility;

dylint_linting::impl_early_lint! {
    #[doc = include_str!("README.md")]
    pub NONCANONICAL_RESTRICTED_VISIBILITY,
    Warn,
    "rejects restricted visibility outside the canonical four-boundary vocabulary",
    NoncanonicalRestrictedVisibility
}

impl NoncanonicalRestrictedVisibility {
    /// Includes one horizontal separator when removing a self-only visibility token.
    fn removal_span(cx: &EarlyContext<'_>, span: Span) -> Span {
        // Inspect exactly one following byte without consuming vertical layout.
        let extended = span.with_hi(span.hi() + BytePos(1));
        let Ok(source) = cx.sess().source_map().span_to_snippet(extended) else {
            return span;
        };

        // Classify only horizontal whitespace as safe to consume with the token.
        let has_horizontal_separator = source
            .as_bytes()
            .last()
            .is_some_and(|byte| matches!(byte, b' ' | b'\t'));

        // Preserve newlines and non-whitespace bytes owned by following syntax.
        if has_horizontal_separator {
            extended
        } else {
            span
        }
    }

    /// Diagnoses one authored visibility when its source spelling is noncanonical.
    fn check_visibility(cx: &EarlyContext<'_>, visibility: &Visibility, declaration: String) {
        // Ignore generated or unavailable source before classifying syntax.
        if visibility.span.from_expansion() {
            return;
        }
        let Ok(source) = cx.sess().source_map().span_to_snippet(visibility.span) else {
            return;
        };
        let Some(remediation) = CanonicalRemediation::from_source(&source) else {
            return;
        };

        // Remove horizontal separation only when the visibility itself disappears.
        let suggestion_span = match remediation {
            CanonicalRemediation::Replacement("") => Self::removal_span(cx, visibility.span),
            CanonicalRemediation::Replacement(_) | CanonicalRemediation::Reorganize => {
                visibility.span
            }
        };

        // Retain declaration identity beside the exact authored spelling.
        let violation = Violation {
            span: visibility.span,
            suggestion_span,
            declaration,
            source,
            remediation,
        };

        // Emit only after all precise remediation context has been retained.
        violation.emit(cx);
    }

    /// Checks independently visible fields on structs and unions.
    fn check_fields(cx: &EarlyContext<'_>, data: &VariantData, owner: &str) {
        for (index, field) in data.fields().iter().enumerate() {
            Self::check_field(cx, field, owner, index);
        }
    }

    /// Renders and checks one named or positional field.
    fn check_field(cx: &EarlyContext<'_>, field: &FieldDef, owner: &str, index: usize) {
        let name = field
            .ident
            .map_or_else(|| index.to_string(), |identifier| identifier.to_string());
        Self::check_visibility(cx, &field.vis, format!("field `{owner}.{name}`"));
    }
}

impl EarlyLintPass for NoncanonicalRestrictedVisibility {
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        // Classify the named declaration before checking independently visible fields.
        let Some(identifier) = item.kind.ident() else {
            return;
        };
        let kind = item.kind.descr();
        Self::check_visibility(cx, &item.vis, format!("{kind} `{}`", identifier.name));

        // Extend the same vocabulary to struct and union field boundaries.
        match &item.kind {
            ItemKind::Struct(owner, _, data) | ItemKind::Union(owner, _, data) => {
                Self::check_fields(cx, data, owner.as_str());
            }
            _ => {}
        }
    }

    fn check_impl_item(&mut self, cx: &EarlyContext<'_>, item: &AssocItem) {
        let Some(identifier) = item.kind.ident() else {
            return;
        };
        Self::check_visibility(
            cx,
            &item.vis,
            format!("associated item `{}`", identifier.name),
        );
    }
}
