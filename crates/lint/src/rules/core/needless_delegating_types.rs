extern crate rustc_errors;
extern crate rustc_hir;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Expr, ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};

use crate::utils::delegating_type_analysis::{DelegatingTypeAnalyzer, DelegatingTypeFinding};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Needless delegating type diagnostic
// -----------------------------------------------------------------------------

/// One single-field wrapper whose complete API only delegates to its stored value.
struct Violation(
    /// Complete wrapper and forwarding evidence.
    DelegatingTypeFinding,
);

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "type `{}` only delegates to its stored `{}` value",
            self.0.declaration.name, self.0.delegation.inner_type
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "every behavior-bearing method forwards receiver, arguments, return value, and error behavior unchanged; no trait contract, invariant, representation policy, or lifecycle behavior gives `{}` distinct ownership",
            self.0.declaration.name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "use `{}` directly, or move a real invariant or behavioral policy into `{}`",
            self.0.delegation.inner_type, self.0.declaration.name
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render stable diagnostic layers before moving method evidence.
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Keep the field and representative forwarding methods adjacent to removal guidance.
        cx.tcx.emit_node_span_lint(
            NEEDLESS_DELEGATING_TYPES,
            self.0.declaration.hir_id,
            self.0.declaration.name_span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.span_label(
                    self.0.delegation.field_span,
                    "all behavior is delegated to this value",
                );
                for method in self.0.delegation.forwarding_methods {
                    diag.span_label(method, "forwards unchanged");
                }
                diag.note(rationale);
                if self.0.is_closed_package_public {
                    diag.note(
                        "this public type is analyzed because the package declares `publish = false`",
                    );
                }
                diag.help(remediation);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// NeedlessDelegatingTypes: Wrapper ownership policy
// -----------------------------------------------------------------------------

/// Crate-wide collector for wrapper declarations, constructions, and inherent behavior.
#[derive(Default)]
struct NeedlessDelegatingTypes {
    /// Shared semantic analyzer.
    analyzer: DelegatingTypeAnalyzer,
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds authored concrete single-field structs whose complete inherent API contains at least
    /// two behavior-bearing methods and only forwards them to the stored value. Forwarding must
    /// preserve the receiver, arguments and their order, return value, error propagation, and async
    /// behavior. An identity constructor or direct field accessor is neutral, but any substantive
    /// constructor or method preserves the wrapper.
    ///
    /// The lint excludes generic wrappers and wrappers with trait implementations, since those can
    /// own a real type-level or behavioral contract. Semantic attributes such as representation or
    /// serialization policy also preserve the type; documentation and lint-control attributes do
    /// not establish such a contract by themselves. Generated declarations and unrestricted public
    /// APIs in publishable libraries are ignored. Binaries and packages marked `publish = false`
    /// are treated as closed, and behavior compiled for active tests is included in the decision.
    ///
    /// ### Why is this bad?
    ///
    /// A wrapper without an invariant, representation boundary, or behavioral policy adds another
    /// name and navigation layer without owning a distinct contract. Using the stored type directly
    /// keeps ownership visible until the wrapper has real behavior to enforce.
    ///
    /// ```rust
    /// struct Connection;
    ///
    /// impl Connection {
    ///     fn read(&self) -> usize { 0 }
    ///     fn write(&self, bytes: usize) -> usize { bytes }
    /// }
    ///
    /// struct Client(Connection);
    ///
    /// impl Client {
    ///     fn read(&self) -> usize {
    ///         self.0.read()
    ///     }
    ///
    ///     fn write(&self, bytes: usize) -> usize {
    ///         self.0.write(bytes)
    ///     }
    /// }
    /// ```
    ///
    /// If `Client` owns no validation, lifecycle, representation, or domain policy, use the stored
    /// type directly:
    ///
    /// ```rust
    /// struct Connection;
    ///
    /// impl Connection {
    ///     fn read(&self) -> usize { 0 }
    ///     fn write(&self, bytes: usize) -> usize { bytes }
    /// }
    ///
    /// fn transfer(connection: &Connection) {
    ///     let bytes = connection.read();
    ///     connection.write(bytes);
    /// }
    /// ```
    ///
    /// Exact forwarding is intentionally a narrow signal: wrappers that transform values, enforce
    /// invariants, implement traits, or contain any substantive API are not diagnosed. No automatic
    /// rewrite is offered because removing a nominal type changes construction, field access, type
    /// signatures, imports, and potentially external data or ABI contracts.
    pub NEEDLESS_DELEGATING_TYPES,
    Warn,
    "questions single-field types whose complete API only delegates",
    NeedlessDelegatingTypes::default()
}

impl<'tcx> LateLintPass<'tcx> for NeedlessDelegatingTypes {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.analyzer.record_item(cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        self.analyzer.record_impl_item(cx, item);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        self.analyzer.record_expression(cx, expression);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for finding in std::mem::take(&mut self.analyzer).findings(cx) {
            Violation(finding).emit(cx);
        }
    }
}
