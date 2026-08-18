extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def_id::DefId;
use rustc_hir::{HirId, Item, ItemKind};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, Ty};
use rustc_span::{Span, Symbol};

use super::free_function_analysis::FreeFunctionExt;
use super::identifier_case;

// -----------------------------------------------------------------------------
// NominalTypeExt: Nominal type resolution
// -----------------------------------------------------------------------------

/// Resolves a compiler type to its concrete nominal identity.
pub(super) trait NominalTypeExt {
    /// Follows references and aliases to a concrete nominal definition.
    fn nominal_def_id(self) -> Option<DefId>;

    /// Returns whether this output exposes a local owner directly or through a success wrapper.
    fn has_local_output_owner(self, cx: &LateContext<'_>) -> bool;
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

    fn has_local_output_owner(self, cx: &LateContext<'_>) -> bool {
        // Resolve direct ownership before inspecting supported success wrappers.
        let ty::Adt(definition, arguments) = self.kind() else {
            return false;
        };

        // A directly local aggregate already provides an output-side semantic owner.
        if definition.did().is_local() {
            return true;
        }

        // Descend through the represented success value of standard wrappers.
        let crate_name = cx.tcx.crate_name(definition.did().krate);
        let item_name = cx.tcx.item_name(definition.did());

        // Require a standard success wrapper before following its represented value.
        crate_name.as_str() == "core"
            && matches!(item_name.as_str(), "Option" | "Result")
            && arguments
                .types()
                .next()
                .is_some_and(|inner| inner.has_local_output_owner(cx))
    }
}

// -----------------------------------------------------------------------------
// ForeignType: Collected functions and public findings
// -----------------------------------------------------------------------------

/// One foreign parameter retained as named, labeled diagnostic evidence.
pub struct ForeignTypeParameterEvidence {
    /// Resolved nominal type name.
    pub(crate) name: Symbol,
    /// Authored parameter span carrying the type.
    pub(crate) span: Span,
}

/// One visible free function whose foreign parameters deserve an owning abstraction.
pub struct ForeignTypeFunctionFinding {
    /// Function node used to respect item-level lint attributes.
    pub(crate) hir_id: HirId,
    /// Function name span used as the primary diagnostic location.
    pub(crate) span: Span,
    /// Function name rendered in the diagnostic.
    pub(crate) name: Symbol,
    /// Foreign nominal types that remain plausible owners after ambient dependencies are removed.
    pub(crate) owners: Vec<ForeignTypeParameterEvidence>,
    /// Every foreign nominal parameter type, used when ownership remains ambiguous.
    pub(crate) candidates: Vec<ForeignTypeParameterEvidence>,
    /// Repeated infrastructure parameters excluded from semantic ownership.
    pub(crate) ambient: Vec<ForeignTypeParameterEvidence>,
}

/// Semantic identity and authored location of one nominal parameter.
struct ForeignTypeParameter {
    /// Resolved nominal type identity.
    def_id: DefId,
    /// Complete authored parameter span.
    span: Span,
}

/// Semantic signature retained until ambient dependencies can be inferred crate-wide.
struct ForeignTypeFunction {
    /// Function node used to respect item-level lint attributes.
    hir_id: HirId,
    /// Function name span used for diagnostics.
    span: Span,
    /// Function name rendered in diagnostics.
    name: Symbol,
    /// Distinct nominal parameter identities in declaration order.
    nominal_parameters: Vec<ForeignTypeParameter>,
}

/// Finds visible free functions that expose behavior through foreign nominal parameters.
#[derive(Default)]
pub struct ForeignTypeAnalyzer {
    /// Candidate signatures collected before ambient dependencies are known.
    functions: Vec<ForeignTypeFunction>,
}

impl ForeignTypeAnalyzer {
    /// Records an authored, Rust-ABI free function visible outside its defining module.
    pub(crate) fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Identify an ordinary top-level function and retain its compiler identity.
        let ItemKind::Fn { body, .. } = item.kind else {
            return;
        };

        // Function items without an identifier cannot define a discoverable owning operation.
        let Some(ident) = item.kind.ident() else {
            return;
        };

        // Non-snake names are generated or outside the ordinary free-function API convention.
        if !identifier_case::is_snake(ident.name.as_str()) {
            return;
        }
        let def_id = item.owner_id.def_id;

        // Ignore generated APIs and functions that cannot be named outside this module.
        if !item.is_authored_rust_free_function(cx)
            || !item.is_visible_outside_module(cx)
            || item.has_external_symbol(cx)
        {
            return;
        }

        // Resolve every distinct nominal parameter independently from its source spelling.
        let signature = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();
        let body = cx.tcx.hir_body(body);
        let mut nominal_parameters = Vec::new();
        for (parameter, source) in signature.inputs().iter().zip(body.params) {
            // Retain the first authored span for each resolved nominal parameter identity.
            let Some(def_id) = parameter.nominal_def_id() else {
                continue;
            };

            // Ignore later spellings of a nominal type already represented by its first span.
            if nominal_parameters
                .iter()
                .any(|parameter: &ForeignTypeParameter| parameter.def_id == def_id)
            {
                continue;
            }
            nominal_parameters.push(ForeignTypeParameter {
                def_id,
                span: source.span,
            });
        }

        // A local input or local success return is a stronger semantic owner than infrastructure
        // received through a foreign parameter.
        if nominal_parameters
            .iter()
            .any(|parameter| parameter.def_id.is_local())
            || signature.output().has_local_output_owner(cx)
        {
            return;
        }

        // A signature without any foreign nominal input has no foreign ownership candidate.
        if !nominal_parameters
            .iter()
            .any(|parameter| !parameter.def_id.is_local())
        {
            return;
        }

        // Retain only signatures that actually expose a foreign nominal type.
        self.functions.push(ForeignTypeFunction {
            hir_id: item.hir_id(),
            span: ident.span,
            name: ident.name,
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
                .map(|parameter| parameter.def_id)
                .filter(|def_id| !def_id.is_local())
            {
                // Count the dependency once and merge every distinct companion type.
                *occurrences.entry(foreign).or_default() += 1;

                // Merge every other nominal parameter as evidence of semantic diversity.
                companions.entry(foreign).or_default().extend(
                    function
                        .nominal_parameters
                        .iter()
                        .map(|parameter| parameter.def_id)
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
            .filter_map(|function| {
                let foreign = function
                    .nominal_parameters
                    .iter()
                    .filter(|parameter| !parameter.def_id.is_local())
                    .collect::<Vec<_>>();
                let owners = foreign
                    .iter()
                    .copied()
                    .filter(|parameter| !ambient.contains(&parameter.def_id))
                    .map(|parameter| ForeignTypeParameterEvidence {
                        name: cx.tcx.item_name(parameter.def_id),
                        span: parameter.span,
                    })
                    .collect::<Vec<_>>();

                // Functions whose foreign inputs are all ambient infrastructure lack a domain owner.
                if owners.is_empty() {
                    return None;
                }
                let candidates = foreign
                    .iter()
                    .copied()
                    .map(|parameter| ForeignTypeParameterEvidence {
                        name: cx.tcx.item_name(parameter.def_id),
                        span: parameter.span,
                    })
                    .collect();
                let ambient = foreign
                    .into_iter()
                    .filter(|parameter| ambient.contains(&parameter.def_id))
                    .map(|parameter| ForeignTypeParameterEvidence {
                        name: cx.tcx.item_name(parameter.def_id),
                        span: parameter.span,
                    })
                    .collect();
                Some(ForeignTypeFunctionFinding {
                    hir_id: function.hir_id,
                    span: function.span,
                    name: function.name,
                    owners,
                    candidates,
                    ambient,
                })
            })
            .collect()
    }
}
