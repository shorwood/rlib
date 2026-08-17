extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::HashSet;

use rustc_errors::DiagDecorator;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Violation: Bidirectional sibling dependency diagnostic
// -----------------------------------------------------------------------------

/// Pair of sibling modules connected by imports in both directions.
struct Violation {
    /// Import that completes the dependency cycle.
    span: Span,
    /// Previously authored import establishing the reverse direction.
    reverse_span: Span,
    /// Fully qualified source module path.
    source_module: String,
    /// Fully qualified target module path.
    target_module: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "modules `{}` and `{}` depend on each other in both directions",
            self.source_module, self.target_module
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "neither `{}` nor `{}` has a clear ownership direction, so changes to either module can require understanding both",
            self.source_module, self.target_module
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "extract the shared concepts or choose one dependency direction between `{}` and `{}`",
            self.source_module, self.target_module
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            BIDIRECTIONAL_MODULE_DEPENDENCIES,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.reverse_span,
                    "the reverse dependency is introduced here",
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// Module: Dependency graph records
// -----------------------------------------------------------------------------
/// One resolved import from a module to one of its siblings.
#[derive(Clone, Copy)]
struct ModuleDependency {
    /// Sibling module containing the import.
    source: LocalDefId,
    /// Sibling module reached by the import.
    target: LocalDefId,
    /// Import span used to relate both directions of a cycle.
    span: Span,
}
/// One directed module pair that has already produced a diagnostic.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct ModulePair {
    /// Origin of the directed dependency.
    source: LocalDefId,
    /// Destination of the directed dependency.
    target: LocalDefId,
}

impl ModulePair {
    /// Constructs one directed pair for cycle deduplication.
    const fn new(source: LocalDefId, target: LocalDefId) -> Self {
        Self { source, target }
    }
}

// -----------------------------------------------------------------------------
// BidirectionalModuleDependencies: Module dependency direction policy
// -----------------------------------------------------------------------------
/// Collects sibling imports and reports each two-way module pair once.
#[derive(Default)]
struct BidirectionalModuleDependencies {
    /// Sibling dependency edges collected from resolved imports.
    dependencies: Vec<ModuleDependency>,
    /// Directed pairs for cycles that have already emitted one diagnostic.
    reported: HashSet<ModulePair>,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub BIDIRECTIONAL_MODULE_DEPENDENCIES,
    Warn,
    "rejects imports that create dependency cycles between sibling modules",
    BidirectionalModuleDependencies::default()
}

impl LateLintPass<'_> for BidirectionalModuleDependencies {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Abort early if this item is not an import, because only imports create dependencies.
        let ItemKind::Use(path, _) = item.kind else {
            return;
        };

        // Macro-generated references do not represent authored module dependencies.
        if item.span.in_external_macro(cx.sess().source_map()) {
            return;
        }

        // Iterate over all resolved namespaces for this import and record the first relevant
        // dependency.
        let source = cx.tcx.parent_module(item.hir_id()).to_local_def_id();
        for resolution in [path.res.type_ns, path.res.value_ns, path.res.macro_ns]
            .into_iter()
            .flatten()
        {
            // Resolve one imported namespace to a local definition.
            let Some(target_definition) = resolution.opt_def_id().and_then(DefId::as_local) else {
                continue;
            };

            // Normalize imported items to the module that owns them.
            let target = if matches!(resolution, Res::Def(DefKind::Mod, _)) {
                target_definition
            } else {
                cx.tcx
                    .parent_module_from_def_id(target_definition)
                    .to_local_def_id()
            };

            // Retain only dependencies between distinct modules under one parent.
            let source_parent = cx.tcx.parent_module_from_def_id(source).to_local_def_id();
            let target_parent = cx.tcx.parent_module_from_def_id(target).to_local_def_id();
            if source == target
                || source == target_parent
                || target == source_parent
                || source_parent != target_parent
            {
                continue;
            }

            // Record the first relevant namespace dependency for this import.
            self.record_dependency(cx, source, target, item.span);
            break;
        }
    }
}

impl BidirectionalModuleDependencies {
    /// Emits a diagnostic when the latest dependency completes an unreported cycle.
    fn report_cycle(&mut self, cx: &LateContext<'_>, dependency: ModuleDependency) {
        // Locate the import that closes the dependency cycle.
        let Some(reverse) = self.dependencies.iter().find(|candidate| {
            candidate.source == dependency.target && candidate.target == dependency.source
        }) else {
            return;
        };

        // Canonicalize both directions before consulting the reported-pair set.
        let pair = ModulePair::new(dependency.source, dependency.target);
        let reverse_pair = ModulePair::new(dependency.target, dependency.source);

        // Each dependency cycle is reported once regardless of traversal direction.
        if self.reported.contains(&pair) || self.reported.contains(&reverse_pair) {
            return;
        }

        // Emit one diagnostic that connects both contributing imports.
        self.reported.insert(pair);

        // Resolve both module identities before crossing the diagnostic boundary.
        Violation {
            span: dependency.span,
            reverse_span: reverse.span,
            source_module: cx.tcx.def_path_str(dependency.source.to_def_id()),
            target_module: cx.tcx.def_path_str(dependency.target.to_def_id()),
        }
        .emit(cx);
    }

    /// Records one resolved dependency and checks whether it closes a cycle.
    fn record_dependency(
        &mut self,
        cx: &LateContext<'_>,
        source: LocalDefId,
        target: LocalDefId,
        span: Span,
    ) {
        self.dependencies.push(ModuleDependency {
            source,
            target,
            span,
        });
        let dependency = *self.dependencies.last().expect("dependency was just added");
        self.report_cycle(cx, dependency);
    }
}
