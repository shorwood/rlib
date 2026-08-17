extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def::DefKind;
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::{
    AmbigArg, GenericBound, ImplPolarity, IsAuto, Item, ItemKind, PolyTraitRef, RestrictionKind,
    Ty, TyKind,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, TypeVisitableExt};
use rustc_session::config::CrateType;
use rustc_span::{Span, Symbol};

use super::visibility_package_policy::VisibilityPackagePolicy;

/// Returns whether the compiler is building an ordinary executable target.
fn is_binary_crate(cx: &LateContext<'_>) -> bool {
    !cx.sess().opts.test
        && cx
            .sess()
            .opts
            .crate_types
            .iter()
            .all(|kind| *kind == CrateType::Executable)
}

// -----------------------------------------------------------------------------
// SingleImplementationTrait: Semantic evidence
// -----------------------------------------------------------------------------

/// An authored trait which can be evaluated after all active items have been visited.
struct SingleImplementationTraitCandidate {
    /// Local definition used to join declarations, implementations, and consumers.
    def_id: LocalDefId,
    /// HIR node used to respect the declaration's lint level.
    hir_id: rustc_hir::HirId,
    /// Authored trait name span.
    span: Span,
    /// Authored trait name rendered in diagnostics.
    name: Symbol,
    /// Whether syntax or a conventional private supertrait prevents downstream implementations.
    is_sealed: bool,
}

/// One concrete local implementation retained as diagnostic evidence.
pub struct SingleImplementationTraitImplementation {
    /// Authored implementation target span.
    pub(crate) span: Span,
    /// Resolved implementation target rendered in guidance.
    pub(crate) target: String,
}

/// Declaration identity and source context retained for diagnostics and lint precedence.
pub struct SingleImplementationTraitDeclaration {
    /// Definition identity used to coordinate precedence with visibility analysis.
    pub(crate) def_id: LocalDefId,
    /// Declaration node used to respect its local lint level.
    pub(crate) hir_id: rustc_hir::HirId,
    /// Authored trait name span receiving the primary diagnostic.
    pub(crate) span: Span,
    /// Authored trait name rendered in guidance.
    pub(crate) name: Symbol,
}

/// Complete evidence for a local trait with one implementation and no polymorphic consumer.
pub struct SingleImplementationTraitFinding {
    /// Trait declaration identity and authored source context.
    pub(crate) declaration: SingleImplementationTraitDeclaration,
    /// Sole concrete local implementation.
    pub(crate) implementation: SingleImplementationTraitImplementation,
    /// Whether a public trait was analyzed under an explicitly closed package policy.
    pub(crate) is_closed_package_public: bool,
}

/// Crate-wide collector for trait declarations, implementations, and abstract type boundaries.
#[derive(Default)]
pub struct SingleImplementationTraitAnalyzer {
    /// Eligible authored trait declarations indexed by definition.
    traits: HashMap<LocalDefId, SingleImplementationTraitCandidate>,
    /// Concrete local implementations indexed by their local trait.
    implementations: HashMap<LocalDefId, Vec<SingleImplementationTraitImplementation>>,
    /// Traits with blanket, generic, or foreign-target implementation evidence.
    ineligible_implementations: HashSet<LocalDefId>,
    /// Traits observed across an active polymorphic type boundary.
    consumers: HashSet<LocalDefId>,
}

impl SingleImplementationTraitAnalyzer {
    /// Resolves an associated item to its local trait container.
    fn projection_trait(cx: &LateContext<'_>, associated_item: DefId) -> Option<LocalDefId> {
        // Only associated types can establish an associated-type projection consumer.
        if !matches!(cx.tcx.def_kind(associated_item), DefKind::AssocTy) {
            return None;
        }
        let trait_def_id = cx.tcx.parent(associated_item);
        matches!(cx.tcx.def_kind(trait_def_id), DefKind::Trait)
            .then(|| trait_def_id.as_local())
            .flatten()
    }

    /// Records a generic bound, `impl Trait`, trait object, supertrait, or trait alias boundary.
    pub(crate) fn record_poly_trait_ref(
        &mut self,
        _cx: &LateContext<'_>,
        trait_ref: &PolyTraitRef<'_>,
    ) {
        // Resolve only local traits because foreign abstractions are never candidates.
        let Some(def_id) = trait_ref.trait_ref.trait_def_id().and_then(DefId::as_local) else {
            return;
        };
        self.consumers.insert(def_id);
    }

    /// Records an associated-type projection such as `<T as Trait>::Item`.
    pub(crate) fn record_ty(&mut self, cx: &LateContext<'_>, ty: &Ty<'_, AmbigArg>) {
        // Resolve only authored type paths with an associated item definition.
        let TyKind::Path(qpath) = ty.kind else {
            return;
        };

        // Unresolved associated paths cannot identify a consumed local trait projection.
        let Some(associated_item) = cx.qpath_res(&qpath, ty.hir_id).opt_def_id() else {
            return;
        };

        // Associated items outside local traits cannot consume a local candidate abstraction.
        let Some(local_trait) = Self::projection_trait(cx, associated_item) else {
            return;
        };
        self.consumers.insert(local_trait);
    }

    /// Derives source-ordered findings after every active item and type has been observed.
    pub(crate) fn findings(self, cx: &LateContext<'_>) -> Vec<SingleImplementationTraitFinding> {
        // Establish the external-consumer policy once for every declaration in this compilation.
        let package = VisibilityPackagePolicy::for_current_package();
        let binary = is_binary_crate(cx);

        // Move implementation storage locally so evidence can be consumed exactly once.
        let mut implementations = self.implementations;
        let mut findings = Vec::new();

        // Evaluate each declaration against complete implementation and consumer evidence.
        for candidate in self.traits.into_values() {
            // Reject any structural reason the trait is intentionally abstract or open-ended.
            if candidate.is_sealed
                || self.consumers.contains(&candidate.def_id)
                || self.ineligible_implementations.contains(&candidate.def_id)
            {
                continue;
            }

            // Require exactly one eligible implementation after considering every active block.
            let Some(mut trait_implementations) = implementations.remove(&candidate.def_id) else {
                continue;
            };
            if trait_implementations.len() != 1 {
                continue;
            }

            // Public APIs in publishable libraries can have unobservable downstream consumers and
            // implementations. Binaries and explicitly closed packages remain locally analyzable.
            let exported = cx
                .tcx
                .effective_visibilities(())
                .is_exported(candidate.def_id);
            if !binary && package.preserves_exported_public_items() && exported {
                continue;
            }

            // Package the sole implementation with the declaration for an evidence-rich finding.
            let implementation = trait_implementations.pop().expect("one implementation");

            // Preserve declaration evidence independently from its sole implementation.
            let declaration = SingleImplementationTraitDeclaration {
                def_id: candidate.def_id,
                hir_id: candidate.hir_id,
                span: candidate.span,
                name: candidate.name,
            };

            // Join both evidence records with the package-policy decision.
            let finding = SingleImplementationTraitFinding {
                declaration,
                implementation,
                is_closed_package_public: package == VisibilityPackagePolicy::Closed && exported,
            };

            // Retain the complete decision for stable source ordering below.
            findings.push(finding);
        }

        // Restore stable authored order independently from hash-map iteration.
        findings.sort_by_key(|finding| finding.declaration.span.lo());
        findings
    }

    /// Retains one editable, behavior-bearing, ordinary, non-generic trait declaration.
    fn record_trait_declaration(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Destructure only trait declarations routed here by the item classifier.
        let ItemKind::Trait(_, is_auto, safety, restriction, ident, generics, bounds, items) =
            item.kind
        else {
            return;
        };

        // Reject syntax and protocols that already establish intentional abstraction semantics.
        if item.span.from_expansion()
            || is_auto == IsAuto::Yes
            || safety.is_unsafe()
            || !generics.params.is_empty()
            || items.is_empty()
        {
            return;
        }

        // A public trait with an unexported local supertrait is structurally sealed downstream.
        let is_exported = cx
            .tcx
            .effective_visibilities(())
            .is_exported(item.owner_id.def_id);
        let has_private_supertrait = bounds.iter().any(|bound| {
            // Non-trait bounds cannot structurally seal a trait through visibility.
            let GenericBound::Trait(poly_trait) = bound else {
                return false;
            };
            poly_trait
                .trait_ref
                .trait_def_id()
                .and_then(DefId::as_local)
                .is_some_and(|id| !cx.tcx.effective_visibilities(()).is_exported(id))
        });

        // Preserve declaration identity and structural sealing intent until crate-post analysis.
        let is_sealed = matches!(restriction.kind, RestrictionKind::Restricted(_))
            || (is_exported && has_private_supertrait);

        // Retain the source identity only after every eligibility decision is complete.
        let candidate = SingleImplementationTraitCandidate {
            def_id: item.owner_id.def_id,
            hir_id: item.hir_id(),
            span: ident.span,
            name: ident.name,
            is_sealed,
        };

        // Index the declaration by the identity shared with trait references and impl blocks.
        self.traits.insert(item.owner_id.def_id, candidate);
    }

    /// Classifies one local trait implementation and retains concrete local evidence.
    fn record_implementation(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Resolve the local trait implemented by this block before inspecting its target.
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };

        // Ignore inherent blocks before resolving the implemented trait identity.
        let Some(trait_ref) = implementation.of_trait else {
            return;
        };

        // Foreign traits cannot match a candidate declared by this compilation.
        let Some(local_trait) = trait_ref.trait_ref.trait_def_id().and_then(DefId::as_local) else {
            return;
        };

        // Negative impls constrain extensibility but are not concrete implementations.
        if !matches!(trait_ref.polarity, ImplPolarity::Positive) {
            self.ineligible_implementations.insert(local_trait);
            return;
        }

        // Resolve aliases before deciding whether the target is one concrete local nominal type.
        let target = cx.tcx.type_of(item.owner_id).instantiate_identity();
        let is_concrete_local_target = matches!(target.kind(), ty::Adt(definition, arguments)
            if definition.did().is_local() && !arguments.has_param());

        // Generic, blanket, or foreign-target impls prove the trait is not single-concrete-use.
        if !implementation.generics.params.is_empty()
            || target.has_param()
            || !is_concrete_local_target
        {
            self.ineligible_implementations.insert(local_trait);
            return;
        }

        // Retain the complete concrete target as implementation evidence and diagnostic context.
        self.implementations.entry(local_trait).or_default().push(
            SingleImplementationTraitImplementation {
                span: implementation.self_ty.span,
                target: target.to_string(),
            },
        );
    }

    /// Records an eligible declaration or one local implementation.
    pub(crate) fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        match item.kind {
            ItemKind::Trait(..) => self.record_trait_declaration(cx, item),
            ItemKind::Impl(..) => self.record_implementation(cx, item),
            _ => {}
        }
    }
}
