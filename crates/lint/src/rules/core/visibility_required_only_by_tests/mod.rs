extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::mem::take;

use rustc_errors::DiagDecorator;
use rustc_hir::{FieldDef, HirId, ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::visibility_boundary::VisibilityBoundary;
use crate::utils::visibility_usage_analysis::{VisibilityFinding, VisibilityUsageAnalyzer};

/// Maximum number of test-only references labeled by one diagnostic.
const VIOLATION_MAX_LABELED_USES: usize = 4;

// -----------------------------------------------------------------------------
// Violation: Test constrained visibility diagnostic
// -----------------------------------------------------------------------------

/// One in-source test reference responsible for widening production reach.
struct ViolationTestUse {
    /// Authored test reference range.
    span: Span,
    /// Test module containing the reference.
    module: String,
}

/// Declaration identity carried into one test-topology diagnostic.
struct ViolationDeclaration {
    /// Declaration node used to respect its local lint level.
    hir_id: HirId,
    /// Authored visibility highlighted by the diagnostic.
    span: Span,
    /// Declaration name used in concrete remediation.
    name: String,
    /// Human-readable declaration category.
    kind: &'static str,
}

/// Production and test-inclusive reach with its defining ownership context.
struct ViolationBoundary {
    /// Boundary currently required only when tests are included.
    test: VisibilityBoundary,
    /// Narrower boundary supported by production uses.
    production: VisibilityBoundary,
    /// Defining module that should own colocated tests.
    defining_module: String,
}

/// One declaration whose production boundary is widened solely by in-source tests.
struct Violation {
    /// Declaration identity and diagnostic anchor.
    declaration: ViolationDeclaration,
    /// Production and test-inclusive visibility decision.
    boundary: ViolationBoundary,
    /// Test references that force the wider boundary.
    test_uses: Vec<ViolationTestUse>,
}

impl Violation {
    /// Captures all test topology before the shared analyzer is discarded.
    fn from_finding(cx: &LateContext<'_>, finding: VisibilityFinding) -> Self {
        // Separate shared finding context into diagnostic-specific identity and reach.
        let finding_declaration = finding.declaration;
        let finding_boundary = finding.boundary;

        // Select only test-owned references and cap the displayed evidence.
        let test_uses = finding
            .uses
            .into_iter()
            .filter(|&usage| usage.is_test_only())
            .map(|usage| ViolationTestUse {
                span: usage.span,
                module: VisibilityUsageAnalyzer::module_name(cx.tcx, usage.module),
            });
        let test_uses = test_uses.take(VIOLATION_MAX_LABELED_USES).collect();

        // Render declaration identity and topology into stable diagnostic records.
        let declaration = ViolationDeclaration {
            hir_id: finding_declaration.hir_id,
            span: finding_declaration.span,
            name: finding_declaration.name.to_string(),
            kind: finding_declaration.kind,
        };

        // Preserve the topology delta and its ownership context as one value.
        let boundary = ViolationBoundary {
            test: finding_boundary.required,
            production: finding_boundary.production_required,
            defining_module: finding_declaration.defining_module,
        };

        // Keep declaration, topology delta, and source evidence in one immutable context.
        Self {
            declaration,
            boundary,
            test_uses,
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "{} `{}` has visibility required only by tests",
            self.declaration.kind, self.declaration.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "production uses require only `{}`, but in-source tests force `{}` beyond defining module `{}`; test layout is therefore expanding the production dependency boundary",
            self.boundary.production, self.boundary.test, self.boundary.defining_module
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "move the tests under `{}` or exercise `{}` through its owning public behavior, then narrow the declaration to `{}`",
            self.boundary.defining_module, self.declaration.name, self.boundary.production
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Explain the production boundary beside the tests that widen it.
        cx.tcx.emit_node_span_lint(
            VISIBILITY_REQUIRED_ONLY_BY_TESTS,
            self.declaration.hir_id,
            self.declaration.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                for test_use in self.test_uses {
                    diag.span_label(
                        test_use.span,
                        format!("test-only boundary use from `{}`", test_use.module),
                    );
                }
                diag.note(rationale);
                diag.help(remediation);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// VisibilityRequiredOnlyByTests: Test topology policy
// -----------------------------------------------------------------------------
/// Crate-wide collector comparing production and test-inclusive reach.
#[derive(Default)]
struct VisibilityRequiredOnlyByTests {
    /// Shared analysis used to derive both canonical boundaries.
    analyzer: VisibilityUsageAnalyzer,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub VISIBILITY_REQUIRED_ONLY_BY_TESTS,
    Warn,
    "rejects production visibility widened solely for in-source tests",
    VisibilityRequiredOnlyByTests::default()
}

impl<'tcx> LateLintPass<'tcx> for VisibilityRequiredOnlyByTests {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.analyzer.record_item(cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        self.analyzer.record_impl_item(cx, item);
    }

    fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &'tcx FieldDef<'tcx>) {
        self.analyzer.record_field(cx, field);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for finding in take(&mut self.analyzer).findings(cx) {
            if !finding.is_test_constrained() {
                continue;
            }
            Violation::from_finding(cx, finding).emit(cx);
        }
    }
}
