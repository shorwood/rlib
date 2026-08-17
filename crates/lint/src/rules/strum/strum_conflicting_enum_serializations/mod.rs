extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use super::utils::contracts::{ContractCatalog, EnumContract, StrumDerive, VariantContract};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Conflicting serialized names for one variant
// -----------------------------------------------------------------------------

/// Variant whose Strum attributes admit incompatible serialized names.
struct Violation {
    /// Declaration whose lint level governs this finding.
    owner: rustc_hir::HirId,
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Specific alias collision or unstable output choice shown to the user.
    detail: String,
}

impl Violation {
    /// Finds one parser name accepted by two different variants.
    fn conflicting_parser_name(
        contract: &EnumContract,
        left: &VariantContract,
        right: &VariantContract,
    ) -> Option<Self> {
        for left_name in &left.parser_names {
            for right_name in &right.parser_names {
                // Exact or case-folded parser-name overlap makes the two variants ambiguous.
                if left_name == right_name
                    || ((left.is_ascii_case_insensitive || right.is_ascii_case_insensitive)
                        && left_name.eq_ignore_ascii_case(right_name))
                {
                    return Some(Self {
                        owner: contract.owner,
                        span: right.span,
                        detail: format!(
                            "`{}` is accepted by both `{}` and `{}`",
                            right_name, left.name, right.name
                        ),
                    });
                }
            }
        }
        None
    }

    /// Classifies the resolved contract without relying on source spelling alone.
    fn classify(contract: &EnumContract) -> Option<Self> {
        if contract.derives(StrumDerive::EnumString) {
            let variants = contract.enabled_variants().collect::<Vec<_>>();
            for (index, left) in variants.iter().enumerate() {
                for right in variants.iter().skip(index + 1) {
                    // The first conflicting spelling is sufficient to diagnose this enum contract.
                    if let Some(violation) = Self::conflicting_parser_name(contract, left, right) {
                        return Some(violation);
                    }
                }
            }
        }

        let has_output = [
            StrumDerive::Display,
            StrumDerive::AsRefStr,
            StrumDerive::IntoStaticStr,
            StrumDerive::VariantNames,
        ]
        .into_iter()
        .any(|derive| contract.derives(derive));

        let variant = has_output.then(|| {
            contract
                .enabled_variants()
                .find(|variant| variant.parser_names.len() > 1 && !variant.has_explicit_output)
        })??;

        Some(Self {
            owner: contract.owner,
            span: variant.span,
            detail: format!(
                "`{}` selects its longest alias `{}` for output without an explicit policy",
                variant.name, variant.preferred_name
            ),
        })
    }
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("this Strum enum has conflicting serialization names")
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.detail)
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("assign unique aliases and an explicit `to_string` spelling")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_CONFLICTING_ENUM_SERIALIZATIONS,
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
// StrumConflictingEnumSerializations: Unambiguous enum text policy
// -----------------------------------------------------------------------------

/// Collects enum contracts and rejects ambiguous Strum text representations.
#[derive(Default)]
struct StrumConflictingEnumSerializations {
    /// Effective Strum contracts consulted after generated items are associated.
    catalog: ContractCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_CONFLICTING_ENUM_SERIALIZATIONS,
    Warn,
    "finds overlapping or unstable Strum enum serialization names",
    StrumConflictingEnumSerializations::default()
}

impl LateLintPass<'_> for StrumConflictingEnumSerializations {
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
