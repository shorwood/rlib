extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind, Variant, VariantData};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Positional aggregate diagnostic
// -----------------------------------------------------------------------------

/// Multi-field aggregate whose component roles exist only by position.
struct Violation {
    /// Complete aggregate declaration receiving the diagnostic.
    span: Span,
    /// User-facing aggregate kind.
    kind: &'static str,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!("this {} has multiple positional fields", self.kind))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "constructors and patterns rely on ordering, so component roles remain implicit at every use site",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("use record fields whose names explain each component's semantic role")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            POSITIONAL_AGGREGATE_FIELDS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// PositionalAggregateFields: Lint pass
// -----------------------------------------------------------------------------

/// Late lint pass that rejects aggregates with several unnamed field roles.
struct PositionalAggregateFields;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds tuple structs and tuple-like enum variants with two or more fields. Unit forms and
    /// single-field newtypes remain valid because they do not present multiple unnamed roles.
    ///
    /// ### Why is this bad?
    ///
    /// Naming an aggregate does not make its individual positions self-explanatory. Constructors
    /// and pattern matches still rely on ordering, so readers and agents must repeatedly infer the
    /// same roles. Record fields make the domain vocabulary explicit at every use site.
    ///
    /// For example, these declarations name the aggregate but not its components:
    ///
    /// ```rust
    /// struct Rename(Span, String);
    ///
    /// enum Finding {
    ///     Replacement(Span, String),
    /// }
    /// ```
    ///
    /// Use record fields that state what each value means:
    ///
    /// ```rust
    /// struct Rename {
    ///     span: Span,
    ///     replacement: String,
    /// }
    ///
    /// enum Finding {
    ///     Replacement { span: Span, replacement: String },
    /// }
    /// ```
    pub POSITIONAL_AGGREGATE_FIELDS,
    Warn,
    "rejects multi-field tuple structs and tuple-like enum variants",
    PositionalAggregateFields
}

impl PositionalAggregateFields {
    /// Returns a diagnostic span when variant data has multiple positional fields.
    fn positional_span(data: &VariantData<'_>, fallback: Span) -> Option<Span> {
        let fields = data.fields();
        (fields.len() >= 2 && fields[0].is_positional()).then_some(fallback)
    }
}

impl LateLintPass<'_> for PositionalAggregateFields {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Classify authored tuple structs with more than one positional role.
        if item.span.from_expansion() {
            return;
        }
        let ItemKind::Struct(_, _, data) = item.kind else {
            return;
        };
        let Some(span) = Self::positional_span(&data, item.span) else {
            return;
        };

        // Emit record-oriented guidance for the complete struct declaration.
        Violation {
            span,
            kind: "tuple struct",
        }
        .emit(cx);
    }

    fn check_variant(&mut self, cx: &LateContext<'_>, variant: &Variant<'_>) {
        // Classify authored enum variants with more than one positional role.
        if variant.span.from_expansion() {
            return;
        }
        let Some(span) = Self::positional_span(&variant.data, variant.span) else {
            return;
        };

        // Emit record-oriented guidance for the complete variant declaration.
        Violation {
            span,
            kind: "tuple-like enum variant",
        }
        .emit(cx);
    }
}
