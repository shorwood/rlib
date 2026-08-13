extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::contracts::{ContractCatalog, StrumDerive};
use crate::utils::diagnostic::LateViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: rustc_hir::HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `variant` value used by this analysis.
    variant: Symbol,
    /// Stores the `derives` value used by this analysis.
    derives: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!("`{}` receives a defaulted payload", self.variant))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "{} constructs this variant without domain payload input",
            self.derives
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "disable this variant for those derives or use an explicit payload constructor",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_DEFAULTED_PAYLOAD_ENUM_CONSTRUCTION,
            self.owner,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
/// Carries the `StrumDefaultedPayloadEnumConstruction` state used by this analysis.
struct StrumDefaultedPayloadEnumConstruction {
    /// Stores the `catalog` value used by this analysis.
    catalog: ContractCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_DEFAULTED_PAYLOAD_ENUM_CONSTRUCTION,
    Warn,
    "finds Strum derives that invent enum payload values",
    StrumDefaultedPayloadEnumConstruction::default()
}

impl LateLintPass<'_> for StrumDefaultedPayloadEnumConstruction {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.contracts() {
            for variant in contract
                .enabled_variants()
                .filter(|variant| variant.has_domain_payload)
            {
                // Prepare the values used by this stage.
                let mut derives = Vec::new();
                if contract.derives(StrumDerive::EnumIter) {
                    derives.push("`EnumIter`");
                }
                if contract.derives(StrumDerive::FromRepr) {
                    derives.push("`FromRepr`");
                }

                // Reject inputs that do not satisfy this stage.
                if contract.derives(StrumDerive::EnumString) && !variant.is_default_capture {
                    derives.push("`EnumString`");
                }
                if derives.is_empty() {
                    continue;
                }

                // Perform the next step of the analysis.
                Violation {
                    owner: contract.owner,
                    span: variant.span,
                    variant: variant.name,
                    derives: derives.join(", "),
                }
                .emit(cx);
            }
        }
    }
}
