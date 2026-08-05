extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_errors::DiagDecorator;
use rustc_hir::def::Res;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;

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
    /// ### What it does
    ///
    /// Finds sibling modules that import from each other in both directions. Dependencies between
    /// parents and children are not considered because child modules routinely use names defined by
    /// their parent.
    ///
    /// ### Why is this bad?
    ///
    /// A two-way dependency leaves neither sibling as the lower-level abstraction. Moving shared
    /// concepts into a neutral module or choosing one dependency direction makes ownership clearer
    /// and lets either side evolve without preserving an accidental cycle.
    ///
    /// For example, these sibling modules form a cycle:
    ///
    /// ```rust
    /// mod parser {
    ///     use super::syntax::Syntax;
    ///     pub struct Parser(pub Syntax);
    /// }
    /// mod syntax {
    ///     use super::parser::Parser;
    ///     pub struct Syntax;
    ///     impl Syntax { fn parse(_: Parser) {} }
    /// }
    /// ```
    ///
    /// Extract shared concepts or move the coordinating behavior so imports flow one way:
    ///
    /// ```rust
    /// mod syntax { pub struct Syntax; }
    /// mod parser {
    ///     use super::syntax::Syntax;
    ///     pub struct Parser(pub Syntax);
    /// }
    /// ```
    pub BIDIRECTIONAL_MODULE_DEPENDENCIES,
    Warn,
    "rejects imports that create dependency cycles between sibling modules",
    BidirectionalModuleDependencies::default()
}

impl LateLintPass<'_> for BidirectionalModuleDependencies {
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        let ItemKind::Use(path, _) = item.kind else {
            return;
        };
        let source = cx.tcx.parent_module(item.hir_id()).to_local_def_id();
        for resolution in [path.res.type_ns, path.res.value_ns, path.res.macro_ns]
            .into_iter()
            .flatten()
        {
            // Resolve one imported namespace to a local definition.
            let Some(target_definition) = resolution
                .opt_def_id()
                .and_then(rustc_span::def_id::DefId::as_local)
            else {
                continue;
            };

            // Normalize imported items to the module that owns them.
            let target = if matches!(resolution, Res::Def(rustc_hir::def::DefKind::Mod, _)) {
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
        if self.reported.contains(&pair) || self.reported.contains(&reverse_pair) {
            return;
        }

        // Emit one diagnostic that connects both contributing imports.
        self.reported.insert(pair);

        // Relate the two import sites and explain how to remove the cycle.
        cx.emit_span_lint(
            BIDIRECTIONAL_MODULE_DEPENDENCIES,
            dependency.span,
            DiagDecorator(|diag| {
                diag.primary_message(
                    "these sibling modules depend on each other in both directions",
                );
                diag.span_label(reverse.span, "the reverse dependency is introduced here");
                diag.help(
                    "extract shared concepts or choose one direction for the module dependency",
                );
            }),
        );
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
