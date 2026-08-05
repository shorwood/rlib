extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind, Variant, VariantData};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

// -----------------------------------------------------------------------------
// PositionalAggregateFields
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

    /// Emits record-field guidance for one positional aggregate.
    fn emit(cx: &LateContext<'_>, span: Span, kind: &str) {
        cx.emit_span_lint(
            POSITIONAL_AGGREGATE_FIELDS,
            span,
            DiagDecorator(|diag| {
                diag.primary_message(format!("this {kind} has multiple positional fields"));
                diag.help("use record fields whose names explain each component's semantic role");
            }),
        );
    }
}

impl LateLintPass<'_> for PositionalAggregateFields {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if item.span.from_expansion() {
            return;
        }
        let ItemKind::Struct(_, _, data) = item.kind else {
            return;
        };
        let Some(span) = Self::positional_span(&data, item.span) else {
            return;
        };
        Self::emit(cx, span, "tuple struct");
    }

    fn check_variant(&mut self, cx: &LateContext<'_>, variant: &Variant<'_>) {
        if variant.span.from_expansion() {
            return;
        }
        let Some(span) = Self::positional_span(&variant.data, variant.span) else {
            return;
        };
        Self::emit(cx, span, "tuple-like enum variant");
    }
}
