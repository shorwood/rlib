extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::authored_contracts::{
    DisplayCandidate, DisplayProvider, StaticValue, VariantValueFamily,
};
use super::utils::contracts::ContractCatalog;
use crate::utils::config::LibraryConfig;
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    enum_name: Symbol,
    is_public: bool,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` string conversion is maintained manually",
            self.enum_name
        ))
    }
    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("the exhaustive match duplicates the enum's canonical static string mapping")
    }
    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("derive `strum::AsRefStr` and migrate callers to `AsRef<str>`")
    }
    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_MANUAL_ENUM_STRING_CONVERSIONS,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                if self.is_public {
                    diag.note("this method is public; retain a forwarding shim if its symbol is part of the API");
                }
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct StrumManualEnumStringConversions {
    catalog: ContractCatalog,
    families: Vec<VariantValueFamily>,
    displays: Vec<DisplayCandidate>,
    display_provider: Option<DisplayProvider>,
}

impl StrumManualEnumStringConversions {
    fn new() -> Self {
        Self {
            catalog: ContractCatalog::default(),
            families: Vec::new(),
            displays: Vec::new(),
            display_provider: LibraryConfig::load().derive_resolution.enum_display(),
        }
    }
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_ENUM_STRING_CONVERSIONS,
    Warn,
    "finds manual enum-to-static-string conversions reproducible by Strum",
    StrumManualEnumStringConversions::new()
}

impl LateLintPass<'_> for StrumManualEnumStringConversions {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        if let Some(display) = DisplayCandidate::from_impl_item(cx, item) {
            self.displays.push(display);
            return;
        }
        let Some(family) = VariantValueFamily::from_impl_item(cx, item) else {
            return;
        };
        if !family.returns_static_str(cx)
            || !matches!(
                family.method_name.as_str(),
                "as_str" | "as_static_str" | "name"
            )
            || family
                .values
                .values()
                .any(|value| !matches!(value, StaticValue::String(_)))
        {
            return;
        }
        self.families.push(family);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        let contracts = self.catalog.contracts();
        for family in self.families.drain(..) {
            let Some(contract) = contracts.iter().find(|contract| {
                contract.def_id == family.enum_def
                    && contract.variants.iter().all(|variant| {
                        family.values.get(&variant.def_id)
                            == Some(&StaticValue::String(variant.preferred_name.clone()))
                    })
            }) else {
                continue;
            };
            Violation {
                owner: family.owner,
                span: family.span,
                enum_name: contract.name,
                is_public: family.is_public,
            }
            .emit(cx);
        }
        if DisplayProvider::selected(cx, self.display_provider)
            != Some(DisplayProvider::StrumDisplay)
        {
            return;
        }
        for display in self.displays.drain(..) {
            let Some(contract) = contracts.iter().find(|contract| {
                contract.def_id == display.enum_def
                    && contract.variants.iter().all(|variant| {
                        display.values.get(&variant.def_id) == Some(&variant.preferred_name)
                    })
            }) else {
                continue;
            };
            Violation {
                owner: display.owner,
                span: display.span,
                enum_name: contract.name,
                is_public: contract.is_public,
            }
            .emit(cx);
        }
    }
}
