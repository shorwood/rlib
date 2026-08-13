extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{Expr, ImplItem, Item, Node};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use super::utils::contracts::{ContractCatalog, StrumAssociatedItem, StrumDerive};
use crate::utils::diagnostic::LateViolation;

/// Carries the `Violation` state used by this analysis.
struct Violation {
    /// Stores the `owner` value used by this analysis.
    owner: rustc_hir::HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `enum_name` value used by this analysis.
    enum_name: Symbol,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}::COUNT` is a total, not a filtered count",
            self.enum_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "the surrounding API promises a subset while `EnumCount` includes every declared variant",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed("count the declared subset explicitly or rename the API as a total count")
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            STRUM_FILTERED_ENUM_COUNT_CONTRACTS,
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

/// Carries the `CountUse` state used by this analysis.
struct CountUse {
    /// Stores the `owner` value used by this analysis.
    owner: rustc_hir::HirId,
    /// Stores the `span` value used by this analysis.
    span: Span,
    /// Stores the `enum_def` value used by this analysis.
    enum_def: LocalDefId,
}

/// Performs the `enclosing_name` step of the lint analysis.
fn enclosing_name(cx: &LateContext<'_>, hir_id: rustc_hir::HirId) -> Option<Symbol> {
    cx.tcx
        .hir_parent_iter(hir_id)
        .find_map(|(_, node)| match node {
            Node::Item(item) => item.kind.ident().map(|ident| ident.name),
            Node::ImplItem(ImplItem { ident, .. }) => Some(ident.name),
            _ => None,
        })
}

/// Performs the `is_subset_name` step of the lint analysis.
fn is_subset_name(name: Symbol) -> bool {
    // Perform the next step of the analysis.
    [
        "enabled",
        "visible",
        "supported",
        "actionable",
        "available",
        "selectable",
    ]
    .into_iter()
    .any(|token| name.as_str().contains(token))
}

#[derive(Default)]
/// Carries the `StrumFilteredEnumCountContracts` state used by this analysis.
struct StrumFilteredEnumCountContracts {
    /// Stores the `catalog` value used by this analysis.
    catalog: ContractCatalog,
    /// Stores the `uses` value used by this analysis.
    uses: Vec<CountUse>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub STRUM_FILTERED_ENUM_COUNT_CONTRACTS,
    Warn,
    "finds total Strum enum counts exposed as filtered subsets",
    StrumFilteredEnumCountContracts::default()
}

impl LateLintPass<'_> for StrumFilteredEnumCountContracts {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        self.catalog.check_item(cx, item);
    }

    fn check_expr(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Prepare the values used by this stage.
        let Some(enum_def) = (StrumAssociatedItem {
            trait_name: "EnumCount",
            item_name: "COUNT",
        })
        .enum_definition(cx, expression) else {
            return;
        };
        if !enclosing_name(cx, expression.hir_id).is_some_and(is_subset_name) {
            return;
        }

        // Update the accumulated analysis state.
        self.uses.push(CountUse {
            owner: expression.hir_id,
            span: expression.span.source_callsite(),
            enum_def,
        });
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        let contracts = self.catalog.contracts();
        for count_use in self.uses.drain(..) {
            // Prepare the values used by this stage.
            let Some(contract) = contracts.iter().find(|contract| {
                contract.def_id == count_use.enum_def
                    && contract.derives(StrumDerive::EnumCount)
                    && contract.variants.iter().any(|variant| {
                        variant.is_disabled
                            || variant.is_deprecated
                            || variant.has_payload
                            || matches!(
                                variant.name.as_str(),
                                "Disabled" | "Unknown" | "Other" | "Unspecified"
                            )
                    })
            }) else {
                continue;
            };

            // Perform the next step of the analysis.
            Violation {
                owner: count_use.owner,
                span: count_use.span,
                enum_name: contract.name,
            }
            .emit(cx);
        }
    }
}
