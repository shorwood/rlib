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
// Violation: Divergent discriminant representations
// -----------------------------------------------------------------------------

/// Enum whose generated discriminant contracts disagree.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Enum name quoted in the diagnostic.
    enum_name: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` generates an externally serialized discriminant schema",
            self.enum_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the generated schema evolves automatically with the payload enum and cannot version independently",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use an authored schema enum with explicit conversions when compatibility is independent",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_DIVERGENT_DISCRIMINANT_CONTRACTS,
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
// StrumDivergentDiscriminantContracts: Single discriminant policy
// -----------------------------------------------------------------------------

/// Compares authored discriminants with the values exposed by Strum conversions.
#[derive(Default)]
struct StrumDivergentDiscriminantContracts {
    /// Effective Strum contracts consulted after generated items are associated.
    catalog: ContractCatalog,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_DIVERGENT_DISCRIMINANT_CONTRACTS,
    Warn,
    "finds externally serialized generated Strum discriminants",
    StrumDivergentDiscriminantContracts::default()
}

impl LateLintPass<'_> for StrumDivergentDiscriminantContracts {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.contracts() {
            if !(contract.derives(StrumDerive::EnumDiscriminants)
                && contract.is_discriminant_external_schema)
            {
                continue;
            }

            Violation {
                owner: contract.owner,
                span: contract.span,
                enum_name: contract.name,
            }
            .emit(cx);
        }
    }
}
