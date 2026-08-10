extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{FieldDef, HirId, ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;
use crate::utils::visibility_boundary::VisibilityBoundary;
use crate::utils::visibility_usage_analysis::{VisibilityFinding, VisibilityUsageAnalyzer};

// -----------------------------------------------------------------------------
// Violation: Unnecessarily broad visibility diagnostic
// -----------------------------------------------------------------------------

/// One source reference proving the selected canonical boundary.
struct ViolationBoundaryUse {
    /// Authored reference range.
    span: Span,
    /// Module containing the reference.
    module: String,
}

/// Declaration identity carried into one visibility diagnostic.
struct ViolationDeclaration {
    /// Declaration node used to respect its local lint level.
    hir_id: HirId,
    /// Authored visibility token span.
    span: Span,
    /// Declaration identity rendered in guidance.
    name: String,
    /// Human-readable declaration category.
    kind: &'static str,
}

/// Authored and required boundaries with their defining ownership context.
struct ViolationBoundary {
    /// Authored canonical visibility.
    current: VisibilityBoundary,
    /// Narrowest canonical visibility supported by observed uses.
    required: VisibilityBoundary,
    /// Defining module that owns private access.
    defining_module: String,
}

/// Maximum number of boundary-setting references labeled by one diagnostic.
const VIOLATION_MAX_LABELED_USES: usize = 4;

/// One declaration whose authored reach exceeds every observed local requirement.
struct Violation {
    /// Declaration identity and diagnostic anchor.
    declaration: ViolationDeclaration,
    /// Complete visibility decision.
    boundary: ViolationBoundary,
    /// Source labels for uses that establish the required boundary.
    boundary_uses: Vec<ViolationBoundaryUse>,
    /// Whether unrestricted public reach was interpreted inside a closed package.
    is_closed_package_public: bool,
}

impl Violation {
    /// Captures complete diagnostic context before the crate analysis is discarded.
    fn from_finding(cx: &LateContext<'_>, finding: VisibilityFinding) -> Self {
        // Separate shared finding context into diagnostic-specific identity and reach.
        let finding_declaration = finding.declaration;
        let finding_boundary = finding.boundary;

        // Bound source labels so one declaration cannot drown the remediation context.
        let selected_uses = finding.uses.into_iter().take(VIOLATION_MAX_LABELED_USES);
        let boundary_uses = selected_uses
            .map(|usage| ViolationBoundaryUse {
                span: usage.span,
                module: VisibilityUsageAnalyzer::module_name(cx.tcx, usage.module),
            })
            .collect();

        // Render declaration identity and boundary ownership into stable diagnostic records.
        let declaration = ViolationDeclaration {
            hir_id: finding_declaration.hir_id,
            span: finding_declaration.span,
            name: finding_declaration.name.to_string(),
            kind: finding_declaration.kind,
        };

        // Preserve the visibility decision and its ownership context as one value.
        let boundary = ViolationBoundary {
            current: finding_boundary.current,
            required: finding_boundary.required,
            defining_module: finding_declaration.defining_module,
        };

        // Keep declaration, reach, evidence, and package policy in one immutable context.
        Self {
            declaration,
            boundary,
            boundary_uses,
            is_closed_package_public: finding.is_closed_package_public,
        }
    }

    /// Returns source text for the narrower canonical visibility.
    const fn replacement(&self) -> &'static str {
        match self.boundary.required {
            VisibilityBoundary::Private => "",
            required => required.source(),
        }
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "{} `{}` has unnecessarily broad `{}` visibility",
            self.declaration.kind, self.declaration.name, self.boundary.current
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "all observed uses are reachable with `{}` from defining module `{}`; broader visibility enlarges the dependency surface and permits ownership that the current module structure does not require",
            self.boundary.required, self.boundary.defining_module
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "narrow `{}` to `{}` after confirming cfg-disabled and external consumers do not require the broader boundary",
            self.declaration.name, self.boundary.required
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render stable diagnostic layers before moving labels and replacement data.
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();
        let replacement = self.replacement();

        // Keep usage evidence adjacent to the visibility suggestion.
        cx.tcx.emit_node_span_lint(
            UNNECESSARILY_BROAD_VISIBILITY,
            self.declaration.hir_id,
            self.declaration.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                for boundary_use in self.boundary_uses {
                    diag.span_label(
                        boundary_use.span,
                        format!("boundary-setting use from `{}`", boundary_use.module),
                    );
                }
                diag.note(rationale);
                if self.is_closed_package_public {
                    diag.note(
                        "unrestricted `pub` is analyzed because this package declares `publish = false`",
                    );
                }
                diag.span_suggestion(
                    self.declaration.span,
                    remediation,
                    replacement,
                    Applicability::MaybeIncorrect,
                );
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// UnnecessarilyBroadVisibility: Observed reach policy
// -----------------------------------------------------------------------------

/// Crate-wide visibility and use collector.
#[derive(Default)]
struct UnnecessarilyBroadVisibility {
    /// Shared analysis used to derive canonical declaration boundaries.
    analyzer: VisibilityUsageAnalyzer,
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds authored module items, types, functions, constants, statics, struct and union fields,
    /// and inherent associated items whose canonical visibility is broader than every resolved use
    /// in the current crate. It recommends private visibility for uses confined to the defining
    /// module, `pub(super)` for the immediate parent subtree, and `pub(crate)` for wider crate use.
    ///
    /// Unrestricted `pub` APIs in publishable library packages are preserved because downstream
    /// users cannot be observed. Binaries and packages explicitly marked `publish = false` are
    /// treated as closed. Missing or malformed manifest metadata is interpreted conservatively.
    /// Authored reach is capped by restricted ancestor modules, so `pub` inside a private namespace
    /// is accepted when that namespace already supplies the narrowest effective boundary. Generated
    /// declarations, foreign entry points, proc macros, and language items are ignored.
    ///
    /// ### Why is this bad?
    ///
    /// Broad visibility is a dependency permission, not decoration. An unnecessarily visible item
    /// allows code to bypass its owning module, grows the compatibility surface, and makes later
    /// relocation or invariant enforcement harder. Generated code often writes `pub` before it has
    /// decided where an operation belongs; the resulting API then outlives that accident.
    ///
    /// ```rust
    /// mod normalization {
    ///     pub fn normalize_internal_key(key: &str) -> String {
    ///         key.trim().to_owned()
    ///     }
    /// }
    /// ```
    ///
    /// If every caller is inside `normalization`, keep ownership local:
    ///
    /// ```rust
    /// mod normalization {
    ///     fn normalize_internal_key(key: &str) -> String {
    ///         key.trim().to_owned()
    ///     }
    /// }
    /// ```
    ///
    /// Suggestions are deliberately `MaybeIncorrect`: inactive `cfg` branches and Rust consumers
    /// outside a closed workspace may be invisible to the current compiler invocation.
    pub UNNECESSARILY_BROAD_VISIBILITY,
    Warn,
    "rejects canonical visibility broader than observed use requires",
    UnnecessarilyBroadVisibility::default()
}

impl<'tcx> LateLintPass<'tcx> for UnnecessarilyBroadVisibility {
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
        for finding in std::mem::take(&mut self.analyzer).findings(cx) {
            if finding.boundary.effective_current <= finding.boundary.required
                || finding.is_test_constrained()
            {
                continue;
            }
            Violation::from_finding(cx, finding).emit(cx);
        }
    }
}
