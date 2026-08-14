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
// Violation: Payload variant constructed from defaults
// -----------------------------------------------------------------------------

/// Generated construction that invents payload values through `Default`.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Variant identity involved in the finding.
    variant: Symbol,
    /// Derive macros that establish the generated behavior.
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

// -----------------------------------------------------------------------------
// StrumDefaultedPayloadEnumConstruction: Explicit payload policy
// -----------------------------------------------------------------------------

/// Rejects generated construction that silently fills domain payloads with defaults.
#[derive(Default)]
struct StrumDefaultedPayloadEnumConstruction {
    /// Effective Strum contracts consulted after generated items are associated.
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
                let mut derives = Vec::new();
                if contract.derives(StrumDerive::EnumIter) {
                    derives.push("`EnumIter`");
                }
                if contract.derives(StrumDerive::FromRepr) {
                    derives.push("`FromRepr`");
                }

                if contract.derives(StrumDerive::EnumString)
                    && !variant.is_default_capture
                    && !variant.has_explicit_string_payload
                {
                    derives.push("`EnumString`");
                }
                if derives.is_empty() {
                    continue;
                }

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
