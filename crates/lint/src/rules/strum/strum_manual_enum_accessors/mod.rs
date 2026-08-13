extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::ImplItem;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use crate::utils::diagnostic::LateViolation;
use crate::utils::variant_methods::AccessorFamilyAnalyzer;

// -----------------------------------------------------------------------------
// Violation: Complete authored accessor family
// -----------------------------------------------------------------------------

/// Complete tuple-variant accessor family reproducible by `EnumTryAs`.
struct Violation {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Enum name quoted in the diagnostic.
    enum_name: Symbol,
    /// Whether replacement would change a public API.
    is_public_api: bool,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` tuple-variant accessors are maintained manually",
            self.enum_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the complete owned, shared, and mutable accessor family duplicates tuple field extraction and `Option` failure behavior",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "derive `strum::EnumTryAs` and remove the equivalent inherent accessor family",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_ENUM_ACCESSORS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.note(rationale);
                if self.is_public_api {
                    diag.note("these accessors are public; generated names, visibility, and return types require a compatibility review");
                }
                diag.help(remediation);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// StrumManualEnumAccessors: Accessor family analysis
// -----------------------------------------------------------------------------
#[derive(Default)]
/// Finds complete manual tuple-variant accessor families reproducible by `EnumTryAs`.
struct StrumManualEnumAccessors {
    /// Shared collector that proves an accessor exists for every eligible variant.
    analyzer: AccessorFamilyAnalyzer,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_ENUM_ACCESSORS,
    Warn,
    "finds complete manual enum payload accessors reproducible by Strum",
    StrumManualEnumAccessors::default()
}

impl LateLintPass<'_> for StrumManualEnumAccessors {
    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        self.analyzer.check_impl_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for family in self.analyzer.complete_families(cx) {
            Violation {
                span: family.span,
                owner: family.owner,
                enum_name: family.enum_name(cx),
                is_public_api: family.is_public_api(),
            }
            .emit(cx);
        }
    }
}
