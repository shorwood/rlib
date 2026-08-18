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
    /// Classifies every non-round-tripping variant without relying on source spelling alone.
    fn classify(contract: &EnumContract) -> Vec<Self> {
        let has_output = [
            StrumDerive::Display,
            StrumDerive::AsRefStr,
            StrumDerive::IntoStaticStr,
        ]
        .into_iter()
        .any(|derive| contract.derives(derive));

        // Round-trip analysis requires both generated parsing and a generated string form.
        if !contract.derives(StrumDerive::EnumString) || !has_output {
            return Vec::new();
        }
        contract
            .enabled_variants()
            .filter(|variant| !variant.has_payload)
            .filter_map(|variant| {
                // `EnumString` emits match arms in declaration order. The first matching arm owns
                // an overlapping spelling; a default-capture variant owns only unmatched input.
                let parsed_variant = contract
                    .enabled_variants()
                    .filter(|candidate| !candidate.is_default_capture)
                    .find(|candidate| {
                        candidate.parser_names.iter().any(|name| {
                            name == &variant.preferred_name
                                || (candidate.is_ascii_case_insensitive
                                    && name.eq_ignore_ascii_case(&variant.preferred_name))
                        })
                    })
                    .or_else(|| {
                        contract
                            .enabled_variants()
                            .find(|candidate| candidate.is_default_capture)
                    });
                (parsed_variant.map(|candidate| candidate.def_id) != Some(variant.def_id)).then(
                    || Self {
                        owner: contract.owner,
                        span: variant.span,
                        variant: variant.name,
                        output: variant.preferred_name.clone(),
                    },
                )
            })
            .collect()
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

crate::impl_late_lint! {
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
            for violation in Violation::classify(&contract) {
                violation.emit(cx);
            }
        }
    }
}
