extern crate rustc_errors;
extern crate rustc_hir;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_session::config::CrateType;

use crate::utils::diagnostic::LateViolation;
use crate::utils::foreign_type_analysis::{
    ForeignTypeAnalyzer, ForeignTypeFunctionFinding, ForeignTypeParameterEvidence,
};

// -----------------------------------------------------------------------------
// Violation: Foreign type operation ownership diagnostic
// -----------------------------------------------------------------------------

/// Visible free function whose foreign-type operation lacks an idiomatic owner.
enum Violation {
    /// One foreign type is the clear semantic receiver.
    ClearOwner {
        /// Complete crate-wide ownership evidence.
        finding: ForeignTypeFunctionFinding,
        /// Foreign type that should receive a focused extension trait.
        owner: ForeignTypeParameterEvidence,
    },
    /// Several foreign types remain plausible semantic receivers.
    AmbiguousOwner {
        /// Complete crate-wide ownership evidence.
        finding: ForeignTypeFunctionFinding,
    },
}

impl Violation {
    /// Renders a source-ordered list of nominal type names.
    fn type_names(parameters: &[ForeignTypeParameterEvidence]) -> String {
        let names = parameters
            .iter()
            .map(|parameter| format!("`{}`", parameter.name))
            .collect::<Vec<_>>();
        names.join(", ")
    }

    /// Returns the complete finding shared by both ownership classifications.
    const fn finding(&self) -> &ForeignTypeFunctionFinding {
        match self {
            Self::ClearOwner { finding, .. } | Self::AmbiguousOwner { finding } => finding,
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        match self {
            Self::ClearOwner { finding, owner } => Cow::Owned(format!(
                "visible free function `{}` behaves like an extension method on `{}`",
                finding.name, owner.name
            )),
            Self::AmbiguousOwner { finding } => Cow::Owned(format!(
                "visible free function `{}` exposes foreign behavior without a clear owner",
                finding.name
            )),
        }
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        match self {
            Self::ClearOwner { owner, .. } => Cow::Owned(format!(
                "`{}` is the only non-ambient foreign parameter, so callers must discover its operation in a helper namespace instead of through the type it extends",
                owner.name
            )),
            Self::AmbiguousOwner { finding } => {
                let candidates = Self::type_names(&finding.candidates);
                Cow::Owned(format!(
                    "foreign parameter types {candidates} compete for ownership, so the API does not reveal which concept owns the operation"
                ))
            }
        }
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        match self {
            Self::ClearOwner { owner, .. } => Cow::Owned(format!(
                "define a focused local extension trait for `{}` and colocate this operation with its impl",
                owner.name
            )),
            Self::AmbiguousOwner { .. } => Cow::Borrowed(
                "choose one semantic subject and define a focused extension trait, or introduce a domain object that owns the operation",
            ),
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Resolve the shared diagnostic anchor before rendering variant-specific messages.
        let span = self.finding().span;

        // Emit only after the variant-specific diagnostic anchor is resolved.
        cx.emit_span_lint(
            FOREIGN_TYPE_METHOD_LIKE_FREE_FUNCTIONS,
            span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                match &self {
                    Self::ClearOwner { owner, .. } => {
                        diag.span_label(
                            owner.span,
                            format!("`{}` is the inferred semantic owner", owner.name),
                        );
                    }
                    Self::AmbiguousOwner { finding } => {
                        for candidate in &finding.candidates {
                            diag.span_label(
                                candidate.span,
                                format!("`{}` remains a plausible owner", candidate.name),
                            );
                        }
                    }
                }
                diag.note(self.rationale_message().into_owned());
                let ambient = &self.finding().ambient;
                if !ambient.is_empty() {
                    diag.note(format!(
                        "excluded {} as ambient infrastructure because it recurs beside distinct nominal subjects in this crate",
                        Self::type_names(ambient)
                    ));
                }
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// ForeignTypeMethodLikeFreeFunctions: Extension trait ownership policy
// -----------------------------------------------------------------------------
/// Collects visible free functions whose behavior may belong on a foreign type extension trait.
#[derive(Default)]
struct ForeignTypeMethodLikeFreeFunctions {
    /// Cross-function analysis used to distinguish subjects from ambient infrastructure.
    analyzer: ForeignTypeAnalyzer,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
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
        for mut finding in self.analyzer.findings(cx) {
            let violation = if finding.owners.len() == 1 {
                let owner = finding.owners.remove(0);
                Violation::ClearOwner { finding, owner }
            } else {
                Violation::AmbiguousOwner { finding }
            };
            violation.emit(cx);
        }
    }
}
