extern crate rustc_errors;
extern crate rustc_hir;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};

use crate::utils::foreign_type_analysis::ForeignTypeAnalyzer;

// -----------------------------------------------------------------------------
// ForeignTypeMethodLikeFreeFunctions: Lint pass
// -----------------------------------------------------------------------------

/// Collects visible free functions whose behavior may belong on a foreign type extension trait.
#[derive(Default)]
struct ForeignTypeMethodLikeFreeFunctions {
    /// Cross-function analysis used to distinguish subjects from ambient infrastructure.
    analyzer: ForeignTypeAnalyzer,
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Checks visible free functions that take foreign nominal types and recommends a focused
    /// extension trait when one foreign parameter remains the clear semantic subject. Repeated
    /// foreign dependencies are treated as ambient infrastructure only when they occur in at
    /// least two functions beside at least two different nominal co-parameters.
    ///
    /// ### Why is this bad?
    ///
    /// A public helper namespace hides which operations belong together and separates behavior
    /// from the type callers already use to discover it. A focused extension trait keeps the
    /// behavior colocated without pretending the foreign type itself can gain inherent methods.
    ///
    /// ```rust
    /// pub fn direct_struct(cx: &LateContext<'_>, item: &Item<'_>) -> Option<LocalDefId> {
    ///     // ...
    /// }
    /// ```
    ///
    /// Put the operation behind the semantic subject instead:
    ///
    /// ```rust
    /// trait ItemExt {
    ///     fn direct_struct(&self, cx: &LateContext<'_>) -> Option<LocalDefId>;
    /// }
    ///
    /// impl ItemExt for Item<'_> {
    ///     fn direct_struct(&self, cx: &LateContext<'_>) -> Option<LocalDefId> {
    ///         // ...
    ///     }
    /// }
    /// ```
    ///
    /// When several foreign parameters remain plausible, the lint still reports the public helper
    /// but asks the author to choose an explicit owner instead of guessing.
    pub FOREIGN_TYPE_METHOD_LIKE_FREE_FUNCTIONS,
    Warn,
    "detects visible free functions whose behavior belongs on a focused foreign-type extension trait",
    ForeignTypeMethodLikeFreeFunctions::default()
}

impl LateLintPass<'_> for ForeignTypeMethodLikeFreeFunctions {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.analyzer.record_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for finding in self.analyzer.findings(cx) {
            cx.emit_span_lint(
                FOREIGN_TYPE_METHOD_LIKE_FREE_FUNCTIONS,
                finding.span,
                DiagDecorator(|diag| {
                    if let [owner] = finding.owners.as_slice() {
                        diag.primary_message(format!(
                            "this visible free function behaves like an extension method on `{owner}`"
                        ));
                        diag.help(format!(
                            "define a focused local extension trait for `{owner}` and colocate this operation with its impl"
                        ));
                        return;
                    }

                    let candidates = finding
                        .candidates
                        .iter()
                        .map(|candidate| format!("`{candidate}`"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    diag.primary_message(
                        "this visible free function exposes behavior through foreign types without a clear owner",
                    );
                    diag.note(format!("foreign parameter candidates: {candidates}"));
                    diag.help(
                        "choose one semantic subject and define a focused extension trait, or introduce a domain object that owns the operation",
                    );
                }),
            );
        }
    }
}
