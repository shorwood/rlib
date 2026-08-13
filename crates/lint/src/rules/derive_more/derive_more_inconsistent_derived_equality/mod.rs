extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

use super::contracts::DeriveMoreContractCatalog;
use crate::utils::diagnostic::LateViolation;

struct EqualitySelection {
    span: Span,
    name: Symbol,
    skipped: usize,
}

struct Violation {
    selection: EqualitySelection,
    conflicting: Vec<&'static str>,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "derived equality for `{}` conflicts with its derived {} contract{}",
            self.selection.name,
            self.conflicting.join(" and "),
            if self.conflicting.len() == 1 { "" } else { "s" }
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "equality skips fields that the structural hash or total-order implementation still observes, violating their required agreement",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "use the same selected fields for equality, hashing, and ordering, or move the alternative identity into a dedicated wrapper type",
        )
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            DERIVE_MORE_INCONSISTENT_DERIVED_EQUALITY,
            self.selection.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(self.selection.span, "derive_more equality is configured here");
                diag.span_label(
                    self.selection.span,
                    format!(
                        "{} field{} omitted only from equality",
                        self.selection.skipped,
                        if self.selection.skipped == 1 { " is" } else { "s are" }
                    ),
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

#[derive(Default)]
struct DeriveMoreInconsistentDerivedEquality {
    catalog: DeriveMoreContractCatalog,
    selections: HashMap<LocalDefId, EqualitySelection>,
    law_traits: HashMap<LocalDefId, HashSet<&'static str>>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_INCONSISTENT_DERIVED_EQUALITY,
    Warn,
    "rejects derive_more equality that violates hash or order laws",
    DeriveMoreInconsistentDerivedEquality::default()
}

impl<'tcx> LateLintPass<'tcx> for DeriveMoreInconsistentDerivedEquality {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.catalog.check_item(cx, item);
        if item.span.from_expansion() {
            if let Some((target, contract)) = generated_law_trait(cx, item) {
                self.law_traits.entry(target).or_default().insert(contract);
            }
            return;
        }
        let ItemKind::Struct(identifier, _, _) = item.kind else {
            return;
        };
        let skipped = equality_skip_count(cx, item);
        if skipped != 0 {
            self.selections.insert(
                item.owner_id.def_id,
                EqualitySelection {
                    span: identifier.span,
                    name: identifier.name,
                    skipped,
                },
            );
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for (definition, selection) in self.selections.drain() {
            if self
                .catalog
                .derived_type(definition, "PartialEq")
                .is_none()
            {
                continue;
            }
            let Some(contracts) = self.law_traits.get(&definition) else {
                continue;
            };
            let conflicting = ["Hash", "Ord"]
                .into_iter()
                .filter(|contract| contracts.contains(contract))
                .collect::<Vec<_>>();
            if !conflicting.is_empty() {
                Violation {
                    selection,
                    conflicting,
                }
                .emit(cx);
            }
        }
    }
}

fn equality_skip_count(cx: &LateContext<'_>, item: &Item<'_>) -> usize {
    let Ok(source) = cx.sess().source_map().span_to_snippet(item.span) else {
        return 0;
    };
    let Ok(item) = syn::parse_str::<syn::ItemStruct>(&source) else {
        return 0;
    };
    item.fields
        .iter()
        .filter(|field| {
            field.attrs.iter().any(|attribute| {
                if !matches!(attribute.path().get_ident().map(ToString::to_string).as_deref(), Some("partial_eq" | "eq")) {
                    return false;
                }
                attribute.meta.require_list().is_ok_and(|list| {
                    let arguments = list.tokens.to_string();
                    arguments.contains("skip") || arguments.contains("ignore")
                })
            })
        })
        .count()
}

fn generated_law_trait(
    cx: &LateContext<'_>,
    item: &Item<'_>,
) -> Option<(LocalDefId, &'static str)> {
    if !matches!(item.kind, ItemKind::Impl(_)) {
        return None;
    }
    let trait_ref = cx
        .tcx
        .impl_opt_trait_ref(item.owner_id.def_id)?
        .instantiate_identity();
    let contract = match cx.tcx.item_name(trait_ref.def_id).as_str() {
        "Hash" => "Hash",
        "Ord" => "Ord",
        _ => return None,
    };
    let is_matching_derive = item.span.macro_backtrace().any(|expansion| {
        expansion
            .macro_def_id
            .is_some_and(|definition| cx.tcx.item_name(definition).as_str() == contract)
    });
    if !is_matching_derive {
        return None;
    }
    let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
        return None;
    };
    Some((definition.did().as_local()?, contract))
}
