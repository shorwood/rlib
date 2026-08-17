extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::collections::HashSet;

use rustc_hir::def::Res;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BodyId, Expr, ExprKind, HirId, LetStmt, PatKind};
use rustc_lint::LateContext;
use rustc_span::def_id::LocalDefId;
use rustc_span::hygiene::{ExpnKind, MacroKind};

// -----------------------------------------------------------------------------
// ViewBindings: Exclusive native binding analysis
// -----------------------------------------------------------------------------

/// Whether an expression carries the binding through only transparent syntax.
enum Origin {
    /// The analyzed binding is absent from the expression.
    Absent,
    /// The binding is present and retains its identity.
    Transparent,
    /// The binding is present behind behavior-bearing syntax.
    Transformed,
}

/// Detects any local binding in one tracked authority set.
struct TrackedBindingFinder<'analysis, 'tcx> {
    /// Compiler context used to resolve local paths.
    cx: &'analysis LateContext<'tcx>,
    /// Local bindings that carry the tracked authority.
    tracked: &'analysis HashSet<HirId>,
    /// Whether traversal reached a tracked binding.
    has_found: bool,
}

impl<'tcx> Visitor<'tcx> for TrackedBindingFinder<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        // The first tracked reference completes this existence query.
        if let ExprKind::Path(path) = expression.kind
            && matches!(self.cx.qpath_res(&path, expression.hir_id), Res::Local(binding) if self.tracked.contains(&binding))
        {
            self.has_found = true;
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

/// Body visitor proving that every tracked use belongs to one native binding.
struct ExclusiveBinding<'analysis, 'tcx> {
    /// Compiler context used for semantic path and method resolution.
    cx: &'analysis LateContext<'tcx>,
    /// Body owner whose type-checking results resolve method calls.
    owner: LocalDefId,
    /// Parameter and transparent local aliases carrying the same authority.
    tracked: HashSet<HirId>,
    /// Native binding expressions consuming the capability.
    bindings: HashSet<HirId>,
    /// Whether any use escaped the accepted forwarding shape.
    has_escaped: bool,
}

impl<'analysis, 'tcx> ExclusiveBinding<'analysis, 'tcx> {
    /// Builds analysis for one authored component parameter.
    fn new(cx: &'analysis LateContext<'tcx>, owner: LocalDefId, binding: HirId) -> Self {
        Self {
            cx,
            owner,
            tracked: HashSet::from([binding]),
            bindings: HashSet::new(),
            has_escaped: false,
        }
    }

    /// Returns whether an expression contains any currently tracked binding.
    fn contains_tracked(&self, expression: &'tcx Expr<'tcx>) -> bool {
        let mut finder = TrackedBindingFinder {
            cx: self.cx,
            tracked: &self.tracked,
            has_found: false,
        };
        finder.visit_expr(expression);
        finder.has_found
    }

    /// Returns whether a method call resolves to semantic `Clone::clone`.
    fn is_clone(&self, expression: &Expr<'_>) -> bool {
        // Resolve both the selected method and its declaring trait.
        // Unresolved calls cannot prove the standard identity-preserving clone operation.
        let Some(def_id) = self
            .cx
            .tcx
            .typeck(self.owner)
            .type_dependent_def_id(expression.hir_id)
        else {
            return false;
        };

        // Confirm the resolved method belongs to the standard clone trait.
        self.cx.tcx.crate_name(def_id.krate).as_str() == "core"
            && self.cx.tcx.item_name(def_id).as_str() == "clone"
            && self
                .cx
                .tcx
                .trait_of_assoc(def_id)
                .is_some_and(|trait_id| self.cx.tcx.item_name(trait_id).as_str() == "Clone")
    }

    /// Classifies a value without following behavior-bearing transformations.
    fn origin(&self, expression: &'tcx Expr<'tcx>) -> Origin {
        // Resolve direct local paths before considering transparent wrapper syntax.
        // A local path determines origin without inspecting any nested behavior.
        if let ExprKind::Path(path) = expression.kind {
            return match self.cx.qpath_res(&path, expression.hir_id) {
                Res::Local(binding) if self.tracked.contains(&binding) => Origin::Transparent,
                _ => Origin::Absent,
            };
        }

        // Tuple origin is the conservative combination of every element origin.
        if let ExprKind::Tup(elements) = expression.kind {
            return self.combined_origin(elements);
        }

        // Peel compiler and authored wrappers that do not add behavior.
        if let ExprKind::Block(block, None) = expression.kind
            && block.stmts.is_empty()
        {
            // An empty transparent block inherits the origin of its optional tail value.
            return block
                .expr
                .map_or(Origin::Absent, |value| self.origin(value));
        }

        // Compiler temporary wrappers preserve the wrapped value's origin.
        if let ExprKind::DropTemps(value) = expression.kind {
            return self.origin(value);
        }

        // Accept clone only when method resolution proves the standard identity operation.
        if let ExprKind::MethodCall(_, receiver, arguments, _) = expression.kind
            && arguments.is_empty()
            && self.is_clone(expression)
        {
            // Standard cloning preserves the tracked capability identity.
            return self.origin(receiver);
        }

        // Any other expression containing the binding has transformed or consumed it.
        if self.contains_tracked(expression) {
            return Origin::Transformed;
        }
        Origin::Absent
    }

    /// Combines tuple element origins while preserving any transformation evidence.
    fn combined_origin(&self, expressions: &'tcx [Expr<'tcx>]) -> Origin {
        expressions
            .iter()
            .fold(Origin::Absent, |combined, expression| {
                match (combined, self.origin(expression)) {
                    (Origin::Transformed, _) | (_, Origin::Transformed) => Origin::Transformed,
                    (Origin::Transparent, _) | (_, Origin::Transparent) => Origin::Transparent,
                    _ => Origin::Absent,
                }
            })
    }

    /// Returns the value argument of a resolved native `bind:*` expansion call.
    fn native_bind_value(&self, expression: &'tcx Expr<'tcx>) -> Option<&'tcx Expr<'tcx>> {
        // Resolve the method, declaring trait, and authored macro provenance together.
        // Only method calls can represent expanded native binding operations.
        let ExprKind::MethodCall(_, _, arguments, _) = expression.kind else {
            return None;
        };

        // Resolve the selected method and its declaring trait.
        let def_id = self
            .cx
            .tcx
            .typeck(self.owner)
            .type_dependent_def_id(expression.hir_id)?;

        // Confirm the declaring trait and native binding contract.
        let trait_id = self.cx.tcx.trait_of_assoc(def_id)?;
        let is_bind_method = self.cx.tcx.crate_name(def_id.krate).as_str() == "tachys"
            && self.cx.tcx.item_name(def_id).as_str() == "bind"
            && self.cx.tcx.item_name(trait_id).as_str() == "BindAttribute";

        // Require authored `view!` provenance in addition to semantic method identity.
        let is_view_expansion = expression.span.macro_backtrace().any(|expansion| {
            matches!(expansion.kind, ExpnKind::Macro(MacroKind::Bang, name) if name.as_str() == "view")
        });
        (is_bind_method && is_view_expansion)
            .then(|| arguments.last())
            .flatten()
    }

    /// Visits the expanded binding call without revisiting its accepted value argument.
    fn visit_native_bind_context(&mut self, expression: &'tcx Expr<'tcx>) {
        // Preserve traversal only for the method-call shape already classified as a bind.
        let ExprKind::MethodCall(_, receiver, arguments, _) = expression.kind else {
            return;
        };
        self.visit_expr(receiver);

        // A bind call without arguments has no accepted value to exclude from traversal.
        let Some((_, preceding)) = arguments.split_last() else {
            return;
        };
        for argument in preceding {
            self.visit_expr(argument);
        }
    }
}

impl<'tcx> Visitor<'tcx> for ExclusiveBinding<'_, 'tcx> {
    fn visit_nested_body(&mut self, body_id: BodyId) {
        self.visit_body(self.cx.tcx.hir_body(body_id));
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let Some(value) = self.native_bind_value(expression) {
            match self.origin(value) {
                // Transparent binding completes accepted-context traversal without visiting value.
                Origin::Transparent => {
                    self.bindings.insert(expression.hir_id);
                    self.visit_native_bind_context(expression);
                    return;
                }
                // A transformed binding proves escape without further descendant traversal.
                Origin::Transformed => {
                    self.has_escaped = true;
                    return;
                }
                Origin::Absent => {}
            }
        }

        // A tracked path outside an accepted native bind is direct escape evidence.
        if let ExprKind::Path(path) = expression.kind
            && matches!(self.cx.qpath_res(&path, expression.hir_id), Res::Local(binding) if self.tracked.contains(&binding))
        {
            self.has_escaped = true;
            return;
        }
        intravisit::walk_expr(self, expression);
    }

    fn visit_local(&mut self, local: &'tcx LetStmt<'tcx>) {
        // Locals without initializers cannot create transparent aliases.
        let Some(initializer) = local.init else {
            intravisit::walk_local(self, local);
            return;
        };

        // A transparent plain binding creates an alias whose initializer must not be revisited.
        if matches!(self.origin(initializer), Origin::Transparent)
            && let PatKind::Binding(_, binding, _, None) = local.pat.kind
        {
            self.tracked.insert(binding);
            return;
        }
        intravisit::walk_local(self, local);
    }
}

/// Semantic analysis of bindings produced by Leptos `view!` expansion.
///
/// This deliberately models only resolved expansion evidence needed by current lints. It should be
/// consolidated with a broader authored/semantic view representation when comment, section, and
/// attribute-group lints introduce source-token analysis.
pub struct ViewBindings;

impl ViewBindings {
    /// Returns whether one prop flows exclusively into exactly one native `bind:*` call.
    pub fn exclusively_forwards_to_native_bind(
        cx: &LateContext<'_>,
        owner: LocalDefId,
        binding: HirId,
    ) -> bool {
        let body = cx.tcx.hir_body_owned_by(owner);
        let mut analysis = ExclusiveBinding::new(cx, owner, binding);
        analysis.visit_body(body);
        !analysis.has_escaped && analysis.bindings.len() == 1
    }
}
