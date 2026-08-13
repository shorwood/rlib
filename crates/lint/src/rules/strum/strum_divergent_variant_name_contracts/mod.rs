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
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` has divergent Strum name contracts",
            self.variant
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "`VariantNames` advertises one preferred name while `EnumString` accepts additional aliases",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("separate canonical names from parser aliases in the surrounding API")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_DIVERGENT_VARIANT_NAME_CONTRACTS,
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
/// Carries the `StrumDivergentVariantNameContracts` state used by this analysis.
struct StrumDivergentVariantNameContracts {
    /// Stores the `catalog` value used by this analysis.
    catalog: ContractCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_DIVERGENT_VARIANT_NAME_CONTRACTS,
    Warn,
    "finds divergent Strum variant-name vocabularies",
    StrumDivergentVariantNameContracts::default()
}

impl LateLintPass<'_> for StrumDivergentVariantNameContracts {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.contracts() {
            // Reject inputs that do not satisfy this stage.
            if !contract.derives(StrumDerive::VariantNames)
                || !contract.derives(StrumDerive::EnumString)
            {
                continue;
            }

            // Prepare the values used by this stage.
            let Some(variant) = contract.enabled_variants().find(|variant| {
                variant
                    .parser_names
                    .iter()
                    .any(|name| name != &variant.preferred_name)
            }) else {
                continue;
            };

            // Perform the next step of the analysis.
            Violation {
                owner: contract.owner,
                span: variant.span,
                variant: variant.name,
            }
            .emit(cx);
        }
    }
}
