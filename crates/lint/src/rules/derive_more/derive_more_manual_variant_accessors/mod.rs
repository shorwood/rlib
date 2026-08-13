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

struct Violation {
    span: Span,
    owner: rustc_hir::HirId,
    enum_name: Symbol,
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
        Cow::Borrowed(
            "the complete one-variant predicate family duplicates enum variant identity",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "derive `derive_more::IsVariant` and remove the equivalent inherent predicate family",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            DERIVE_MORE_MANUAL_VARIANT_ACCESSORS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                if self.is_public_api {
                    diag.note("these predicates are public; generated visibility and method names require a compatibility review");
                }
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

struct DeriveMoreManualVariantAccessors {
    analyzer: PredicateFamilyAnalyzer,
    provider: Option<PredicateProvider>,
}

impl DeriveMoreManualVariantAccessors {
    fn new() -> Self {
        Self {
            analyzer: PredicateFamilyAnalyzer::default(),
            provider: LibraryConfig::load()
                .derive_resolution
                .enum_variant_predicates(),
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_MANUAL_VARIANT_ACCESSORS,
    Warn,
    "finds manual enum predicate families reproducible by derive_more",
    DeriveMoreManualVariantAccessors::new()
}

impl LateLintPass<'_> for DeriveMoreManualVariantAccessors {
    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        self.analyzer.check_impl_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for family in self.analyzer.complete_families(cx) {
            if PredicateFamily::selected(cx, self.provider)
                == Some(PredicateProvider::DeriveMoreIsVariant)
            {
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
}
