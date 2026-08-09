extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def::DefKind;
use rustc_hir::{Item, ItemKind, TraitItem, TraitItemKind};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, Ty, TypeVisitableExt};
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

use crate::utils::foreign_type_analysis::NominalTypeExt;

// -----------------------------------------------------------------------------
// ExtensionTargetExt: Extension target classification
// -----------------------------------------------------------------------------

/// Classifies compiler types that can make a local trait an extension trait.
trait ExtensionTargetExt {
    /// Returns whether this is a foreign nominal type or a generic blanket target.
    fn is_extension_target(&self) -> bool;
}

impl ExtensionTargetExt for Ty<'_> {
    fn is_extension_target(&self) -> bool {
        let mut target = *self;
        while let ty::Ref(_, inner, _) = *target.kind() {
            target = inner;
        }
        match target.kind() {
            ty::Adt(definition, _) => !definition.did().is_local(),
            _ => target.has_param(),
        }
    }
}

// -----------------------------------------------------------------------------
// ExtensionTrait: Semantic model and public findings
// -----------------------------------------------------------------------------

/// Reason a focused extension trait has become incoherent.
pub enum ExtensionTraitProblem {
    /// Methods operate on several unrelated concrete subject families.
    MultipleSubjects(
        /// Concrete family names found across the trait's methods.
        Vec<Symbol>,
    ),
    /// The trait exceeds the configured method budget.
    TooManyMethods(
        /// Number of methods declared by the trait.
        usize,
    ),
}

/// One extension trait whose API is too broad to remain a single discoverable concept.
pub struct ExtensionTraitFinding {
    /// Trait name span used as the primary diagnostic location.
    pub span: Span,
    /// Trait name shown in guidance.
    pub name: Symbol,
    /// Independent coherence failures found on the trait.
    pub problems: Vec<ExtensionTraitProblem>,
}

/// One extension trait whose declaration and authored impls do not form one group.
pub struct ExtensionTraitPlacementFinding {
    /// First misplaced impl, or the trait when its whole group is cross-module.
    pub span: Span,
    /// Trait declaration span linked to the misplaced group.
    pub trait_span: Span,
    /// Trait name shown in guidance.
    pub name: Symbol,
}

/// One authored module item used to test declaration adjacency.
struct ExtensionTraitModuleItem {
    /// Definition identity distinguishing traits and impl blocks from intervening items.
    def_id: LocalDefId,
    /// Owning module identity.
    module: LocalDefId,
    /// Source position used to restore declaration order.
    span: Span,
}

/// One local trait declaration and the subjects used by its methods.
struct ExtensionTraitDefinition {
    /// Trait name span.
    span: Span,
    /// Trait name.
    name: Symbol,
    /// Owning module.
    module: LocalDefId,
    /// Number of declared methods, including receiver-only accessors.
    method_count: usize,
    /// Concrete nonreceiver nominal parameter types.
    subjects: HashSet<rustc_span::def_id::DefId>,
}

/// One authored impl that makes a local trait an extension trait.
struct ExtensionTraitImpl {
    /// Impl definition identity.
    def_id: LocalDefId,
    /// Extended local trait identity.
    trait_def_id: LocalDefId,
    /// Owning module.
    module: LocalDefId,
    /// Complete impl span.
    span: Span,
}

/// Collects the shared semantic model used by extension-trait policy lints.
#[derive(Default)]
pub struct ExtensionTraitAnalyzer {
    /// Local trait declarations indexed by definition identity.
    traits: HashMap<LocalDefId, ExtensionTraitDefinition>,
    /// Authored foreign-target and blanket impls.
    extension_impls: Vec<ExtensionTraitImpl>,
    /// Every authored module item, including declarations that interrupt a group.
    module_items: Vec<ExtensionTraitModuleItem>,
}

impl ExtensionTraitAnalyzer {
    /// Records trait declarations, extension impls, and potential group interruptions.
    pub fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Retain every authored top-level declaration so unrelated items can interrupt a group.
        let def_id = item.owner_id.def_id;
        let Some(parent) = cx.tcx.opt_local_parent(def_id) else {
            return;
        };
        if cx.tcx.def_kind(parent) != DefKind::Mod
            || item.span.in_external_macro(cx.sess().source_map())
        {
            return;
        }

        // Record the declaration's module and physical position before classifying its kind.
        let module = parent;
        self.module_items.push(ExtensionTraitModuleItem {
            def_id,
            module,
            span: item.span,
        });

        // Initialize semantic state for a local trait declaration.
        if let ItemKind::Trait(..) = item.kind {
            // Resolve the declaration name before creating its empty method analysis.
            let Some(ident) = item.kind.ident() else {
                return;
            };

            // Seed the trait record before its associated items are visited.
            let definition = ExtensionTraitDefinition {
                span: ident.span,
                name: ident.name,
                module,
                method_count: 0,
                subjects: HashSet::new(),
            };

            // Retain the initialized trait and stop before impl classification.
            self.traits.insert(def_id, definition);
            return;
        }

        // Resolve trait impls and retain only local traits on extension targets.
        let ItemKind::Impl(_) = item.kind else {
            return;
        };

        // Resolve the local trait implemented by this authored block.
        let Some(trait_ref) = cx
            .tcx
            .impl_opt_trait_ref(def_id)
            .map(|trait_ref| trait_ref.instantiate_identity().def_id)
        else {
            return;
        };

        // Only locally declared traits are governed by this crate's organization policy.
        let Some(trait_def_id) = trait_ref.as_local() else {
            return;
        };

        // Require a foreign nominal or blanket target before retaining the impl.
        let target = cx.tcx.type_of(def_id).instantiate_identity();
        if !target.is_extension_target() {
            return;
        }

        // Retain the authored impl for coherence and physical-placement analysis.
        self.extension_impls.push(ExtensionTraitImpl {
            def_id,
            trait_def_id,
            module,
            span: item.span,
        });
    }

    /// Records one method's size contribution and concrete nonreceiver subject types.
    pub fn record_trait_item(&mut self, cx: &LateContext<'_>, item: &TraitItem<'_>) {
        // Resolve the owning local trait and count only methods.
        if !matches!(item.kind, TraitItemKind::Fn(..)) {
            return;
        }
        let def_id = item.owner_id.def_id;
        let Some(trait_def_id) = cx.tcx.opt_local_parent(def_id) else {
            return;
        };
        let Some(trait_) = self.traits.get_mut(&trait_def_id) else {
            return;
        };
        trait_.method_count += 1;

        // Ignore the receiver and retain concrete nominal co-parameter families.
        let associated = cx.tcx.associated_item(def_id);
        let signature = cx.tcx.fn_sig(def_id).instantiate_identity().skip_binder();
        let skip = usize::from(associated.is_method());
        for parameter in signature.inputs().iter().skip(skip) {
            let Some(subject) = parameter.nominal_def_id() else {
                continue;
            };
            trait_.subjects.insert(subject);
        }
    }

    /// Returns local traits proven to extend a foreign or generic target.
    fn extension_trait_ids(&self) -> HashSet<LocalDefId> {
        self.extension_impls
            .iter()
            .map(|impl_| impl_.trait_def_id)
            .collect()
    }

    /// Returns extension traits that exceed either configured coherence boundary.
    pub fn coherence_findings(
        &self,
        cx: &LateContext<'_>,
        max_methods: usize,
    ) -> Vec<ExtensionTraitFinding> {
        // Convert semantic limit failures into source-ordered diagnostics.
        let extension_traits = self.extension_trait_ids();

        // Build one combined finding for every extension trait that crosses a boundary.
        let mut findings = extension_traits
            .into_iter()
            .filter_map(|def_id| {
                let trait_ = self.traits.get(&def_id)?;
                let mut problems = Vec::new();
                if trait_.subjects.len() > 1 {
                    let mut subjects = trait_
                        .subjects
                        .iter()
                        .map(|subject| cx.tcx.item_name(*subject))
                        .collect::<Vec<_>>();
                    subjects.sort_unstable_by_key(ToString::to_string);
                    problems.push(ExtensionTraitProblem::MultipleSubjects(subjects));
                }
                if trait_.method_count > max_methods {
                    problems.push(ExtensionTraitProblem::TooManyMethods(trait_.method_count));
                }
                (!problems.is_empty()).then_some(ExtensionTraitFinding {
                    span: trait_.span,
                    name: trait_.name,
                    problems,
                })
            })
            .collect::<Vec<_>>();

        // Keep diagnostics deterministic despite hash-based semantic collections.
        findings.sort_unstable_by_key(|finding| finding.span.lo());
        findings
    }

    /// Returns traits whose declaration and extension impls are cross-module or interrupted.
    pub fn placement_findings(&self) -> Vec<ExtensionTraitPlacementFinding> {
        // Restore source order independently inside each authored module.
        let mut items_by_module = HashMap::<LocalDefId, Vec<&ExtensionTraitModuleItem>>::new();
        for item in &self.module_items {
            items_by_module.entry(item.module).or_default().push(item);
        }
        for items in items_by_module.values_mut() {
            items.sort_unstable_by_key(|item| item.span.lo());
        }

        // Compare every extension impl group with the positions following its trait.
        let extension_traits = self.extension_trait_ids();

        // Build one placement finding per interrupted or cross-module trait group.
        let mut findings = extension_traits
            .into_iter()
            .filter_map(|trait_def_id| {
                let trait_ = self.traits.get(&trait_def_id)?;
                let mut impls = self
                    .extension_impls
                    .iter()
                    .filter(|impl_| impl_.trait_def_id == trait_def_id)
                    .collect::<Vec<_>>();
                impls.sort_unstable_by_key(|impl_| impl_.span.lo());

                if let Some(cross_module) = impls.iter().find(|impl_| impl_.module != trait_.module)
                {
                    return Some(ExtensionTraitPlacementFinding {
                        span: cross_module.span,
                        trait_span: trait_.span,
                        name: trait_.name,
                    });
                }

                let module_items = items_by_module.get(&trait_.module)?;
                let trait_index = module_items
                    .iter()
                    .position(|item| item.def_id == trait_def_id)?;
                let misplaced = impls.iter().enumerate().find(|(offset, impl_)| {
                    module_items
                        .get(trait_index + offset + 1)
                        .is_none_or(|item| item.def_id != impl_.def_id)
                })?;
                Some(ExtensionTraitPlacementFinding {
                    span: misplaced.1.span,
                    trait_span: trait_.span,
                    name: trait_.name,
                })
            })
            .collect::<Vec<_>>();

        // Keep placement diagnostics in declaration order.
        findings.sort_unstable_by_key(|finding| finding.trait_span.lo());
        findings
    }
}
