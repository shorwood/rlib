extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::BTreeMap;

use rustc_errors::DiagDecorator;
use rustc_hir::Item;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use super::contracts::{DiagnosticCatalog, DiagnosticContract, DiagnosticMember};
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Diagnostic code shared by distinct contracts
// -----------------------------------------------------------------------------

/// One diagnostic contract claiming a stable code.
struct ViolationUse {
    /// Declaration carrying the code.
    span: Span,
    /// Qualified diagnostic name shown to the author.
    diagnostic: String,
}

/// All distinct diagnostics that claim the same code.
struct Violation {
    /// Duplicated stable identifier.
    code: String,
    /// Diagnostic declarations sharing that identifier.
    uses: Vec<ViolationUse>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "diagnostic code `{}` identifies distinct diagnostics",
            self.code
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the code is shared by {}",
            self.uses
                .iter()
                .map(|usage| format!("`{}`", usage.diagnostic))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("assign a unique stable code to each distinct diagnostic contract")
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary = self.uses[0].span;
        cx.emit_span_lint(
            MIETTE_DUPLICATE_DIAGNOSTIC_CODES,
            primary,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                for usage in &self.uses {
                    diag.span_label(usage.span, format!("`{}` uses this code", usage.diagnostic));
                }
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Records the effective code inherited or declared by an enum variant.
fn record_member(
    codes: &mut BTreeMap<String, Vec<ViolationUse>>,
    contract: &DiagnosticContract,
    member: &DiagnosticMember,
) {
    let Some(code) = member
        .metadata
        .code
        .as_ref()
        .or(contract.metadata.code.as_ref())
    else {
        return;
    };
    codes.entry(code.clone()).or_default().push(ViolationUse {
        span: member.span,
        diagnostic: format!("{}::{}", contract.name, member.name),
    });
}

/// Records the effective codes exposed by one diagnostic contract.
fn record_contract(codes: &mut BTreeMap<String, Vec<ViolationUse>>, contract: &DiagnosticContract) {
    if contract.members.is_empty() {
        if let Some(code) = &contract.metadata.code {
            codes.entry(code.clone()).or_default().push(ViolationUse {
                span: contract.span,
                diagnostic: contract.name.clone(),
            });
        }
        return;
    }
    for member in &contract.members {
        record_member(codes, contract, member);
    }
}

// -----------------------------------------------------------------------------
// MietteDuplicateDiagnosticCodes: Unique machine identity policy
// -----------------------------------------------------------------------------

/// Collects diagnostic contracts before comparing their effective codes.
#[derive(Default)]
struct MietteDuplicateDiagnosticCodes {
    /// Derived diagnostic declarations in the crate.
    catalog: DiagnosticCatalog,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub MIETTE_DUPLICATE_DIAGNOSTIC_CODES,
    Warn,
    "finds distinct Miette diagnostics sharing one code",
    MietteDuplicateDiagnosticCodes::default()
}

impl LateLintPass<'_> for MietteDuplicateDiagnosticCodes {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        let mut codes = BTreeMap::<String, Vec<ViolationUse>>::new();
        for contract in self.catalog.derived_contracts() {
            record_contract(&mut codes, contract);
        }
        for (code, uses) in codes {
            if uses.len() <= 1 {
                continue;
            }
            Violation { code, uses }.emit(cx);
        }
    }
}
