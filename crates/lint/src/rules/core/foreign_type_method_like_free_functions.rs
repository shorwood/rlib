extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_session::config::CrateType;
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;
use crate::utils::foreign_type_analysis::ForeignTypeAnalyzer;

// -----------------------------------------------------------------------------
// Violation: Foreign type operation ownership diagnostic
// -----------------------------------------------------------------------------

/// Visible free function whose foreign-type operation lacks an idiomatic owner.
enum Violation {
    /// One foreign type is the clear semantic receiver.
    ClearOwner {
        /// Function name span used as the primary diagnostic location.
        span: Span,
        /// Foreign type that should receive a focused extension trait.
        owner: Symbol,
    },
    /// Several foreign types remain plausible semantic receivers.
    AmbiguousOwner {
        /// Function name span used as the primary diagnostic location.
        span: Span,
        /// Foreign types whose competing ownership must be resolved explicitly.
        candidates: Vec<Symbol>,
    },
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        match self {
            Self::ClearOwner { owner, .. } => Cow::Owned(format!(
                "this visible free function behaves like an extension method on `{owner}`"
            )),
            Self::AmbiguousOwner { .. } => Cow::Borrowed(
                "this visible free function exposes behavior through foreign types without a clear owner",
            ),
        }
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        match self {
            Self::ClearOwner { owner, .. } => Cow::Owned(format!(
                "callers must discover this `{owner}` operation in a helper namespace instead of through the type it extends"
            )),
            Self::AmbiguousOwner { candidates, .. } => {
                let rendered = candidates.iter().map(|candidate| format!("`{candidate}`"));
                let candidates = rendered.collect::<Vec<_>>().join(", ");
                Cow::Owned(format!(
                    "foreign parameter candidates {candidates} compete for ownership, so the API does not reveal which concept owns the operation"
                ))
            }
        }
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        match self {
            Self::ClearOwner { owner, .. } => Cow::Owned(format!(
                "define a focused local extension trait for `{owner}` and colocate this operation with its impl"
            )),
            Self::AmbiguousOwner { .. } => Cow::Borrowed(
                "choose one semantic subject and define a focused extension trait, or introduce a domain object that owns the operation",
            ),
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Resolve the shared diagnostic anchor before rendering variant-specific messages.
        let span = match &self {
            Self::ClearOwner { span, .. } | Self::AmbiguousOwner { span, .. } => *span,
        };

        // Emit only after the variant-specific diagnostic anchor is resolved.
        cx.emit_span_lint(
            FOREIGN_TYPE_METHOD_LIKE_FREE_FUNCTIONS,
            span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

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
        // Public functions in a proc-macro crate are compiler entry points with fixed signatures.
        if cx.sess().opts.crate_types.contains(&CrateType::ProcMacro) {
            return;
        }
        self.analyzer.record_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for finding in self.analyzer.findings(cx) {
            let violation = if let [owner] = finding.owners.as_slice() {
                Violation::ClearOwner {
                    span: finding.span,
                    owner: *owner,
                }
            } else {
                Violation::AmbiguousOwner {
                    span: finding.span,
                    candidates: finding.candidates,
                }
            };
            violation.emit(cx);
        }
    }
}
