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

// -----------------------------------------------------------------------------
// Violation: Divergent public names for one variant
// -----------------------------------------------------------------------------

/// Variant whose generated naming APIs expose different identities.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Variant identity involved in the finding.
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
            "`VariantNames` advertises a finite preferred vocabulary while `EnumString` accepts additional spellings",
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

// -----------------------------------------------------------------------------
// StrumDivergentVariantNameContracts: Coherent variant identity policy
// -----------------------------------------------------------------------------

/// Compares generated variant names with the enum's parsing and display spellings.
#[derive(Default)]
struct StrumDivergentVariantNameContracts {
    /// Effective Strum contracts consulted after generated items are associated.
    catalog: ContractCatalog,
}

crate::impl_late_lint! {
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
            if !contract.derives(StrumDerive::VariantNames)
                || !contract.derives(StrumDerive::EnumString)
            {
                continue;
            }

            let Some(variant) = contract.enabled_variants().find(|variant| {
                variant.is_default_capture
                    || variant
                        .parser_names
                        .iter()
                        .any(|name| name != &variant.preferred_name)
            }) else {
                continue;
            };

            Violation {
                owner: contract.owner,
                span: variant.span,
                variant: variant.name,
            }
            .emit(cx);
        }
    }
}
