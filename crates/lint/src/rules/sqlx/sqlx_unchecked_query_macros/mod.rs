extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::BTreeSet;

use rustc_errors::DiagDecorator;
use rustc_hir::Expr;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Pos, Span};

use super::utils::SqlxMacroSpanExt as _;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Unchecked query macro
// -----------------------------------------------------------------------------

/// One authored `SQLx` macro invocation that skips Rust type checking.
struct Violation {
    /// HIR owner receiving the lint.
    owner: rustc_hir::HirId,
    /// Authored macro call span.
    span: Span,
    /// Public unchecked macro name.
    macro_name: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}!` skips SQLx's Rust type checks",
            self.macro_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "unchecked query macros weaken the compile-time contract between database columns, bind parameters, and Rust types",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use the corresponding checked SQLx query macro and express exceptional mappings with supported type overrides",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            SQLX_UNCHECKED_QUERY_MACROS,
            self.owner,
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
// MacroRange: Deduplication identity
// -----------------------------------------------------------------------------

/// Byte range identifying one authored macro call site.
#[derive(Eq, Ord, PartialEq, PartialOrd)]
struct MacroRange {
    /// Inclusive start byte position.
    start: u32,
    /// Exclusive end byte position.
    end: u32,
}

// -----------------------------------------------------------------------------
// SqlxUncheckedQueryMacros: Lint pass
// -----------------------------------------------------------------------------

/// Detects each authored unchecked macro once across its expanded expressions.
#[derive(Default)]
struct SqlxUncheckedQueryMacros {
    /// Byte ranges of macro call sites already diagnosed.
    seen: BTreeSet<MacroRange>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub SQLX_UNCHECKED_QUERY_MACROS,
    Warn,
    "rejects SQLx query macros that skip Rust type checking",
    SqlxUncheckedQueryMacros::default()
}

impl LateLintPass<'_> for SqlxUncheckedQueryMacros {
    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Expressions outside SQLx macro expansions have no macro policy to inspect.
        let Some(macro_call) = expression.span.sqlx_macro(cx) else {
            return;
        };
        let name = macro_call.name;
        let call_site = macro_call.call_site;

        // Checked macros and previously visited expansion nodes need no diagnostic.
        if !name.contains("unchecked")
            || !self.seen.insert(MacroRange {
                start: call_site.lo().to_u32(),
                end: call_site.hi().to_u32(),
            })
        {
            return;
        }
        Violation {
            owner: expression.hir_id,
            span: call_site,
            macro_name: name,
        }
        .emit(cx);
    }
}
