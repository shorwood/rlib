extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FnDecl};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::diagnostic::LateViolation;
use crate::utils::policy_literal_analysis::{PolicyLiteralAnalyzer, PolicyLiteralFinding};
use crate::utils::policy_literal_kind::PolicyCategory;

// -----------------------------------------------------------------------------
// Violation: Unnamed operational policy
// -----------------------------------------------------------------------------

/// Literal-derived value whose operational meaning has no constant identity.
struct Violation {
    /// Authored literal or maximal literal-derived expression.
    span: Span,
    /// Source spelling retained for a precise primary message.
    source: String,
    /// Behavioral dimension controlled by the value.
    category: PolicyCategory,
    /// Semantic or structural proof that this value controls behavior.
    context: String,
    /// Confidence-gated constant name inferred from authored vocabulary.
    suggested_name: Option<String>,
}

impl Violation {
    /// Converts analyzer evidence into complete owned diagnostic context.
    fn from_finding(cx: &LateContext<'_>, finding: PolicyLiteralFinding) -> Self {
        // Recover the authored expression without extending compiler-context lifetimes.
        let source_map = cx.sess().source_map();
        let source = source_map
            .span_to_snippet(finding.span)
            .unwrap_or_else(|_| "numeric value".to_owned());

        // Preserve every remediation fact before the analyzer finding is consumed.
        Self {
            span: finding.span,
            source,
            category: finding.category,
            context: finding.context,
            suggested_name: finding.suggested_name,
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "literal `{}` encodes an unnamed {}",
            self.source,
            self.category.description()
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "{}; without a constant, reviewers cannot tell whether the value is incidental or contractual",
            self.context
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        self.suggested_name.as_ref().map_or(
            Cow::Borrowed(
                "introduce a domain-specific constant whose name states the controlled operation, policy role, and unit where applicable",
            ),
            |name| Cow::Owned(format!(
                "introduce a domain-specific constant such as `{name}` and use it here; preserve the controlled operation and unit in the name"
            )),
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            UNNAMED_POLICY_LITERALS,
            self.span,
            DiagDecorator(|diagnostic| {
                diagnostic.primary_message(self.primary_message().into_owned());
                diagnostic.note(self.rationale_message().into_owned());
                diagnostic.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// UnnamedPolicyLiterals: Named behavioral threshold policy
// -----------------------------------------------------------------------------

/// Analyzes each executable body for literals with proven operational meaning.
struct UnnamedPolicyLiterals;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub UNNAMED_POLICY_LITERALS,
    Warn,
    "rejects literals that encode operational policy without a named constant",
    UnnamedPolicyLiterals
}

impl<'tcx> LateLintPass<'tcx> for UnnamedPolicyLiterals {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _: Span,
        _: LocalDefId,
    ) {
        for finding in PolicyLiteralAnalyzer::analyze(cx, body) {
            Violation::from_finding(cx, finding).emit(cx);
        }
    }
}
