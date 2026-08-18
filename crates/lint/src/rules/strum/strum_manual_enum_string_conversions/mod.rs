extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{ImplItem, Item};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::authored_contracts::{DisplayCandidate, StaticValue, VariantValueFamily};
use super::utils::contracts::ContractCatalog;
use crate::config::providers::DisplayProvider;
use crate::config::store::ConfigStore;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Hand-maintained enum string conversion
// -----------------------------------------------------------------------------

/// Complete enum-to-string mapping reproducible by the configured provider.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Enum name quoted in the diagnostic.
    enum_name: Symbol,
    /// Whether the declaration is visible outside its defining module.
    is_public: bool,
    /// Generated Strum surface that exactly replaces this contract.
    replacement: &'static str,
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
        Cow::Borrowed(self.replacement)
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

// -----------------------------------------------------------------------------
// StrumManualEnumStringConversions: Declarative display policy
// -----------------------------------------------------------------------------

/// Correlates manual string conversions with complete enum value families.
#[derive(Default)]
struct StrumManualEnumStringConversions {
    /// Effective Strum contracts consulted after generated items are associated.
    catalog: ContractCatalog,
    /// Complete manual variant-to-value method families awaiting contract analysis.
    families: Vec<VariantValueFamily>,
    /// Manual `Display` implementations that may duplicate generated output.
    displays: Vec<DisplayCandidate>,
    /// Framework selected to replace an exact manual `Display` implementation.
    display_provider: Option<DisplayProvider>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_MANUAL_ENUM_STRING_CONVERSIONS,
    Warn,
    "finds manual enum-to-static-string conversions reproducible by Strum",
    StrumManualEnumStringConversions::new()
}

impl StrumManualEnumStringConversions {
    /// Starts string-conversion analysis with no collected method families.
    fn new() -> Self {
        Self {
            catalog: ContractCatalog::default(),
            families: Vec::new(),
            displays: Vec::new(),
            display_provider: ConfigStore::get().derive_resolution.enum_display(),
        }
    }
}

impl LateLintPass<'_> for StrumManualEnumStringConversions {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // A manual display candidate is recorded separately and needs no value-family analysis.
        if let Some(display) = DisplayCandidate::from_impl_item(cx, item) {
            self.displays.push(display);
            return;
        }

        // Associated items without a complete enum-value family are unrelated conversions.
        let Some(family) = VariantValueFamily::from_impl_item(cx, item) else {
            return;
        };

        // Only complete static-string accessors match the generated conversion contract.
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
                    && contract.variants.iter().all(|variant| !variant.is_disabled)
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
                replacement: "derive `strum::AsRefStr` and migrate callers to `AsRef<str>`",
            }
            .emit(cx);
        }

        // Manual display replacement is valid only when Strum is the selected provider.
        if DisplayProvider::selected(cx, self.display_provider)
            != Some(DisplayProvider::StrumDisplay)
        {
            return;
        }
        for display in self.displays.drain(..) {
            let Some(contract) = contracts.iter().find(|contract| {
                contract.def_id == display.enum_def
                    && contract.variants.iter().all(|variant| !variant.is_disabled)
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
                replacement: "derive `strum::Display` and remove the equivalent formatting implementation",
            }
            .emit(cx);
        }
    }
}
