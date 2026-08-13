extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, Expr, ExprKind, FnDecl, HirId, Item, ItemKind, Node};
use rustc_lint::{LateContext, LateLintPass};
use rustc_span::{Span, Symbol};

use crate::utils::collection_construction_analysis::CollectionConstructionAnalysis;
use crate::utils::comparison_analysis::ComparisonAnalysis;
use crate::utils::construction_analysis::ConstructionAnalysis;
use crate::utils::conversion_analysis::ConversionAnalysis;
use crate::utils::diagnostic::LateViolation;
use crate::utils::method_candidate_analysis::migration::{MigrationBuilder, MigrationEdit};
use crate::utils::method_candidate_analysis::{MethodCandidate, MethodCandidateBindingUse};
use crate::utils::standard_interface_analysis::StandardInterfaceAnalysis;

// -----------------------------------------------------------------------------
// Violation: Method relocation diagnostic
// -----------------------------------------------------------------------------
#[derive(Clone, Copy)]
/// Whether a `candidate` can reuse its authored function name as a method.
enum ViolationMigrationAvailability {
    /// The destination method name is free.
    Available,
    /// An existing method already occupies the destination name.
    NameCollision,
}

impl ViolationMigrationAvailability {
    /// Resolves whether a `candidate`'s authored name is available on its destination type.
    fn for_candidate(cx: &LateContext<'_>, candidate: &MethodCandidate) -> Self {
        if candidate.has_method_collision(cx) {
            Self::NameCollision
        } else {
            Self::Available
        }
    }
}

/// Concrete remediation available after crate-wide call-site and collision analysis.
enum ViolationRemediation {
    /// Complete machine-applicable definition and use-site migration.
    Migration(
        /// Verified edits covering the definition and every resolved use site.
        Vec<MigrationEdit>,
    ),
    /// Destination already has a method with the authored function name.
    NameCollision,
    /// Migration intent is known but one or more edits are not safely derivable.
    Manual {
        /// Receiver syntax that preserves the original first-parameter contract.
        receiver: &'static str,
    },
}

/// Method-like free function with complete relocation and call-site context.
struct Violation {
    /// Function HIR node used to anchor the lint level.
    hir_id: HirId,
    /// Function identifier span used as the primary diagnostic location.
    span: Span,
    /// Authored free-function name.
    function_name: Symbol,
    /// Same-module struct that owns the operation.
    struct_name: Symbol,
    /// Safest remediation supported by the collected crate facts.
    remediation: ViolationRemediation,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "free function `{}` should be an inherent method on `{}`",
            self.function_name, self.struct_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "`{}` behavior is hidden in the module namespace instead of being discoverable through `{}`",
            self.function_name, self.struct_name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        match &self.remediation {
            ViolationRemediation::Migration(_) => {
                Cow::Borrowed("move the function into an inherent impl and update its uses")
            }
            ViolationRemediation::NameCollision => Cow::Owned(format!(
                "remove this wrapper or choose a name other than the existing `{}` method",
                self.function_name
            )),
            ViolationRemediation::Manual { receiver } => Cow::Owned(format!(
                "move `{}` into an `impl {}` block and replace its first parameter with {receiver}",
                self.function_name, self.struct_name
            )),
        }
    }

    fn emit(self, cx: &LateContext<'_>) {
        let primary_message = self.primary_message().into_owned();
        let rationale_message = self.rationale_message().into_owned();
        let remediation_message = self.remediation_message().into_owned();

        // Emit after the exact migration form and its stable wording are resolved.
        cx.tcx.emit_node_span_lint(
            METHOD_LIKE_FREE_FUNCTIONS,
            self.hir_id,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary_message);
                diag.note(rationale_message);
                match self.remediation {
                    ViolationRemediation::Migration(edits) => diag.multipart_suggestion(
                        remediation_message,
                        edits
                            .into_iter()
                            .map(|edit| {
                                let span = edit.span();
                                (span, edit.into_replacement())
                            })
                            .collect(),
                        Applicability::MachineApplicable,
                    ),
                    ViolationRemediation::NameCollision | ViolationRemediation::Manual { .. } => {
                        diag.help(remediation_message)
                    }
                };
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// MethodLikeFreeFunctions: Receiver ownership policy
// -----------------------------------------------------------------------------
#[derive(Default)]
/// Collects the information needed to find misplaced functions and safely move them.
struct MethodLikeFreeFunctions {
    /// Free functions whose first parameter identifies a same-module struct.
    candidates: Vec<MethodCandidate>,
    /// Local parameter references that a receiver migration must rewrite.
    binding_uses: HashMap<HirId, Vec<MethodCandidateBindingUse>>,
    /// Resolved free-function references that must become qualified method paths.
    function_uses: HashMap<LocalDefId, Vec<Span>>,
    /// Imported functions for which moving the definition would strand an alias.
    imported_functions: HashSet<LocalDefId>,
    /// Target-construction evidence used to defer conversions to their result type.
    constructions: ConstructionAnalysis,
    /// One-source conversion ownership discovered across the crate.
    conversions: ConversionAnalysis,
    /// Standard comparison protocols that supersede generic method relocation.
    comparisons: ComparisonAnalysis,
    /// Standard collection protocols that supersede generic method relocation.
    collections: CollectionConstructionAnalysis,
    /// Canonical formatting protocols that supersede generic method relocation.
    interfaces: StandardInterfaceAnalysis,
}

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub METHOD_LIKE_FREE_FUNCTIONS,
    Warn,
    "enforces inherent methods for functions that operate on a same-module struct",
    MethodLikeFreeFunctions::default()
}

impl<'tcx> LateLintPass<'tcx> for MethodLikeFreeFunctions {
    /// Looks at each top-level item and remembers functions that may belong on a struct.
    ///
    /// Imports are recorded separately because moving an imported function could silently change
    /// what its alias means.
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Record stronger ownership protocols before generic method candidacy.
        self.constructions.record_item(cx, item);
        self.conversions.record_item(cx, item);

        // Comparison and collection protocols also take precedence over generic ownership.
        self.comparisons.record_item(cx, item);
        self.collections.record_item(cx, item);

        // Formatting ownership likewise needs complete crate-wide interface evidence.
        self.interfaces.record_item(cx, item);
        if self.record_function_import(item) {
            return;
        }

        let Some(candidate) = MethodCandidate::discover(cx, item) else {
            return;
        };
        self.candidates.push(candidate);
    }

    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        span: Span,
        def_id: LocalDefId,
    ) {
        // Collect stronger constructor and standard protocol ownership first.
        self.constructions
            .record_function(cx, kind, body, span, def_id);
        self.comparisons.record_function(cx, kind, body, def_id);
        self.collections.record_function(cx, kind, body, def_id);

        // Retain canonical text helpers for formatting-specific precedence.
        self.interfaces.record_function(cx, kind, body, def_id);
        let Some(candidate) = self.constructions.candidate(def_id) else {
            return;
        };
        self.conversions.record_function(cx, kind, body, candidate);
    }

    /// Remembers uses of local names and free functions that a later fix may need to rewrite.
    ///
    /// This pass only gathers facts. It does not offer a fix until the whole crate has been seen.
    fn check_expr(&mut self, cx: &LateContext<'_>, expr: &Expr<'_>) {
        let ExprKind::Path(qpath) = expr.kind else {
            return;
        };

        match cx.qpath_res(&qpath, expr.hir_id) {
            Res::Def(DefKind::Fn, def_id) => {
                if let Some(def_id) = def_id.as_local() {
                    self.function_uses
                        .entry(def_id)
                        .or_default()
                        .push(expr.span);
                }
            }
            Res::Local(binding_id) => {
                self.record_binding_use(cx, expr, binding_id);
            }
            _ => {}
        }
    }

    /// Reports every `candidate` after all possible references have been collected.
    ///
    /// Waiting until the end prevents a fix from overlooking a call that appears later in the
    /// source.
    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        let comparison_definitions = self.comparisons.reportable_definitions();
        let collection_definitions = self.collections.reportable_definitions();
        let formatting_definitions = self.interfaces.reportable_formatting_definitions(cx);

        // References are only complete after the entire crate has been visited. Waiting until now
        // lets a migration update every call site or decline the fix as one atomic decision.
        for candidate in &self.candidates {
            // Suppress generic relocation whenever a more specific protocol owns the definition.
            let definition = candidate.definition_id();
            let has_conversion = self
                .conversions
                .target_owned_definitions()
                .contains(&definition);

            // Check the remaining standard protocol analyzers together.
            let has_protocol = comparison_definitions.contains(&definition)
                || collection_definitions.contains(&definition)
                || formatting_definitions.contains(&definition);
            if has_conversion || has_protocol {
                continue;
            }

            // Generic method ownership remains the most specific available advice.
            self.emit_candidate(cx, candidate);
        }
    }
}

impl MethodLikeFreeFunctions {
    /// Returns the field name when `expr` is used as struct-literal shorthand.
    fn shorthand_field(cx: &LateContext<'_>, expr: &Expr<'_>) -> Option<Symbol> {
        cx.tcx
            .hir_parent_iter(expr.hir_id)
            .next()
            .and_then(|(_, node)| match node {
                Node::ExprField(field) if field.is_shorthand => Some(field.ident.name),
                _ => None,
            })
    }

    /// Records one source reference to a `candidate`'s first-parameter binding.
    fn record_binding_use(&mut self, cx: &LateContext<'_>, expr: &Expr<'_>, binding_id: HirId) {
        // Preserve struct-shorthand context alongside the binding reference span.
        let shorthand_field = Self::shorthand_field(cx, expr);
        let binding_use = MethodCandidateBindingUse::new(expr.span, shorthand_field);

        // Retain every source use under the local binding it resolves to.
        self.binding_uses
            .entry(binding_id)
            .or_default()
            .push(binding_use);
    }

    /// Records a function import and returns whether the item was an import.
    ///
    /// An imported alias is part of the function's public shape inside the module, so the fixer
    /// leaves that move to the author.
    fn record_function_import(&mut self, item: &Item<'_>) -> bool {
        let ItemKind::Use(path, _) = item.kind else {
            return false;
        };

        // Moving an imported function would also change the meaning of its alias. The lint still
        // applies, but remembering the import prevents an incomplete automatic migration.
        if let Some(Res::Def(DefKind::Fn, def_id)) = path.res.value_ns
            && let Some(def_id) = def_id.as_local()
        {
            self.imported_functions.insert(def_id);
        }
        true
    }
}

impl MethodLikeFreeFunctions {
    /// Builds an atomic migration unless a collision or unsafe edit prevents it.
    fn candidate_migration(
        &self,
        cx: &LateContext<'_>,
        candidate: &MethodCandidate,
        availability: ViolationMigrationAvailability,
    ) -> Option<Vec<MigrationEdit>> {
        matches!(availability, ViolationMigrationAvailability::Available)
            .then(|| {
                MigrationBuilder::new(
                    cx,
                    candidate,
                    &self.candidates,
                    &self.binding_uses,
                    &self.function_uses,
                    &self.imported_functions,
                )
                .build()
            })
            .flatten()
    }

    /// Emits the warning and includes a complete migration only when every edit is known to be
    /// safe.
    ///
    /// When no automatic migration is available, the help still explains the intended method form
    /// or the naming collision that requires a manual choice.
    fn emit_candidate(&self, cx: &LateContext<'_>, candidate: &MethodCandidate) {
        // Resolve name collisions and the complete safe migration before reporting.
        let availability = ViolationMigrationAvailability::for_candidate(cx, candidate);
        let migration = self.candidate_migration(cx, candidate, availability);

        // Preserve the exact safe migration or its crate-wide manual barrier.
        let remediation = match migration {
            Some(edits) => ViolationRemediation::Migration(edits),
            None if matches!(availability, ViolationMigrationAvailability::NameCollision) => {
                ViolationRemediation::NameCollision
            }
            None => ViolationRemediation::Manual {
                receiver: candidate.receiver_description(),
            },
        };

        // Capture the complete ownership and migration context before emitting.
        let violation = Violation {
            hir_id: candidate.hir_id(),
            span: candidate.name_span(),
            function_name: candidate.function_name(),
            struct_name: candidate.struct_name(),
            remediation,
        };

        // Emit only after every context-derived fact has crossed the violation boundary.
        violation.emit(cx);
    }
}
