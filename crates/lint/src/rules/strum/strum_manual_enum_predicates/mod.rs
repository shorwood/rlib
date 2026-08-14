extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::ImplItem;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::LateViolation;
use crate::utils::variant_methods::{PredicateFamily, PredicateFamilyAnalyzer, PredicateProvider};

// -----------------------------------------------------------------------------
// Violation: Complete authored predicate family
// -----------------------------------------------------------------------------

/// Complete enum predicate family reproducible by `EnumIs`.
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
            "`{}` variant predicates are maintained manually",
            self.enum_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("the complete one-variant predicate family duplicates enum variant identity")
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("derive `strum::EnumIs` and remove the equivalent inherent predicate family")
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_ENUM_PREDICATES,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                diag.note(rationale);
                if self.is_public_api {
                    diag.note("these predicates are public; generated visibility and method names require a compatibility review");
                }
                diag.help(remediation);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// StrumManualEnumPredicates: Predicate family analysis
// -----------------------------------------------------------------------------

/// Finds complete manual variant-predicate families reproducible by `EnumIs`.
struct StrumManualEnumPredicates {
    /// Shared collector that proves a predicate exists for every eligible variant.
    analyzer: PredicateFamilyAnalyzer,
    /// Explicitly resolved framework provider, when one is available.
    provider: Option<PredicateProvider>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_ENUM_PREDICATES,
    Warn,
    "finds complete manual enum predicates reproducible by Strum",
    StrumManualEnumPredicates::new()
}

impl StrumManualEnumPredicates {
    /// Loads the explicit provider policy and starts an empty family analysis.
    fn new() -> Self {
        Self {
            analyzer: PredicateFamilyAnalyzer::default(),
            provider: LibraryConfig::load()
                .derive_resolution
                .enum_variant_predicates(),
        }
    }
}
impl LateLintPass<'_> for StrumManualEnumPredicates {
    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        self.analyzer.check_impl_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        let selected = PredicateFamily::selected(cx, self.provider);
        if selected != Some(PredicateProvider::StrumEnumIs) {
            return;
        }
        for family in self.analyzer.complete_families(cx, selected) {
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
