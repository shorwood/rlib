extern crate rustc_abi;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_abi::ExternAbi;
use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::{Span, Symbol};

// -----------------------------------------------------------------------------
// NominalTypeExt: Nominal type resolution
// -----------------------------------------------------------------------------

/// Resolves a compiler type to its concrete nominal identity.
pub trait NominalTypeExt {
    /// Follows references and aliases to a concrete nominal definition.
    fn nominal_def_id(self) -> Option<DefId>;
}

impl NominalTypeExt for Ty<'_> {
    fn nominal_def_id(mut self) -> Option<DefId> {
        while let ty::Ref(_, inner, _) = *self.kind() {
            self = inner;
        }
        match self.kind() {
            ty::Adt(definition, _) => Some(definition.did()),
            _ => None,
        }
    }
}

// -----------------------------------------------------------------------------
// ForeignType: Collected functions and public findings
// -----------------------------------------------------------------------------

/// One visible free function whose foreign parameters deserve an owning abstraction.
pub struct ForeignTypeFunctionFinding {
    /// Function name span used as the primary diagnostic location.
    pub span: Span,
    /// Foreign nominal types that remain plausible owners after ambient dependencies are removed.
    pub owners: Vec<Symbol>,
    /// Every foreign nominal parameter type, used when ownership remains ambiguous.
    pub candidates: Vec<Symbol>,
}

/// Semantic signature retained until ambient dependencies can be inferred crate-wide.
struct ForeignTypeFunction {
    /// Function name span used for diagnostics.
    span: Span,
    /// Distinct nominal parameter identities in declaration order.
    nominal_parameters: Vec<DefId>,
}

/// Finds visible free functions that expose behavior through foreign nominal parameters.
#[derive(Default)]
pub struct ForeignTypeAnalyzer {
    /// Candidate signatures collected before ambient dependencies are known.
    functions: Vec<ForeignTypeFunction>,
}

impl ForeignTypeAnalyzer {
    /// Determines whether another module can name the function.
    fn is_visible_outside_module(cx: &LateContext<'_>, item: &Item<'_>) -> bool {
        if item.vis_span.is_empty() {
            return false;
        }
        let source_map = cx.sess().source_map();
        let visibility = source_map.span_to_snippet(item.vis_span);
        visibility.is_ok_and(|visibility| visibility.trim() != "pub(self)")
    }

    /// Returns whether an exported symbol is an ABI integration entry point rather than an API.
    fn has_external_symbol(cx: &LateContext<'_>, item: &Item<'_>) -> bool {
        cx.tcx
            .codegen_fn_attrs(item.owner_id.def_id)
            .contains_extern_indicator()
    }

    /// Records an authored, Rust-ABI free function visible outside its defining module.
    pub(crate) fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Identify an ordinary top-level function and retain its compiler identity.
        let ItemKind::Fn { sig, .. } = item.kind else {
            return;
        };
        let Some(ident) = item.kind.ident() else {
            return;
        };
        let def_id = item.owner_id.def_id;

        // Associated and nested functions are already colocated with an owning declaration.
        let Some(parent) = cx.tcx.opt_local_parent(def_id) else {
            return;
        };
        if cx.tcx.def_kind(parent) != DefKind::Mod || sig.header.abi != ExternAbi::Rust {
            return;
        }

        // Ignore generated APIs and functions that cannot be named outside this module.
        if item.span.in_external_macro(cx.sess().source_map())
            || !Self::is_visible_outside_module(cx, item)
            || Self::has_external_symbol(cx, item)
        {
            return;
        }

        // Resolve every distinct nominal parameter independently from its source spelling.
        let signature = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();
        let mut nominal_parameters = Vec::new();
        for parameter in signature.inputs() {
            let Some(nominal) = parameter.nominal_def_id() else {
                continue;
            };
            if nominal_parameters.contains(&nominal) {
                continue;
            }
            nominal_parameters.push(nominal);
        }
        if !nominal_parameters.iter().any(|def_id| !def_id.is_local()) {
            return;
        }

        // Retain only signatures that actually expose a foreign nominal type.
        self.functions.push(ForeignTypeFunction {
            span: ident.span,
            nominal_parameters,
        });
    }

    /// Infers foreign types that recur as infrastructure beside several semantic subjects.
    fn ambient_foreign_types(&self) -> HashSet<DefId> {
        let mut occurrences = HashMap::<DefId, usize>::new();
        let mut companions = HashMap::<DefId, HashSet<DefId>>::new();
        for function in &self.functions {
            for foreign in function
                .nominal_parameters
                .iter()
                .copied()
                .filter(|def_id| !def_id.is_local())
            {
                // Count the dependency once and merge every distinct companion type.
                *occurrences.entry(foreign).or_default() += 1;

                // Merge every other nominal parameter as evidence of semantic diversity.
                companions.entry(foreign).or_default().extend(
                    function
                        .nominal_parameters
                        .iter()
                        .copied()
                        .filter(|candidate| *candidate != foreign),
                );
            }
        }
        occurrences
            .into_iter()
            .filter_map(|(def_id, count)| {
                let companion_count = companions.get(&def_id).map_or(0, HashSet::len);
                (count >= 2 && companion_count >= 2).then_some(def_id)
            })
            .collect()
    }

    /// Returns findings after distinguishing repeated infrastructure from semantic subjects.
    pub(crate) fn findings(&self, cx: &LateContext<'_>) -> Vec<ForeignTypeFunctionFinding> {
        let ambient = self.ambient_foreign_types();
        self.functions
            .iter()
            .map(|function| {
                let foreign = function
                    .nominal_parameters
                    .iter()
                    .copied()
                    .filter(|def_id| !def_id.is_local())
                    .collect::<Vec<_>>();
                let owners = foreign
                    .iter()
                    .copied()
                    .filter(|def_id| !ambient.contains(def_id))
                    .map(|def_id| cx.tcx.item_name(def_id))
                    .collect();
                let candidates = foreign
                    .into_iter()
                    .map(|def_id| cx.tcx.item_name(def_id))
                    .collect();
                ForeignTypeFunctionFinding {
                    span: function.span,
                    owners,
                    candidates,
                }
            })
            .collect()
    }
}
