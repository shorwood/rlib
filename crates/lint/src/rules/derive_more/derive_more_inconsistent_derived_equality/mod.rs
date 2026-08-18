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

use super::utils::contracts::DeriveMoreContractCatalog;
use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Inconsistent generated equality laws
// -----------------------------------------------------------------------------

/// Fields omitted from `derive_more` equality for one local type.
struct EqualitySelection {
    /// Authored declaration or expression range used as the diagnostic anchor.
    span: Span,
    /// Local type name used to identify the affected derive contract.
    name: Symbol,
    /// Number of fields deliberately excluded from equality.
    skipped: usize,
}

/// Equality selection paired with generated law traits that observe different fields.
struct Violation {
    /// Authored equality field selection that establishes the mismatch.
    selection: EqualitySelection,
    /// Generated law traits that still observe the skipped fields.
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
                diag.span_label(
                    self.selection.span,
                    "derive_more equality is configured here",
                );
                diag.span_label(
                    self.selection.span,
                    format!(
                        "{} field{} omitted only from equality",
                        self.selection.skipped,
                        if self.selection.skipped == 1 {
                            " is"
                        } else {
                            "s are"
                        }
                    ),
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

/// Identifies a generated law trait and the local type it governs.
struct GeneratedLawTrait {
    /// Local type receiving the generated implementation.
    target: LocalDefId,
    /// Law trait implemented by the generated code.
    contract: &'static str,
}

impl GeneratedLawTrait {
    /// Recognizes a generated hash or ordering implementation.
    fn for_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Only implementation items can establish a generated law trait.
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
            // Other traits do not impose equality coherence with this rule's selection.
            _ => return None,
        };

        let is_matching_derive = item.span.macro_backtrace().any(|expansion| {
            expansion
                .macro_def_id
                .is_some_and(|definition| cx.tcx.item_name(definition).as_str() == contract)
        });

        // Manual implementations are not evidence about a generated structural law.
        if !is_matching_derive {
            return None;
        }

        // Only algebraic self types can join an authored derive contract by definition.
        let ty::Adt(definition, _) = trait_ref.self_ty().kind() else {
            return None;
        };
        Some(Self {
            target: definition.did().as_local()?,
            contract,
        })
    }
}

// -----------------------------------------------------------------------------
// DeriveMoreInconsistentDerivedEquality: Coherent equality policy
// -----------------------------------------------------------------------------

/// Correlates `derive_more` equality selections with generated hash and order laws.
#[derive(Default)]
struct DeriveMoreInconsistentDerivedEquality {
    /// Authored type contracts and `derive_more` expansions consulted by this rule.
    catalog: DeriveMoreContractCatalog,
    /// Per-type equality selections recovered from authored attributes.
    selections: HashMap<LocalDefId, EqualitySelection>,
    /// Generated `Hash` and `Ord` implementations awaiting cross-contract comparison.
    law_traits: HashMap<LocalDefId, HashSet<&'static str>>,
}

crate::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub DERIVE_MORE_INCONSISTENT_DERIVED_EQUALITY,
    Warn,
    "rejects derive_more equality that violates hash or order laws",
    DeriveMoreInconsistentDerivedEquality::default()
}

impl DeriveMoreInconsistentDerivedEquality {
    /// Projects equality-bearing fields from supported aggregate declarations.
    fn aggregate_fields(item: &syn::Item) -> Vec<&syn::Field> {
        match item {
            syn::Item::Struct(structure) => structure.fields.iter().collect(),
            syn::Item::Enum(enumeration) => enumeration
                .variants
                .iter()
                .flat_map(|variant| variant.fields.iter())
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Counts fields omitted from `derive_more` equality.
    fn skipped_field_count(cx: &LateContext<'_>, item: &Item<'_>) -> usize {
        // Missing authored item text cannot establish field-level equality selection.
        let Ok(source) = cx.sess().source_map().span_to_snippet(item.span) else {
            return 0;
        };

        // Unparseable item text cannot provide trustworthy derive attributes.
        let Ok(item) = syn::parse_str::<syn::Item>(&source) else {
            return 0;
        };
        let fields = Self::aggregate_fields(&item);
        fields
            .into_iter()
            .filter(|field| {
                field.attrs.iter().any(|attribute| {
                    // Unrelated field attributes cannot mark an equality omission.
                    if !matches!(
                        attribute
                            .path()
                            .get_ident()
                            .map(ToString::to_string)
                            .as_deref(),
                        Some("partial_eq" | "eq")
                    ) {
                        return false;
                    }
                    attribute.meta.require_list().is_ok_and(|list| {
                        list.tokens
                            .to_string()
                            .chars()
                            .filter(|character| !character.is_whitespace())
                            .collect::<String>()
                            == "skip"
                    })
                })
            })
            .count()
    }
}

impl<'tcx> LateLintPass<'tcx> for DeriveMoreInconsistentDerivedEquality {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.catalog.check_item(cx, item);

        // Expanded items contribute generated law evidence rather than authored selections.
        if item.span.from_expansion() {
            if let Some(generated) = GeneratedLawTrait::for_item(cx, item) {
                self.law_traits
                    .entry(generated.target)
                    .or_default()
                    .insert(generated.contract);
            }
            return;
        }

        // Only structs and enums expose authored field equality selections.
        let (ItemKind::Struct(identifier, _, _) | ItemKind::Enum(identifier, _, _)) = item.kind
        else {
            return;
        };
        let skipped = Self::skipped_field_count(cx, item);

        // Complete equality selection cannot conflict with structural hash or order.
        if skipped == 0 {
            return;
        }

        self.selections.insert(
            item.owner_id.def_id,
            EqualitySelection {
                span: identifier.span,
                name: identifier.name,
                skipped,
            },
        );
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let mut selections = self.selections.drain().collect::<Vec<_>>();
        selections.sort_by_key(|(_, selection)| selection.span.lo());
        for (definition, selection) in selections {
            // Without generated equality, the authored selection establishes no law contract.
            if self.catalog.derived_type(definition, "PartialEq").is_none() {
                continue;
            }

            // Types without generated hash or order have no competing structural observation.
            let Some(contracts) = self.law_traits.get(&definition) else {
                continue;
            };

            let conflicting = ["Hash", "Ord"]
                .into_iter()
                .filter(|contract| contracts.contains(contract))
                .collect::<Vec<_>>();

            // No selected law trait observes fields omitted from equality.
            if conflicting.is_empty() {
                continue;
            }
            Violation {
                selection,
                conflicting,
            }
            .emit(cx);
        }
    }
}
