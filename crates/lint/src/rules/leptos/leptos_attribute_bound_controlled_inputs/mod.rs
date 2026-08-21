extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rustc_errors::DiagDecorator;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::{BytePos, Span, Symbol};

use super::utils::reactive_capability::ReactiveCapability;
use crate::utils::diagnostic::LateViolation;

/// Stable source coordinates used to deduplicate macro-expanded findings.
#[derive(Eq, Hash, PartialEq)]
struct SourceRange {
    /// First byte in the authored value.
    start: BytePos,
    /// Byte immediately after the authored value.
    end: BytePos,
}

// -----------------------------------------------------------------------------
// Violation: Attribute bound control diagnostic
// -----------------------------------------------------------------------------

/// Writable state supplied through an initial-state HTML attribute.
struct Violation {
    /// Attribute call used to honor local lint attributes.
    owner: rustc_hir::HirId,
    /// Authored attribute expression highlighted by the diagnostic.
    span: Span,
    /// Form state name used in the remediation.
    attribute: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "writable state is passed through the `{}` HTML attribute",
            self.attribute
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the HTML attribute sets initial state, while the DOM property holds the value after user interaction",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "use `bind:{0}` for two-way state, or `prop:{0}` with an explicit event handler",
            self.attribute
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            LEPTOS_ATTRIBUTE_BOUND_CONTROLLED_INPUTS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.span,
                    "this sets an attribute rather than the live property",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// LeptosAttributeBoundControlledInputs: Form state policy
// -----------------------------------------------------------------------------

/// Late lint pass that keeps reactive form state on live DOM properties.
#[derive(Default)]
struct LeptosAttributeBoundControlledInputs {
    /// Authored value ranges already reported through macro-expanded HIR nodes.
    reported: HashSet<SourceRange>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub LEPTOS_ATTRIBUTE_BOUND_CONTROLLED_INPUTS,
    Warn,
    "rejects writable form state supplied through initial-state HTML attributes",
    LeptosAttributeBoundControlledInputs::default()
}

impl LeptosAttributeBoundControlledInputs {
    /// Recovers a raw authored state attribute immediately before a reactive value.
    fn raw_state_attribute(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<Symbol> {
        let callsite = expression.span.source_callsite();
        let source_file = cx.sess().source_map().lookup_source_file(callsite.lo());
        let source = source_file.src.as_deref()?;

        // An invalid source-relative offset prevents reliable inspection of the preceding attribute.
        let Ok(offset) = usize::try_from(callsite.lo().0.checked_sub(source_file.start_pos.0)?)
        else {
            return None;
        };
        let source = source.get(..offset)?.trim_end();
        let tag = source.rsplit_once('<')?.1.split_ascii_whitespace().next()?;

        // Only form controls define the raw state attributes governed by this lint.
        if !matches!(tag, "input" | "select" | "textarea") {
            return None;
        }

        let before_equals = source.strip_suffix('=')?.trim_end();
        for attribute in ["value", "checked"] {
            let Some(before) = before_equals.strip_suffix(attribute) else {
                continue;
            };

            // A token-delimited suffix proves the reactive value belongs to this exact attribute.
            if before
                .chars()
                .next_back()
                .is_none_or(|character| character == '<' || character.is_ascii_whitespace())
            {
                return Some(Symbol::intern(attribute));
            }
        }

        None
    }
}

impl<'tcx> LateLintPass<'tcx> for LeptosAttributeBoundControlledInputs {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        // Expressions without an adjacent raw state attribute are unrelated to controlled inputs.
        let Some(attribute) = Self::raw_state_attribute(cx, expression) else {
            return;
        };
        let owner = cx.tcx.hir_enclosing_body_owner(expression.hir_id);
        let ty = cx.tcx.typeck(owner).expr_ty(expression);

        // Immutable values cannot participate in two-way controlled input state.
        if !ReactiveCapability::has_mutation(cx, owner, ty) {
            return;
        }

        let span = expression.span.source_callsite();

        // A source range already reported through expansion aliases should emit only once.
        if !self.reported.insert(SourceRange {
            start: span.lo(),
            end: span.hi(),
        }) {
            return;
        }

        Violation {
            owner: expression.hir_id,
            span,
            attribute,
        }
        .emit(cx);
    }
}
