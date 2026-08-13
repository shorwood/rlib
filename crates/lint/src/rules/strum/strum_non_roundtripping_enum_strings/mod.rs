extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::contracts::{ContractCatalog, EnumContract, StrumDerive};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Displayed enum text rejected by its parser
// -----------------------------------------------------------------------------

/// Variant whose generated display and parsing contracts do not round-trip.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Variant identity involved in the finding.
    variant: Symbol,
    /// Serialized spelling that the parser cannot recover as the same variant.
    output: String,
}

impl Violation {
    /// Classifies the resolved contract without relying on source spelling alone.
    fn classify(contract: &EnumContract) -> Option<Self> {
        let has_output = [
            StrumDerive::Display,
            StrumDerive::AsRefStr,
            StrumDerive::IntoStaticStr,
        ]
        .into_iter()
        .any(|derive| contract.derives(derive));

        if !contract.derives(StrumDerive::EnumString) || !has_output {
            return None;
        }
        for variant in contract
            .enabled_variants()
            .filter(|variant| !variant.has_payload)
        {
            let owners = contract
                .enabled_variants()
                .filter(|candidate| {
                    candidate.parser_names.iter().any(|name| {
                        name == &variant.preferred_name
                            || (candidate.is_ascii_case_insensitive
                                && name.eq_ignore_ascii_case(&variant.preferred_name))
                    })
                })
                .map(|candidate| candidate.def_id)
                .collect::<Vec<_>>();
            if owners.as_slice() != [variant.def_id] {
                return Some(Self {
                    owner: contract.owner,
                    span: variant.span,
                    variant: variant.name,
                    output: variant.preferred_name.clone(),
                });
            }
        }

        None
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` does not round-trip through Strum text",
            self.variant
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the generated output `{}` is not parsed back into this variant",
            self.output
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "align `serialize` and `to_string` names or remove the unintended text contract",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_NON_ROUNDTRIPPING_ENUM_STRINGS,
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
// StrumNonRoundtrippingEnumStrings: Round-tripping text policy
// -----------------------------------------------------------------------------

/// Compares each generated Strum output spelling with its parsing contract.
#[derive(Default)]
struct StrumNonRoundtrippingEnumStrings {
    /// Effective Strum contracts consulted after generated items are associated.
    catalog: ContractCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_NON_ROUNDTRIPPING_ENUM_STRINGS,
    Warn,
    "finds non-round-tripping Strum enum text contracts",
    StrumNonRoundtrippingEnumStrings::default()
}

impl LateLintPass<'_> for StrumNonRoundtrippingEnumStrings {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        for contract in self.catalog.contracts() {
            let Some(violation) = Violation::classify(&contract) else {
                continue;
            };
            violation.emit(cx);
        }
    }
}
