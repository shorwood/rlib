extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{FieldDef, HirId, def::DefKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Boolean field naming diagnostic
// -----------------------------------------------------------------------------

/// Boolean struct field whose name does not read as a predicate.
struct Violation {
    /// Field node used for field-level lint attributes.
    hir_id: HirId,
    /// Authored identifier highlighted by the diagnostic.
    span: Span,
    /// Field name used in diagnostic guidance.
    name: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "boolean struct field `{}` should start with `is_` or `has_`",
            self.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` otherwise reads like a value or command instead of a yes-or-no property",
            self.name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("choose a predicate-style name that describes what the boolean means")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            BOOL_FIELDS_WITHOUT_PREDICATE_PREFIX,
            self.hir_id,
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
// BoolFieldsWithoutPredicatePrefix: Lint pass
// -----------------------------------------------------------------------------

/// Late lint pass that requires boolean field names to read as predicates.
struct BoolFieldsWithoutPredicatePrefix;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Checks that named boolean fields in structs start with `is_` or `has_`.
    ///
    /// ### Why is this bad?
    ///
    /// A predicate prefix makes the meaning of a boolean field clear at call sites. Without one,
    /// the field can read like a command, an event, or an arbitrary value instead of a yes-or-no
    /// property.
    ///
    /// For example, these field names do not communicate that they are predicates:
    ///
    /// ```rust
    /// struct Window {
    ///     active: bool,
    ///     children: bool,
    /// }
    /// ```
    ///
    /// Prefixing them makes their role explicit wherever the fields are read:
    ///
    /// ```rust
    /// struct Window {
    ///     is_active: bool,
    ///     has_children: bool,
    /// }
    /// ```
    pub BOOL_FIELDS_WITHOUT_PREDICATE_PREFIX,
    Warn,
    "enforces is_ or has_ prefixes for boolean struct fields",
    BoolFieldsWithoutPredicatePrefix
}

impl LateLintPass<'_> for BoolFieldsWithoutPredicatePrefix {
    fn check_field_def(&mut self, cx: &LateContext<'_>, field: &FieldDef<'_>) {
        // Tuple fields cannot carry a meaningful predicate name, while code from external macros
        // is outside the crate author's control. Local macro expansions remain lintable.
        if field.is_positional() || field.span.in_external_macro(cx.sess().source_map()) {
            return;
        }

        // `check_field_def` also visits fields on enum variants, so restrict the lint to structs.
        let parent = cx.tcx.parent(field.def_id.to_def_id());
        if cx.tcx.def_kind(parent) != DefKind::Struct {
            return;
        }

        // Ask the type context for the semantic type so aliases to `bool` are covered as well.
        let ty = cx.tcx.type_of(field.def_id).instantiate_identity();
        if !ty.is_bool() {
            return;
        }

        // Predicate-style names already communicate that callers should expect a boolean value.
        let name = field.ident.name.as_str();
        if name.starts_with("is_") || name.starts_with("has_") {
            return;
        }

        // Preserve the field identity and name for one declaration-focused diagnostic.
        Violation {
            hir_id: field.hir_id,
            span: field.ident.span,
            name: field.ident.name,
        }
        .emit(cx);
    }
}
