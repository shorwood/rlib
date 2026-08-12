extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::Span;

use super::utils::contracts::{ContractCatalog, EnumContract, StrumDerive};
use crate::utils::diagnostic::LateViolation;

struct Violation {
    owner: rustc_hir::HirId,
    span: Span,
    detail: String,
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

#[derive(Default)]
struct StrumConflictingEnumSerializations {
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
            if let Some(violation) = classify(&contract) {
                violation.emit(cx);
            }
        }
    }
}

fn classify(contract: &EnumContract) -> Option<Violation> {
    if contract.derives(StrumDerive::EnumString) {
        let variants = contract.enabled_variants().collect::<Vec<_>>();
        for (index, left) in variants.iter().enumerate() {
            for right in variants.iter().skip(index + 1) {
                for left_name in &left.parser_names {
                    for right_name in &right.parser_names {
                        if left_name == right_name
                            || ((left.ascii_case_insensitive || right.ascii_case_insensitive)
                                && left_name.eq_ignore_ascii_case(right_name))
                        {
                            return Some(Violation {
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
    Some(Violation {
        owner: contract.owner,
        span: variant.span,
        detail: format!(
            "`{}` selects its longest alias `{}` for output without an explicit policy",
            variant.name, variant.preferred_name
        ),
    })
}
