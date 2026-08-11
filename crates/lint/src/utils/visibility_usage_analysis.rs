extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{CRATE_DEF_ID, LocalDefId};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{
    Expr, ExprField, ExprKind, FieldDef, HirId, ImplItem, ImplItemImplKind, Item, ItemKind, Pat,
    PatField, PatKind, Path, QPath, StructTailExpr,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, TyCtxt};
use rustc_session::config::CrateType;
use rustc_span::{Span, Symbol, sym};

use super::source_provenance::is_framework_generated_item;
use super::visibility_boundary::VisibilityBoundary;
use super::visibility_package_policy::VisibilityPackagePolicy;

// -----------------------------------------------------------------------------
// AuthoredVisibility: Source visibility vocabulary
// -----------------------------------------------------------------------------

/// Authored canonical visibility retained independently from rustc's semantic reach.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AuthoredVisibility {
    /// Unrestricted `pub` syntax.
    Public,
    /// Crate-scoped `pub(super)` syntax.
    Crate,
    /// Parent-scoped `pub(super)` syntax.
    Super,
    /// A noncanonical `pub(self)` or `pub(in …)` restriction.
    Noncanonical,
}

impl AuthoredVisibility {
    /// Parses a visibility snippet while leaving inherited visibility outside the candidate set.
    fn from_source(source: &str) -> Option<Self> {
        // Normalize explicit source before classifying the accepted vocabulary.
        let source = source.trim();
        if source.is_empty() {
            return None;
        }

        // Match the four authored forms after empty inherited visibility is removed.
        match source {
            "pub" => Some(Self::Public),
            "pub(crate)" => Some(Self::Crate),
            "pub(super)" => Some(Self::Super),
            source if source.starts_with("pub(") => Some(Self::Noncanonical),
            _ => None,
        }
    }

    /// Returns the semantic boundary of canonical authored syntax.
    const fn boundary(self) -> Option<VisibilityBoundary> {
        match self {
            Self::Public => Some(VisibilityBoundary::Public),
            Self::Crate => Some(VisibilityBoundary::Crate),
            Self::Super => Some(VisibilityBoundary::Super),
            Self::Noncanonical => None,
        }
    }
}

// -----------------------------------------------------------------------------
// Visibility: Resolved local reference
// -----------------------------------------------------------------------------

/// Compilation topology owning one resolved local reference.
#[derive(Clone, Copy, Eq, PartialEq)]
enum VisibilityUseKind {
    /// Reference compiled as part of ordinary production code.
    Production,
    /// Reference owned by a recognized in-source test module.
    Test,
}

impl VisibilityUseKind {
    /// Classifies one owner module while keeping the binary mapping on this enum.
    fn for_module(analyzer: &VisibilityUsageAnalyzer, tcx: TyCtxt<'_>, module: LocalDefId) -> Self {
        if analyzer.module_is_test_owned(tcx, module) {
            Self::Test
        } else {
            Self::Production
        }
    }
}

/// One resolved local use carrying the module and test topology that require visibility.
#[derive(Clone, Copy)]
pub struct VisibilityUse {
    /// Module containing the reference.
    pub(crate) module: LocalDefId,
    /// Authored reference span used for diagnostic labels.
    pub(crate) span: Span,
    /// Whether the reference belongs to a recognized in-source test module.
    kind: VisibilityUseKind,
}

impl VisibilityUse {
    /// Returns whether this reference exists only in recognized in-source tests.
    pub(crate) const fn is_test_only(&self) -> bool {
        matches!(self.kind, VisibilityUseKind::Test)
    }
}

/// Named source identity for a field selected in aggregate syntax.
struct VisibilitySourceField {
    /// Field expression or pattern node resolved by type checking.
    hir_id: HirId,
    /// Authored field name used for diagnostics.
    span: Span,
}

// -----------------------------------------------------------------------------
// VisibilityFinding: Complete boundary evidence
// -----------------------------------------------------------------------------

/// One authored visibility broader than the canonical boundary supported by observed uses.
pub struct VisibilityFindingDeclaration {
    /// Declaration node used for lint-level lookup.
    pub(crate) hir_id: HirId,
    /// Visibility token span receiving the primary diagnostic.
    pub(crate) span: Span,
    /// Declaration name.
    pub(crate) name: Symbol,
    /// Human-readable declaration kind.
    pub(crate) kind: &'static str,
    /// Defining module rendered for remediation context.
    pub(crate) defining_module: String,
}

/// Canonical authored, complete-use, and production-only reach for one declaration.
pub struct VisibilityFindingBoundary {
    /// Authored canonical boundary.
    pub(crate) current: VisibilityBoundary,
    /// Authored boundary capped by the nearest restricted ancestor module.
    pub(crate) effective_current: VisibilityBoundary,
    /// Narrowest canonical boundary supporting every observed use.
    pub(crate) required: VisibilityBoundary,
    /// Narrowest canonical boundary supporting production uses only.
    pub(crate) production_required: VisibilityBoundary,
}

/// One authored visibility broader than the canonical boundary supported by observed uses.
pub struct VisibilityFinding {
    /// Declaration identity and ownership context.
    pub(crate) declaration: VisibilityFindingDeclaration,
    /// Complete and production-only boundary decision.
    pub(crate) boundary: VisibilityFindingBoundary,
    /// Distinct boundary-setting uses in source order.
    pub(crate) uses: Vec<VisibilityUse>,
    /// Whether unrestricted `pub` was analyzed because the package is closed.
    pub(crate) is_closed_package_public: bool,
}

impl VisibilityFinding {
    /// Returns whether tests alone force a broader boundary than production code.
    pub(crate) const fn is_test_constrained(&self) -> bool {
        self.boundary.required as u8 > self.boundary.production_required as u8
    }
}

// -----------------------------------------------------------------------------
// VisibilityCandidate: Authored declaration
// -----------------------------------------------------------------------------

/// Visibility-bearing declaration retained until all local references are known.
struct VisibilityCandidateIdentity {
    /// Definition receiving local references.
    def_id: LocalDefId,
    /// Declaration node used for lint-level lookup.
    hir_id: HirId,
    /// Visibility token span.
    span: Span,
    /// Declaration name.
    name: Symbol,
    /// Human-readable declaration kind.
    kind: &'static str,
}

/// Visibility-bearing declaration retained until all local references are known.
struct VisibilityCandidate {
    /// Source identity used by diagnostics and reference indexing.
    identity: VisibilityCandidateIdentity,
    /// Defining module that owns private reach.
    defining_module: LocalDefId,
    /// Parsed source visibility.
    authored: AuthoredVisibility,
}

// -----------------------------------------------------------------------------
// VisibilityUsageAnalyzer: Crate local reach
// -----------------------------------------------------------------------------

/// Collects authored declarations and resolved references before deriving visibility findings.
#[derive(Default)]
pub struct VisibilityUsageAnalyzer {
    /// Visibility-bearing authored declarations indexed by definition.
    candidates: HashMap<LocalDefId, VisibilityCandidate>,
    /// Resolved local references indexed by their target definition.
    uses: HashMap<LocalDefId, Vec<VisibilityUse>>,
    /// Definitions exposed through another candidate's authored interface.
    interface_dependencies: HashMap<LocalDefId, HashSet<LocalDefId>>,
    /// Types named by trait associated-type bindings whose minimum reach is compiler-enforced.
    trait_interface_types: HashSet<LocalDefId>,
    /// Canonically named test modules discovered in test-harness compilations.
    test_modules: HashSet<LocalDefId>,
}

impl VisibilityUsageAnalyzer {
    /// Records a module-level authored declaration and its direct references.
    pub(crate) fn record_item<'tcx>(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Discover test ownership before classifying references made by descendants.
        if cx.sess().opts.test
            && matches!(item.kind, ItemKind::Mod(..))
            && item
                .kind
                .ident()
                .is_some_and(|ident| matches!(ident.name.as_str(), "test" | "tests"))
        {
            self.test_modules.insert(item.owner_id.def_id);
        }

        // Retain only semantic named declarations with authored visibility.
        if !item.span.from_expansion()
            && !is_framework_generated_item(item)
            && let Some(kind) = Self::item_kind(item)
            && let Some(identifier) = item.kind.ident()
        {
            // Store the declaration independently from references in its body.
            self.record_item_candidate(cx, item, identifier.name, kind);

            // Record interface dependencies independently from executable references.
            self.record_item_interface(cx, item);
        }

        // Resolve references owned directly by this item without descending into child items.
        let module = cx
            .tcx
            .parent_module_from_def_id(item.owner_id.def_id)
            .to_local_def_id();

        // Traverse the item with its exact production or test ownership.
        let use_kind = VisibilityUseKind::for_module(self, cx.tcx, module);
        let mut collector = VisibilityReferenceCollector::new(cx, module, use_kind);
        collector.visit_item(item);
        self.merge_uses(collector.finish());
    }

    /// Records one inherent associated item and its direct references.
    pub(crate) fn record_impl_item<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        item: &'tcx ImplItem<'tcx>,
    ) {
        // Trait implementations contribute uses but inherit visibility rather than declaring it.
        if let ImplItemImplKind::Inherent { vis_span } = item.impl_kind {
            let kind = match item.kind {
                rustc_hir::ImplItemKind::Const(..) => "associated constant",
                rustc_hir::ImplItemKind::Fn(..) => "inherent method",
                rustc_hir::ImplItemKind::Type(..) => "associated type",
            };
            if !item.span.from_expansion() {
                // Store only directly authored inherent item visibility.
                self.record_impl_item_candidate(cx, item, vis_span, kind);

                // Record interface dependencies independently from executable references.
                self.record_impl_item_interface(cx, item);
            }
        } else {
            // Trait items inherit visibility, but their associated-type bindings can expose
            // otherwise local declarations through the implementing type's public contract.
            self.record_impl_item_interface(cx, item);
        }

        // Resolve references from the associated declaration and body.
        let module = cx
            .tcx
            .parent_module_from_def_id(item.owner_id.def_id)
            .to_local_def_id();

        // Traverse trait and inherent bodies because both contribute local uses.
        let use_kind = VisibilityUseKind::for_module(self, cx.tcx, module);
        let mut collector = VisibilityReferenceCollector::new(cx, module, use_kind);
        collector.visit_impl_item(item);
        self.merge_uses(collector.finish());
    }
}

impl VisibilityUsageAnalyzer {
    /// Records one struct or union field with independently authored visibility.
    pub(crate) fn record_field(&mut self, cx: &LateContext<'_>, field: &FieldDef<'_>) {
        // Enum variant fields inherit the enum's visibility and cannot be narrowed independently.
        let parent = cx.tcx.parent(field.def_id.to_def_id());
        if !matches!(cx.tcx.def_kind(parent), DefKind::Struct | DefKind::Union) {
            return;
        }

        // Generated fields do not own an independently editable boundary.
        if field.span.from_expansion() {
            return;
        }

        // Retain the authored field as an independently narrowable candidate.
        self.record_field_candidate(cx, field);

        // Connect field consumers to the aggregate interface that declares its type.
        self.interface_dependencies
            .entry(field.def_id)
            .or_default()
            .insert(parent.expect_local());
    }
}

impl VisibilityUsageAnalyzer {
    /// Returns usage-based findings for canonical authored visibility.
    pub(crate) fn findings(self, cx: &LateContext<'_>) -> Vec<VisibilityFinding> {
        // Propagate every item use through modules that must expose its canonical path.
        let mut uses = self.uses;
        Self::propagate_interface_uses(&self.interface_dependencies, &mut uses);
        Self::propagate_module_uses(cx.tcx, &self.candidates, &mut uses);
        let package = VisibilityPackagePolicy::for_current_package();
        let binary = Self::is_binary_crate(cx);

        // Derive one ordered finding for every canonical visibility that can shrink.
        let mut findings = Vec::new();
        for candidate in self.candidates.into_values() {
            // Trait associated types cannot be narrowed independently from their implementation
            // target; rustc rejects a private binding even when no caller names it directly.
            if self
                .trait_interface_types
                .contains(&candidate.identity.def_id)
            {
                continue;
            }

            // Resolve canonical syntax and references attributed to this declaration.
            let Some(current) = candidate.authored.boundary() else {
                continue;
            };
            let effective_current =
                current.min(Self::enclosing_boundary(cx.tcx, candidate.defining_module));
            let candidate_uses = uses.remove(&candidate.identity.def_id).unwrap_or_default();

            // Derive the complete use boundary before separating production topology.
            let required = VisibilityBoundary::for_uses(
                cx.tcx,
                candidate.defining_module,
                candidate_uses.iter().map(|usage| usage.module),
            );

            // Remove test-owned references to expose the production-only boundary.
            let production_modules = candidate_uses
                .iter()
                .filter(|usage| usage.kind == VisibilityUseKind::Production)
                .map(|usage| usage.module);
            let production_required =
                VisibilityBoundary::for_uses(cx.tcx, candidate.defining_module, production_modules);

            // Preserve downstream APIs only when the package and effective visibility prove them.
            let exported = cx
                .tcx
                .effective_visibilities(())
                .is_exported(candidate.identity.def_id);

            // Preserve only externally reachable APIs in packages that may be published.
            let preserve_public = current == VisibilityBoundary::Public
                && !binary
                && package.preserves_exported_public_items()
                && exported;

            // Retain shrinkable declarations and test-constrained equal boundaries.
            if preserve_public
                || effective_current < required
                || (effective_current == required && required == production_required)
            {
                continue;
            }

            // Resolve ownership text before packaging declaration source identity.
            let defining_module = Self::module_name(cx.tcx, candidate.defining_module);

            // Package definition and source identity independently from boundary calculation.
            let declaration = VisibilityFindingDeclaration {
                hir_id: candidate.identity.hir_id,
                span: candidate.identity.span,
                name: candidate.identity.name,
                kind: candidate.identity.kind,
                defining_module,
            };

            // Keep the complete and production-only reach comparable as one decision.
            let boundary = VisibilityFindingBoundary {
                current,
                effective_current,
                required,
                production_required,
            };

            // Retain complete remediation context in one source-ordered finding.
            findings.push(VisibilityFinding {
                declaration,
                boundary,
                uses: Self::boundary_uses(candidate.defining_module, required, candidate_uses),
                is_closed_package_public: current == VisibilityBoundary::Public
                    && package == VisibilityPackagePolicy::Closed,
            });
        }
        findings.sort_by_key(|finding| finding.declaration.span.lo());
        findings
    }
}

impl VisibilityUsageAnalyzer {
    /// Classifies abstract type declarations governed by visibility narrowing.
    const fn abstract_item_kind(item: &Item<'_>) -> Option<&'static str> {
        match item.kind {
            ItemKind::Trait(..) => Some("trait"),
            ItemKind::TraitAlias(..) => Some("trait alias"),
            ItemKind::TyAlias(..) => Some("type alias"),
            _ => None,
        }
    }

    /// Classifies namespace and nominal type declarations governed by visibility narrowing.
    const fn nominal_item_kind(item: &Item<'_>) -> Option<&'static str> {
        match item.kind {
            ItemKind::Mod(..) => Some("module"),
            ItemKind::Enum(..) => Some("enum"),
            ItemKind::Struct(..) => Some("struct"),
            ItemKind::Union(..) => Some("union"),
            _ => None,
        }
    }

    /// Classifies runtime value declarations governed by visibility narrowing.
    const fn value_item_kind(item: &Item<'_>) -> Option<&'static str> {
        match item.kind {
            ItemKind::Const(..) => Some("constant"),
            ItemKind::Fn { .. } => Some("function"),
            ItemKind::Static(..) => Some("static"),
            _ => None,
        }
    }

    /// Classifies module-level declaration kinds governed by visibility narrowing.
    fn item_kind(item: &Item<'_>) -> Option<&'static str> {
        Self::value_item_kind(item)
            .or_else(|| Self::nominal_item_kind(item))
            .or_else(|| Self::abstract_item_kind(item))
    }

    /// Records one candidate after parsing and validating its authored visibility.
    fn record_candidate(
        &mut self,
        cx: &LateContext<'_>,
        def_id: LocalDefId,
        hir_id: HirId,
        vis_span: Span,
        name: Symbol,
        kind: &'static str,
    ) {
        // Generated declarations and inherited private visibility are outside this rule.
        if vis_span.from_expansion() {
            return;
        }
        let Ok(source) = cx.sess().source_map().span_to_snippet(vis_span) else {
            return;
        };
        let Some(authored) = AuthoredVisibility::from_source(&source) else {
            return;
        };

        // Compiler and foreign-linkage entry points own contracts outside Rust path visibility.
        let attributes = cx.tcx.hir_attrs(hir_id);
        let has_external_attribute = attributes.iter().any(|attribute| {
            attribute.has_name(sym::no_mangle)
                || attribute.has_name(sym::export_name)
                || attribute.has_name(sym::proc_macro)
                || attribute.has_name(sym::proc_macro_attribute)
                || attribute.has_name(sym::proc_macro_derive)
                || attribute.has_name(sym::lang)
        });

        // Include unsafe-wrapped symbol attributes through rustc's codegen contract.
        let has_external_codegen_contract =
            matches!(
                cx.tcx.def_kind(def_id),
                DefKind::Fn | DefKind::AssocFn | DefKind::Static { .. }
            ) && cx.tcx.codegen_fn_attrs(def_id).contains_extern_indicator();
        if has_external_attribute || has_external_codegen_contract {
            return;
        }

        // Preserve the definition and module facts needed after all references are collected.
        let defining_module = cx.tcx.parent_module_from_def_id(def_id).to_local_def_id();

        // Retain the authored source identity independently from semantic ownership.
        let identity = VisibilityCandidateIdentity {
            def_id,
            hir_id,
            span: vis_span,
            name,
            kind,
        };

        // Pair source identity with the semantic module and authored boundary.
        let candidate = VisibilityCandidate {
            identity,
            defining_module,
            authored,
        };

        // Index the complete declaration by the definition references will resolve to.
        self.candidates.insert(def_id, candidate);
    }

    /// Adapts one module item to the shared candidate representation.
    fn record_item_candidate(
        &mut self,
        cx: &LateContext<'_>,
        item: &Item<'_>,
        name: Symbol,
        kind: &'static str,
    ) {
        let def_id = item.owner_id.def_id;
        let hir_id = item.hir_id();
        self.record_candidate(cx, def_id, hir_id, item.vis_span, name, kind);
    }

    /// Adapts one inherent associated item to the shared candidate representation.
    fn record_impl_item_candidate(
        &mut self,
        cx: &LateContext<'_>,
        item: &ImplItem<'_>,
        vis_span: Span,
        kind: &'static str,
    ) {
        let def_id = item.owner_id.def_id;
        let hir_id = item.hir_id();
        self.record_candidate(cx, def_id, hir_id, vis_span, item.ident.name, kind);
    }

    /// Adapts one struct or union field to the shared candidate representation.
    fn record_field_candidate(&mut self, cx: &LateContext<'_>, field: &FieldDef<'_>) {
        let def_id = field.def_id;
        let hir_id = field.hir_id;
        let span = field.vis_span;
        let name = field.ident.name;
        self.record_candidate(cx, def_id, hir_id, span, name, "field");
    }

    /// Merges references collected from one declaration owner.
    fn merge_uses(&mut self, collected: HashMap<LocalDefId, Vec<VisibilityUse>>) {
        for (definition, uses) in collected {
            self.uses.entry(definition).or_default().extend(uses);
        }
    }

    /// Records types named by a module item's public interface without descending into its body.
    fn record_item_interface<'tcx>(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        let mut collector = VisibilityReferenceCollector::for_interface(cx);
        collector.visit_item(item);
        self.interface_dependencies
            .insert(item.owner_id.def_id, collector.definitions());
    }

    /// Records types named by an inherent associated item's authored interface.
    fn record_impl_item_interface<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        item: &'tcx ImplItem<'tcx>,
    ) {
        let mut collector = VisibilityReferenceCollector::for_interface(cx);
        collector.visit_impl_item(item);
        if let rustc_hir::ImplItemKind::Type(assigned) = item.kind
            && let Some(assigned) = assigned.try_as_ambig_ty()
        {
            collector.visit_ty(assigned);
        }
        let mut definitions = collector.definitions();

        // Inherent items expose their signatures through their own authored visibility.
        if matches!(item.impl_kind, ImplItemImplKind::Inherent { .. }) {
            self.interface_dependencies
                .insert(item.owner_id.def_id, definitions);
            return;
        }

        // HIR traversal does not expose the normalized right-hand side of every trait
        // associated-type binding, so supplement it from the semantic assigned type.
        if matches!(item.kind, rustc_hir::ImplItemKind::Type(..)) {
            let assigned = cx.tcx.type_of(item.owner_id).instantiate_identity();
            for component in assigned.walk() {
                let Some(component) = component.as_type() else {
                    continue;
                };
                let ty::Adt(definition, _) = component.kind() else {
                    continue;
                };
                if let Some(definition) = definition.did().as_local() {
                    definitions.insert(definition);
                }
            }
        }
        self.trait_interface_types
            .extend(definitions.iter().copied());

        // Trait associated types are exposed wherever the concrete implementing type is usable.
        // Rust rejects narrowing such a type below the target even when no consumer names the
        // associated type directly, so attach the dependency to that local target declaration.
        let parent = cx.tcx.local_parent(item.owner_id.def_id);
        let self_type = cx.tcx.type_of(parent).instantiate_identity();
        let ty::Adt(definition, _) = self_type.kind() else {
            return;
        };
        let Some(target) = definition.did().as_local() else {
            return;
        };
        self.interface_dependencies
            .entry(target)
            .or_default()
            .extend(definitions);
    }

    /// Returns whether a module lies within a canonical in-source test module.
    fn module_is_test_owned(&self, tcx: TyCtxt<'_>, module: LocalDefId) -> bool {
        let mut cursor = Some(module);
        while let Some(candidate) = cursor {
            if self.test_modules.contains(&candidate) {
                return true;
            }
            cursor = tcx
                .opt_local_parent(candidate)
                .filter(|parent| tcx.def_kind(*parent) == DefKind::Mod);
        }
        false
    }
}

impl VisibilityUsageAnalyzer {
    /// Returns the broadest effective reach permitted by restricted ancestor modules.
    fn enclosing_boundary(tcx: TyCtxt<'_>, defining_module: LocalDefId) -> VisibilityBoundary {
        let mut boundary = VisibilityBoundary::Public;
        let mut module = defining_module;
        while module != CRATE_DEF_ID {
            if let ty::Visibility::Restricted(scope) = tcx.visibility(module) {
                let scope = scope.expect_local();
                let restriction = VisibilityBoundary::for_uses(tcx, defining_module, [scope]);
                boundary = boundary.min(restriction);
            }
            module = tcx.local_parent(module);
        }
        boundary
    }

    /// Propagates item references to every authored module on the defining path.
    fn propagate_module_uses(
        tcx: TyCtxt<'_>,
        candidates: &HashMap<LocalDefId, VisibilityCandidate>,
        uses: &mut HashMap<LocalDefId, Vec<VisibilityUse>>,
    ) {
        let direct = uses.clone();
        for (definition, definition_uses) in direct {
            let mut module = tcx.parent_module_from_def_id(definition).to_local_def_id();
            loop {
                // Attribute the item reference to an authored module on its definition path.
                Self::extend_module_uses(candidates, uses, module, &definition_uses);

                // Advance toward the crate root until no parent module remains.
                let Some(parent) = tcx
                    .opt_local_parent(module)
                    .filter(|parent| tcx.def_kind(*parent) == DefKind::Mod)
                else {
                    break;
                };
                module = parent;
            }
        }
    }

    /// Propagates consumer reach through types exposed by candidate interfaces.
    fn propagate_interface_uses(
        dependencies: &HashMap<LocalDefId, HashSet<LocalDefId>>,
        uses: &mut HashMap<LocalDefId, Vec<VisibilityUse>>,
    ) {
        loop {
            let before = uses.values().map(Vec::len).sum::<usize>();
            for (owner, exposed) in dependencies {
                let owner_uses = uses.get(owner).cloned().unwrap_or_default();
                Self::extend_exposed_definitions(uses, exposed, &owner_uses);
            }
            if uses.values().map(Vec::len).sum::<usize>() != before {
                continue;
            }
            break;
        }
    }

    /// Extends every definition exposed by one owner interface.
    fn extend_exposed_definitions(
        uses: &mut HashMap<LocalDefId, Vec<VisibilityUse>>,
        exposed: &HashSet<LocalDefId>,
        owner_uses: &[VisibilityUse],
    ) {
        for definition in exposed {
            Self::extend_interface_uses(uses, *definition, owner_uses);
        }
    }

    /// Adds consumer reach to one type exposed by an interface without duplicating evidence.
    fn extend_interface_uses(
        uses: &mut HashMap<LocalDefId, Vec<VisibilityUse>>,
        definition: LocalDefId,
        owner_uses: &[VisibilityUse],
    ) {
        let definition_uses = uses.entry(definition).or_default();
        for usage in owner_uses {
            let is_known = definition_uses.iter().any(|existing| {
                existing.module == usage.module
                    && existing.span == usage.span
                    && existing.kind == usage.kind
            });
            if is_known {
                continue;
            }
            definition_uses.push(*usage);
        }
    }

    /// Extends one module candidate with references to an item defined beneath it.
    fn extend_module_uses(
        candidates: &HashMap<LocalDefId, VisibilityCandidate>,
        uses: &mut HashMap<LocalDefId, Vec<VisibilityUse>>,
        module: LocalDefId,
        definition_uses: &[VisibilityUse],
    ) {
        if candidates
            .get(&module)
            .is_none_or(|candidate| candidate.identity.kind != "module")
        {
            return;
        }
        uses.entry(module)
            .or_default()
            .extend(definition_uses.iter().copied());
    }

    /// Retains distinct uses that actually establish the selected canonical boundary.
    fn boundary_uses(
        defining_module: LocalDefId,
        required: VisibilityBoundary,
        mut uses: Vec<VisibilityUse>,
    ) -> Vec<VisibilityUse> {
        uses.sort_by_key(|usage| usage.span.lo());
        uses.dedup_by_key(|usage| usage.module);
        uses.retain(|usage| match required {
            VisibilityBoundary::Private => false,
            VisibilityBoundary::Super => usage.module != defining_module,
            VisibilityBoundary::Crate | VisibilityBoundary::Public => true,
        });
        uses
    }
}

impl VisibilityUsageAnalyzer {
    /// Renders the crate root explicitly instead of exposing rustc's empty root path.
    pub(crate) fn module_name(tcx: TyCtxt<'_>, module: LocalDefId) -> String {
        let name = tcx.def_path_str(module.to_def_id());
        if name.is_empty() {
            "crate".to_owned()
        } else {
            name
        }
    }

    /// Returns whether the current compiler target is an ordinary executable crate.
    fn is_binary_crate(cx: &LateContext<'_>) -> bool {
        !cx.sess().opts.test
            && cx
                .sess()
                .opts
                .crate_types
                .iter()
                .all(|crate_type| *crate_type == CrateType::Executable)
    }
}

// -----------------------------------------------------------------------------
// VisibilityReference: Definition use resolution
// -----------------------------------------------------------------------------

/// Body ownership and topology mode for one reference traversal.
struct VisibilityReferenceTraversal {
    /// Whether this owner belongs exclusively to in-source tests.
    use_kind: VisibilityUseKind,
    /// Body owner whose type-checking tables resolve method and field access.
    body_owner: Option<LocalDefId>,
    /// Whether traversal should enter executable bodies instead of retaining only interfaces.
    has_body_traversal: bool,
}

/// HIR visitor that resolves local paths and type-dependent member access for one owner.
struct VisibilityReferenceCollector<'analysis, 'tcx> {
    /// Compiler context used for type-dependent resolution.
    cx: &'analysis LateContext<'tcx>,
    /// Module containing every reference collected by this owner traversal.
    module: LocalDefId,
    /// Traversal mode and optional type-checking owner.
    traversal: VisibilityReferenceTraversal,
    /// Collected local references indexed by definition.
    uses: HashMap<LocalDefId, Vec<VisibilityUse>>,
}

impl<'analysis, 'tcx> VisibilityReferenceCollector<'analysis, 'tcx> {
    /// Starts reference collection for one declaration owner.
    fn new(
        cx: &'analysis LateContext<'tcx>,
        module: LocalDefId,
        use_kind: VisibilityUseKind,
    ) -> Self {
        // Select executable traversal before packaging collector ownership.
        let traversal = VisibilityReferenceTraversal {
            use_kind,
            body_owner: None,
            has_body_traversal: true,
        };

        // Initialize an executable traversal with its complete owner topology.
        Self {
            cx,
            module,
            traversal,
            uses: HashMap::new(),
        }
    }

    /// Starts an interface-only traversal whose synthetic topology is never emitted as a use.
    fn for_interface(cx: &'analysis LateContext<'tcx>) -> Self {
        // Select interface traversal before packaging synthetic collector ownership.
        let traversal = VisibilityReferenceTraversal {
            use_kind: VisibilityUseKind::Production,
            body_owner: None,
            has_body_traversal: false,
        };

        // Initialize a body-free traversal with synthetic topology used only during collection.
        Self {
            cx,
            module: CRATE_DEF_ID,
            traversal,
            uses: HashMap::new(),
        }
    }

    /// Returns the local definitions named by this interface traversal.
    fn definitions(self) -> HashSet<LocalDefId> {
        self.uses.into_keys().collect()
    }

    /// Returns all local references accumulated from the owner.
    fn finish(self) -> HashMap<LocalDefId, Vec<VisibilityUse>> {
        self.uses
    }

    /// Adds one resolved definition use with the collector's complete owner topology.
    fn push_use(&mut self, definition: LocalDefId, span: Span) {
        // Retain owner module, source location, and compilation topology together.
        let usage = VisibilityUse {
            module: self.module,
            span,
            kind: self.traversal.use_kind,
        };
        self.uses.entry(definition).or_default().push(usage);
    }

    /// Records one local resolution after normalizing constructor definitions.
    fn record_resolution(&mut self, resolution: Res, span: Span) {
        let Some(mut definition) = resolution
            .opt_def_id()
            .and_then(rustc_hir::def_id::DefId::as_local)
        else {
            return;
        };
        if matches!(self.cx.tcx.def_kind(definition), DefKind::Ctor(..)) {
            definition = self.cx.tcx.local_parent(definition);
        }
        self.push_use(definition, span);
    }

    /// Resolves the local definition selected by one typed method-call expression.
    fn method_definition(&self, expression: &Expr<'_>) -> Option<LocalDefId> {
        if !matches!(expression.kind, ExprKind::MethodCall(..)) {
            return None;
        }
        let owner = self.traversal.body_owner?;
        self.cx
            .tcx
            .typeck(owner)
            .type_dependent_def_id(expression.hir_id)
            .and_then(rustc_hir::def_id::DefId::as_local)
    }

    /// Records a field definition selected through typed member access.
    fn record_field_access(&mut self, expression: &'tcx Expr<'tcx>, base: &'tcx Expr<'tcx>) {
        // Resolve the selected field index and adjusted aggregate base type.
        let Some(owner) = self.traversal.body_owner else {
            return;
        };
        let typeck = self.cx.tcx.typeck(owner);
        let index = typeck.field_index(expression.hir_id);
        let base_type = typeck.expr_ty_adjusted(base).peel_refs();
        let ty::Adt(definition, _) = base_type.kind() else {
            return;
        };

        // Convert the typed field identity into a local definition use.
        let field = definition.non_enum_variant().fields[index].did;
        let Some(field) = field.as_local() else {
            return;
        };
        self.push_use(field, expression.span);
    }

    /// Records named fields selected by a struct expression or pattern.
    fn record_aggregate_fields(
        &mut self,
        path: &QPath<'tcx>,
        hir_id: HirId,
        fields: impl IntoIterator<Item = VisibilitySourceField>,
    ) {
        // Resolve the aggregate definition selected by the expression or pattern path.
        let Some(owner) = self.traversal.body_owner else {
            return;
        };
        let Some(mut definition) = self.cx.qpath_res(path, hir_id).opt_def_id() else {
            return;
        };
        if matches!(self.cx.tcx.def_kind(definition), DefKind::Ctor(..)) {
            definition = self.cx.tcx.parent(definition);
        }
        if matches!(self.cx.tcx.def_kind(definition), DefKind::Variant) {
            definition = self.cx.tcx.parent(definition);
        }

        // Restrict independently visible fields to local non-enum aggregates.
        let aggregate_type = self
            .cx
            .tcx
            .type_of(definition)
            .instantiate_identity()
            .kind();

        // Continue only when semantic type resolution produced an aggregate definition.
        let ty::Adt(adt, _) = aggregate_type else {
            return;
        };
        if adt.is_enum() {
            return;
        }

        // Map every authored field node through the body's type-checking table.
        let typeck = self.cx.tcx.typeck(owner);
        for field_source in fields {
            let Some(index) = typeck.opt_field_index(field_source.hir_id) else {
                continue;
            };
            let field = adt.non_enum_variant().fields[index].did;
            let Some(field) = field.as_local() else {
                continue;
            };
            self.push_use(field, field_source.span);
        }
    }

    /// Records every omitted field whose visibility is exercised by aggregate update syntax.
    fn record_aggregate_rest_fields(&mut self, path: &QPath<'tcx>, hir_id: HirId, span: Span) {
        // Resolve and normalize the aggregate selected by the update expression.
        let Some(mut definition) = self.cx.qpath_res(path, hir_id).opt_def_id() else {
            return;
        };
        if matches!(
            self.cx.tcx.def_kind(definition),
            DefKind::Ctor(..) | DefKind::Variant
        ) {
            definition = self.cx.tcx.parent(definition);
        }

        // Record every local non-enum field required by the struct update operation.
        let aggregate_type = self.cx.tcx.type_of(definition).instantiate_identity();
        let ty::Adt(adt, _) = aggregate_type.kind() else {
            return;
        };
        if adt.is_enum() {
            return;
        }
        for field in &adt.non_enum_variant().fields {
            let Some(field) = field.did.as_local() else {
                continue;
            };
            self.push_use(field, span);
        }
    }
}

impl<'tcx> Visitor<'tcx> for VisibilityReferenceCollector<'_, 'tcx> {
    fn visit_qpath(&mut self, path: &'tcx QPath<'tcx>, hir_id: HirId, _: Span) {
        // Resolve type-relative associated item paths, including function values not immediately called.
        if let QPath::TypeRelative(_, segment) = path {
            self.record_resolution(self.cx.qpath_res(path, hir_id), segment.ident.span);
        }
        intravisit::walk_qpath(self, path, hir_id);
    }

    fn visit_path(&mut self, path: &Path<'tcx>, _: HirId) {
        // Record final and intermediate segment resolutions so module paths retain their reach.
        self.record_resolution(path.res, path.span);
        for segment in path.segments {
            self.record_resolution(segment.res, segment.ident.span);
        }
        intravisit::walk_path(self, path);
    }

    fn visit_nested_body(&mut self, body_id: rustc_hir::BodyId) {
        if !self.traversal.has_body_traversal {
            return;
        }
        let previous = self
            .traversal
            .body_owner
            .replace(self.cx.tcx.hir_body_owner_def_id(body_id));
        self.visit_body(self.cx.tcx.hir_body(body_id));
        self.traversal.body_owner = previous;
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // Resolve associated function calls through their type-dependent path identity.
        if let ExprKind::Call(callee, _) = expression.kind
            && let ExprKind::Path(path) = callee.kind
        {
            self.record_resolution(self.cx.qpath_res(&path, callee.hir_id), callee.span);
        }

        // Resolve method calls and direct field access from the active body's type tables.
        if let Some(definition) = self.method_definition(expression) {
            self.push_use(definition, expression.span);
        }

        // Resolve direct member access independently from aggregate construction.
        if let ExprKind::Field(base, _) = expression.kind {
            self.record_field_access(expression, base);
        }

        // Resolve named fields participating in aggregate construction.
        if let ExprKind::Struct(path, fields, base) = expression.kind {
            // Preserve authored nodes so diagnostics label selected aggregate fields.
            let fields = fields
                .iter()
                .map(|field: &ExprField<'_>| VisibilitySourceField {
                    hir_id: field.hir_id,
                    span: field.ident.span,
                });
            self.record_aggregate_fields(path, expression.hir_id, fields);

            // Resolve every implicit field when aggregate rest syntax is present.
            let rest_span = match base {
                StructTailExpr::Base(base) => Some(base.span),
                StructTailExpr::DefaultFields(span) => Some(span),
                StructTailExpr::None | StructTailExpr::NoneWithError(_) => None,
            };
            if let Some(span) = rest_span {
                self.record_aggregate_rest_fields(path, expression.hir_id, span);
            }
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_pat(&mut self, pattern: &'tcx Pat<'tcx>) {
        if let PatKind::Struct(path, fields, _) = pattern.kind {
            // Preserve authored nodes so diagnostics label destructured aggregate fields.
            let fields = fields
                .iter()
                .map(|field: &PatField<'_>| VisibilitySourceField {
                    hir_id: field.hir_id,
                    span: field.ident.span,
                });
            self.record_aggregate_fields(&path, pattern.hir_id, fields);
        }
        intravisit::walk_pat(self, pattern);
    }
}
