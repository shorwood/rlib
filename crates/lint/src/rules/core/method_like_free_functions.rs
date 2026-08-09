extern crate rustc_abi;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use rustc_abi::ExternAbi;
use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{
    Expr, ExprKind, GenericParamKind, Generics, HirId, Item, ItemKind, Mutability, Node, Param,
    PatKind, Ty as HirTy, TyKind,
};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::{Pos, Span, Symbol};

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// Receiver: Receiver analysis
// -----------------------------------------------------------------------------

/// The form of `self` that preserves how the first parameter is passed.
#[derive(Clone, Copy)]
enum ReceiverKind {
    /// Receiver owns the original first-parameter value.
    Value,
    /// Receiver borrows the value with the parameter's original mutability.
    Ref(
        /// Borrow mutability preserved by the generated receiver.
        Mutability,
    ),
}

impl ReceiverKind {
    /// Describes the receiver form in the help shown to the user.
    ///
    /// For example, `item: Item`, `item: &Item`, and `item: &mut Item` become `self`, `&self`, and
    /// `&mut self`, respectively.
    const fn description(self) -> &'static str {
        match self {
            Self::Value => "`self`",
            Self::Ref(Mutability::Not) => "`&self`",
            Self::Ref(Mutability::Mut) => "`&mut self`",
        }
    }
}

/// Semantic first-parameter type resolved independently of its source spelling.
#[derive(Clone, Copy)]
struct ReceiverSemantics {
    /// Method receiver form preserving ownership and mutability.
    kind: ReceiverKind,
    /// Local struct definition received by the free function.
    struct_def_id: LocalDefId,
}

/// Source-level receiver type and whether it can be migrated verbatim.
struct ReceiverSyntax {
    /// Span of the nominal type used to form the inherent impl header.
    span: Span,
    /// Whether source syntax names the struct directly rather than through an alias.
    is_direct: bool,
}

// -----------------------------------------------------------------------------
// Candidate: Candidate discovery and collected state
// -----------------------------------------------------------------------------

/// Simple binding extracted from a candidate function's first parameter.
struct CandidateBinding {
    /// HIR identity used to find every body reference to the binding.
    id: Option<HirId>,
    /// Source symbol replaced by `self` during migration.
    name: Option<Symbol>,
}

/// Function generics that may safely move to a generated impl header.
struct CandidateGenerics {
    /// Complete generic-parameter span to move, when present.
    span: Option<Span>,
    /// Whether generic syntax permits a mechanical migration.
    is_safe: bool,
}

/// Source identity and spans of a candidate free function.
struct CandidateFunction {
    /// Local definition identity of the free function.
    def_id: LocalDefId,
    /// HIR node on which the diagnostic is emitted.
    hir_id: HirId,
    /// Function name reused as the proposed method name.
    name: Symbol,
    /// Identifier span used as the primary diagnostic location.
    name_span: Span,
    /// Complete function span replaced by the generated impl.
    item_span: Span,
}

/// Semantic receiver identity and syntax of a candidate free function.
struct CandidateReceiver {
    /// First-parameter span replaced by receiver syntax.
    parameter_span: Span,
    /// Nominal receiver type span reused in the impl header.
    receiver_type_span: Span,
    /// Ownership form and local struct that should own the method.
    semantics: ReceiverSemantics,
    /// Struct name shown in user-facing guidance.
    struct_name: Symbol,
}

/// Syntax-safety facts governing a candidate's automatic migration.
struct CandidateMigration {
    /// Generic parameter span moved from the function to the impl.
    impl_generics_span: Option<Span>,
    /// Mechanically replaceable first-parameter binding.
    binding: CandidateBinding,
    /// Whether all candidate-local syntax is safe to migrate.
    is_suggestible: bool,
}

/// A free function that belongs on a struct according to the rule.
struct Candidate {
    /// Free-function identity and complete source span.
    function: CandidateFunction,
    /// Receiver syntax and owning struct identity.
    receiver: CandidateReceiver,
    /// Syntax-safety facts governing automatic migration.
    migration: CandidateMigration,
}

impl Candidate {
    /// Turns a free function into a candidate when its first parameter is a same-module struct.
    ///
    /// The warning follows the meaning of the types, while the automatic fix also checks whether
    /// the original spelling can be moved without inventing code.
    ///
    /// ```rust
    /// struct Item;
    ///
    /// // This is a candidate because `Item` and `inspect` share a module.
    /// fn inspect(item: &Item) {}
    /// ```
    fn discover(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Require an authored free function body before extracting its syntax.
        if !matches!(item.kind, ItemKind::Fn { has_body: true, .. }) {
            return None;
        }

        // Extract the signature and body from the validated function.
        let ItemKind::Fn { sig, body, .. } = item.kind else {
            return None;
        };
        let ident = item.kind.ident()?;

        // Extract generic syntax independently for impl-migration analysis.
        let ItemKind::Fn { generics, .. } = item.kind else {
            unreachable!();
        };

        // Associated items, nested functions, and foreign ABIs cannot become the inherent method
        // described by this rule without changing what the program means.
        let def_id = item.owner_id.def_id;
        let parent = cx.tcx.opt_local_parent(def_id)?;
        if cx.tcx.def_kind(parent) != DefKind::Mod || sig.header.abi != ExternAbi::Rust {
            return None;
        }

        // External generators are not under the crate author's control. Local macros remain in
        // scope because their definitions can be changed with the rest of the crate.
        if item.span.in_external_macro(cx.sess().source_map()) {
            return None;
        }

        // Resolve the semantic receiver and require its struct to share the module.
        let semantic_receiver = Self::semantic_receiver(cx, def_id)?;
        if !Self::shares_module_with_struct(cx, def_id, semantic_receiver.struct_def_id) {
            return None;
        }

        // Resolve the first parameter's binding and direct receiver syntax.
        let body = cx.tcx.hir_body(body);
        let parameter = body.params.first()?;
        let binding = Self::simple_binding(parameter);
        let declared_receiver = &sig.decl.inputs[0];
        let receiver_syntax = Self::direct_receiver_type(cx, declared_receiver, semantic_receiver);

        // Separate impl-level generics from syntax that must remain on the method.
        let candidate_generics = Self::movable_generics(cx, generics, receiver_syntax.span);

        // Warnings use semantic types and are intentionally broad. Suggestions require editable,
        // private source whose syntax can be moved without guessing about an API contract.
        let source_is_editable = item.vis_span.is_empty()
            && cx.tcx.hir_attrs(item.hir_id()).is_empty()
            && binding.id.is_some()
            && !item.span.from_expansion()
            && !parameter.span.from_expansion();

        // Require direct authored receiver syntax and a source-controlled struct.
        let receiver_is_editable = !cx
            .tcx
            .def_span(semantic_receiver.struct_def_id)
            .from_expansion()
            && receiver_syntax.is_direct;

        // Combine local source safety with generic-movement safety.
        let is_suggestible =
            source_is_editable && receiver_is_editable && candidate_generics.is_safe;

        // Group the free function's identity and complete replacement span.
        let function = CandidateFunction {
            def_id,
            hir_id: item.hir_id(),
            name: ident.name,
            name_span: ident.span,
            item_span: item.span,
        };

        // Group receiver syntax with the struct that should own the method.
        let struct_name = cx
            .tcx
            .item_name(semantic_receiver.struct_def_id.to_def_id());

        // Retain direct receiver syntax and its semantic owning struct.
        let receiver = CandidateReceiver {
            parameter_span: parameter.span,
            receiver_type_span: receiver_syntax.span,
            semantics: semantic_receiver,
            struct_name,
        };

        // Retain only the syntax-safety facts needed by automatic migration.
        let migration = CandidateMigration {
            impl_generics_span: candidate_generics.span,
            binding,
            is_suggestible,
        };

        // Assemble the candidate from its three independently meaningful concerns.
        Some(Self {
            function,
            receiver,
            migration,
        })
    }

    /// Finds the struct represented by the first parameter and the matching `self` form.
    ///
    /// Type aliases are followed here so the warning reflects what the parameter really is rather
    /// than only how its type was written.
    ///
    /// ```rust
    /// struct Item;
    /// type ItemAlias = Item;
    ///
    /// // The first parameter still represents `Item`.
    /// fn inspect(item: &ItemAlias) {}
    /// ```
    fn semantic_receiver(
        cx: &LateContext<'_>,
        function_def_id: LocalDefId,
    ) -> Option<ReceiverSemantics> {
        // Semantic types make transparent aliases behave like the struct they name.
        let signature = cx.tcx.fn_sig(function_def_id).instantiate_identity();
        let first_type = *signature.inputs().skip_binder().first()?;
        let (kind, receiver_type) = match first_type.kind() {
            ty::Adt(..) => (ReceiverKind::Value, first_type),
            ty::Ref(_, inner, mutability) => (ReceiverKind::Ref(*mutability), *inner),
            _ => return None,
        };

        // Require the resolved receiver target to be a local struct.
        let ty::Adt(adt, _) = receiver_type.kind() else {
            return None;
        };
        if !adt.is_struct() {
            return None;
        }
        Some(ReceiverSemantics {
            kind,
            struct_def_id: adt.did().as_local()?,
        })
    }

    /// Returns whether the function and struct are owned by the same source module.
    ///
    /// Code produced by an external macro is excluded because this crate cannot reasonably move
    /// or maintain it.
    fn shares_module_with_struct(
        cx: &LateContext<'_>,
        function_def_id: LocalDefId,
        struct_def_id: LocalDefId,
    ) -> bool {
        cx.tcx.parent_module_from_def_id(function_def_id)
            == cx.tcx.parent_module_from_def_id(struct_def_id)
            // An externally generated struct is no more editable than an externally generated
            // function, even when expansion makes both appear in the same module.
            && !cx
                .tcx
                .def_span(struct_def_id)
                .in_external_macro(cx.sess().source_map())
    }

    /// Extracts the name of a plain parameter such as `item` or `mut item`.
    ///
    /// Destructuring patterns return no name because there is no single word that can safely become
    /// `self` throughout the body.
    ///
    /// ```rust
    /// struct Item(u8);
    ///
    /// fn plain(item: Item) {}
    /// fn destructured(Item(value): Item) {}
    /// ```
    ///
    /// `plain` can be rewritten mechanically. `destructured` still receives a warning, but moving
    /// its pattern into a method body requires a decision from the author.
    const fn simple_binding(parameter: &Param<'_>) -> CandidateBinding {
        let (id, name) = match parameter.pat.kind {
            PatKind::Binding(_, binding_id, binding, None) => {
                (Some(binding_id), Some(binding.name))
            }
            // Destructuring has no single name that can be replaced by `self` throughout the body.
            _ => (None, None),
        };
        CandidateBinding { id, name }
    }

    /// Finds the exact source text that would become the type in an `impl` block.
    ///
    /// A reference hidden behind an alias is rejected for fixing because spelling out its lifetime
    /// correctly would require guessing, though the warning still applies.
    ///
    /// ```rust
    /// struct Item;
    /// type ItemRef<'a> = &'a Item;
    ///
    /// // Should this become `&self` or `&'a self`? The lint does not guess.
    /// fn inspect(item: ItemRef<'_>) {}
    /// ```
    fn direct_receiver_type(
        cx: &LateContext<'_>,
        declared_type: &HirTy<'_>,
        semantics: ReceiverSemantics,
    ) -> ReceiverSyntax {
        // Separate direct receiver syntax from aliases that hide reference semantics.
        let receiver_type = match (semantics.kind, declared_type.kind) {
            (ReceiverKind::Value, _) => declared_type,
            (ReceiverKind::Ref(_), TyKind::Ref(_, mut_ty)) => mut_ty.ty,
            // An alias can hide a reference and its lifetime contract. It remains lintable, but
            // spelling a receiver from that alias would require the fixer to invent syntax.
            (ReceiverKind::Ref(_), _) => {
                return ReceiverSyntax {
                    span: declared_type.span,
                    is_direct: false,
                };
            }
        };

        // Confirm that the authored receiver syntax directly names the semantic struct.
        let direct_definition = match receiver_type.kind {
            TyKind::Path(qpath) => cx.qpath_res(&qpath, receiver_type.hir_id).opt_def_id(),
            _ => None,
        };
        let is_direct_struct = direct_definition == Some(semantics.struct_def_id.to_def_id());

        // Retain the direct type span and whether it names the semantic struct.
        ReceiverSyntax {
            span: receiver_type.span,
            is_direct: is_direct_struct,
        }
    }

    /// Decides whether the function's generic parameters can move to the `impl` unchanged.
    ///
    /// A single plain type parameter is unambiguous. Bounds, defaults, or several parameters can be
    /// placed in meaningfully different locations, so those cases remain warning-only.
    ///
    /// ```rust
    /// struct Item<T>(T);
    ///
    /// fn take<T>(item: Item<T>) -> T {
    ///     item.0
    /// }
    ///
    /// // The safe migration is `impl<T> Item<T> { fn take(self) -> T { self.0 } }`.
    /// ```
    fn movable_generics(
        cx: &LateContext<'_>,
        generics: &Generics<'_>,
        receiver_type_span: Span,
    ) -> CandidateGenerics {
        // Collect explicit authored type parameters that could move to the impl.
        let explicit_type_params = generics
            .params
            .iter()
            .filter(|param| {
                matches!(
                    param.kind,
                    GenericParamKind::Type {
                        synthetic: false,
                        ..
                    }
                ) && !param.span.is_empty()
            })
            .collect::<Vec<_>>();

        if explicit_type_params.is_empty() {
            // Reject implicit parameters that cannot move to an impl unchanged.
            let has_unsupported_parameter = generics
                .params
                .iter()
                .any(Self::is_unsupported_implicit_parameter);

            // Preserve warnings while withholding unsafe generic source movement.
            return CandidateGenerics {
                span: None,
                is_safe: !has_unsupported_parameter,
            };
        }

        // A single unbounded type parameter can move wholesale to the impl. Bounds, defaults, and
        // multiple parameters may belong on either the impl or method, which is an API decision.
        let source_map = cx.sess().source_map();
        let receiver = source_map.span_to_snippet(receiver_type_span);
        let receiver_mentions_parameter = receiver.is_ok_and(|receiver| {
            receiver.contains(explicit_type_params[0].name.ident().name.as_str())
        });

        // Move only one unbounded parameter that visibly occurs in the receiver type.
        let has_one_unbounded_parameter =
            explicit_type_params.len() == 1 && generics.predicates.is_empty();

        // Reject defaults and authored parameters that cannot move to an impl unchanged.
        let parameters_are_movable = generics.params.iter().all(|param| {
            param.span.is_empty()
                || matches!(
                    param.kind,
                    GenericParamKind::Type {
                        default: None,
                        synthetic: false
                    }
                )
        });

        // Combine syntax, arity, and semantic receiver-use requirements.
        let can_move =
            has_one_unbounded_parameter && parameters_are_movable && receiver_mentions_parameter;

        CandidateGenerics {
            span: can_move.then_some(generics.span),
            is_safe: can_move,
        }
    }

    /// Returns whether an implicit or non-type parameter prevents safe generic movement.
    fn is_unsupported_implicit_parameter(param: &rustc_hir::GenericParam<'_>) -> bool {
        // Accept implicit lifetime parameters.
        let is_lifetime = matches!(param.kind, GenericParamKind::Lifetime { .. });

        // Accept compiler-synthesized type parameters.
        let is_synthetic_type = matches!(
            param.kind,
            GenericParamKind::Type {
                synthetic: true,
                ..
            }
        );

        // Reject any remaining authored generic parameter kind.
        !(is_lifetime || is_synthetic_type || param.span.is_empty())
    }

    /// Returns whether a source range is part of this function.
    fn contains(&self, span: Span) -> bool {
        self.function.item_span.lo() <= span.lo() && span.hi() <= self.function.item_span.hi()
    }

    /// Checks whether the struct already has an item with the proposed method name.
    ///
    /// A collision needs a naming decision from the author and therefore cannot be fixed
    /// automatically.
    fn has_method_collision(&self, cx: &LateContext<'_>) -> bool {
        // Resolve every inherent implementation for the candidate's receiver struct.
        let impls = cx
            .tcx
            .inherent_impls(self.receiver.semantics.struct_def_id)
            .iter();

        // Compare every associated item with the proposed method name.
        let associated =
            impls.flat_map(|impl_id| cx.tcx.associated_items(*impl_id).in_definition_order());
        associated
            .into_iter()
            .any(|item| item.name() == self.function.name)
    }

    /// Builds an unambiguous path such as `crate::module::Struct::method` for rewritten call sites.
    ///
    /// For example, a function value written as `let callback = inspect;` can become
    /// `let callback = crate::Item::inspect;` without depending on imports at that location.
    fn qualified_method_path(&self, cx: &LateContext<'_>) -> String {
        let path = cx
            .tcx
            .def_path_str(self.receiver.semantics.struct_def_id.to_def_id());
        let struct_path = path.split_once("::").map_or_else(
            || format!("crate::{path}"),
            |(_, rest)| format!("crate::{rest}"),
        );
        format!("{struct_path}::{}", self.function.name)
    }
}

/// One place where the first parameter's name is used inside the function body.
#[derive(Clone, Copy)]
struct CandidateBindingUse {
    /// Source span occupied by this reference to the parameter binding.
    span: Span,
    /// Field name preserved when the reference appears as struct shorthand.
    shorthand_field: Option<Symbol>,
}

// -----------------------------------------------------------------------------
// Migration: Conservative automatic migration
// -----------------------------------------------------------------------------

/// One replacement made inside a larger source range.
struct MigrationEdit {
    /// Source range replaced by this edit.
    span: Span,
    /// Complete replacement text for `span`.
    replacement: String,
}

/// A group of replacements made inside one larger source range.
#[derive(Default)]
struct MigrationEdits(
    /// Source replacements accumulated before offset-stable application.
    Vec<MigrationEdit>,
);

impl MigrationEdits {
    /// Adds one source replacement to the group.
    fn push(&mut self, span: Span, replacement: String) {
        self.0.push(MigrationEdit { span, replacement });
    }

    /// Applies every replacement to a source string while preserving the original offsets.
    ///
    /// Editing from right to left ensures that an earlier replacement cannot move the text used by
    /// a later one.
    fn apply_to(&mut self, source: &mut String, outer: Span) -> Option<()> {
        self.0
            .sort_unstable_by_key(|edit| std::cmp::Reverse(edit.span.lo()));
        for edit in &self.0 {
            let start = usize::try_from((edit.span.lo() - outer.lo()).to_u32()).ok()?;
            let end = usize::try_from((edit.span.hi() - outer.lo()).to_u32()).ok()?;
            source.replace_range(start..end, &edit.replacement);
        }
        Some(())
    }
}

/// Cross-reference indexes consulted while constructing one candidate migration.
struct MigrationReferences<'rule> {
    /// All candidates, used to prevent overlapping simultaneous migrations.
    candidates: &'rule [Candidate],
    /// Body references keyed by first-parameter binding identity.
    binding_uses: &'rule HashMap<HirId, Vec<CandidateBindingUse>>,
    /// Resolved references keyed by free-function definition.
    function_uses: &'rule HashMap<LocalDefId, Vec<Span>>,
    /// Functions whose imported aliases make automatic migration incomplete.
    imported_functions: &'rule HashSet<LocalDefId>,
}

/// Builds all edits needed to move one free function without leaving broken references behind.
struct MigrationBuilder<'rule, 'cx, 'tcx> {
    /// Compiler context used for snippets, paths, and source ownership.
    cx: &'cx LateContext<'tcx>,
    /// Candidate currently being migrated.
    candidate: &'rule Candidate,
    /// Cross-reference indexes needed to validate and rewrite the migration.
    references: MigrationReferences<'rule>,
    /// Replacements applied inside the candidate's whole-item edit.
    internal_edits: MigrationEdits,
    /// Call-site replacements outside the candidate function.
    external_edits: Vec<MigrationEdit>,
}

impl<'rule, 'cx, 'tcx> MigrationBuilder<'rule, 'cx, 'tcx> {
    /// Starts an empty migration for one candidate.
    fn new(
        cx: &'cx LateContext<'tcx>,
        candidate: &'rule Candidate,
        candidates: &'rule [Candidate],
        binding_uses: &'rule HashMap<HirId, Vec<CandidateBindingUse>>,
        function_uses: &'rule HashMap<LocalDefId, Vec<Span>>,
        imported_functions: &'rule HashSet<LocalDefId>,
    ) -> Self {
        // Group the shared reference indexes used by safety checks and rewrites.
        let references = MigrationReferences {
            candidates,
            binding_uses,
            function_uses,
            imported_functions,
        };

        // Initialize candidate-local edits independently from external call-site edits.
        Self {
            cx,
            candidate,
            references,
            internal_edits: MigrationEdits::default(),
            external_edits: Vec::new(),
        }
    }

    /// Returns whether migrating `other` would overlap this candidate's whole-item edit.
    fn overlaps_candidate_migration(&self, other: &Candidate) -> bool {
        other.function.def_id != self.candidate.function.def_id
            && self
                .references
                .function_uses
                .get(&other.function.def_id)
                .is_some_and(|uses| uses.iter().any(|span| self.candidate.contains(*span)))
    }

    /// Rejects moves that cannot be applied as one complete, non-overlapping change.
    ///
    /// This covers imports, generic call syntax, and interactions with other candidates that
    /// Rustfix would otherwise try to edit at the same time.
    fn check_whole_migration_is_safe(&self) -> Option<()> {
        // Require editable local syntax with no imported alias contract.
        if !self.candidate.migration.is_suggestible
            || self
                .references
                .imported_functions
                .contains(&self.candidate.function.def_id)
        {
            return None;
        }

        // Generic call paths may carry turbofish arguments. Rewriting those correctly needs more
        // than replacing the resolved function path, so generic migrations currently stay local.
        if self.candidate.migration.impl_generics_span.is_some()
            && self
                .references
                .function_uses
                .contains_key(&self.candidate.function.def_id)
        {
            return None;
        }

        // Rustfix applies all machine suggestions together. If this function refers to another
        // candidate, moving both would produce overlapping whole-item edits. Keep the caller as a
        // warning-only case and let the callee safely rewrite the reference inside it.
        let overlaps_another_migration = self
            .references
            .candidates
            .iter()
            .any(|other| self.overlaps_candidate_migration(other));
        (!overlaps_another_migration).then_some(())
    }

    /// Reads the original source text covered by a compiler source range.
    fn snippet(&self, span: Span) -> Option<String> {
        let source_map = self.cx.sess().source_map();
        source_map.span_to_snippet(span).ok()
    }

    /// Preserves explicit reference syntax while replacing its binding with `self`.
    fn rewrite_reference_receiver(&self, parameter: &str) -> Option<String> {
        let inner = self.snippet(self.candidate.receiver.receiver_type_span)?;
        let offset = parameter.rfind(&inner)?;
        format!(
            "{}self{}",
            &parameter[..offset],
            &parameter[offset + inner.len()..]
        )
        .split_once(':')
        .map(|(_, receiver)| receiver.trim().to_owned())
    }

    /// Replaces the first parameter with the appropriate `self` spelling.
    ///
    /// It also moves a simple generic parameter to the `impl` and preserves explicit lifetimes and
    /// mutability where their meaning is clear.
    fn rewrite_receiver(&mut self) -> Option<String> {
        // Resolve a plain binding and its complete first-parameter source.
        let binding_name = self.candidate.migration.binding.name?;
        let parameter = self.snippet(self.candidate.receiver.parameter_span)?;
        let pattern = parameter.split_once(':')?.0.trim();
        if pattern != binding_name.as_str() && pattern != format!("mut {binding_name}") {
            return None;
        }

        // `mut binding: &T` permits reassigning the reference itself. `&mut self` only permits
        // mutating the referent, so that spelling cannot be migrated without semantic analysis.
        if matches!(self.candidate.receiver.semantics.kind, ReceiverKind::Ref(_))
            && pattern.starts_with("mut ")
        {
            return None;
        }

        // Preserve ownership and mutability in the replacement receiver spelling.
        let receiver = match self.candidate.receiver.semantics.kind {
            ReceiverKind::Value if pattern.starts_with("mut ") => "mut self".to_owned(),
            ReceiverKind::Value => "self".to_owned(),
            ReceiverKind::Ref(_) => self.rewrite_reference_receiver(&parameter)?,
        };
        self.internal_edits
            .push(self.candidate.receiver.parameter_span, receiver);

        // Move safe generic syntax from the function to its new impl header.
        self.candidate.migration.impl_generics_span.map_or_else(
            || Some(String::new()),
            |span| {
                let generics = self.snippet(span)?;
                self.internal_edits.push(span, String::new());
                Some(generics)
            },
        )
    }

    /// Applies the edits inside the function, wraps it in an `impl`, and adds call-site edits.
    fn finish(mut self, impl_generics: &str) -> Option<Vec<MigrationEdit>> {
        // Apply candidate-local rewrites before wrapping the function in its impl.
        let mut function = self.snippet(self.candidate.function.item_span)?;
        self.internal_edits
            .apply_to(&mut function, self.candidate.function.item_span)?;
        let self_type = self.snippet(self.candidate.receiver.receiver_type_span)?;
        let moved = format!("impl{impl_generics} {self_type} {{\n{function}\n}}");

        // Combine the whole-item replacement with every external reference rewrite.
        let mut edits = vec![MigrationEdit {
            span: self.candidate.function.item_span,
            replacement: moved,
        }];
        edits.append(&mut self.external_edits);
        Some(edits)
    }

    /// Returns whether a source range is ordinary editable text in the candidate's file.
    ///
    /// Macro expansions and other files are rejected because the displayed edit would not own the
    /// text it claims to change.
    fn is_editable_in_candidate_file(&self, span: Span) -> bool {
        let source_map = self.cx.sess().source_map();
        !span.from_expansion()
            && source_map.span_to_filename(span)
                == source_map.span_to_filename(self.candidate.function.item_span)
    }

    /// Replaces uses of the old parameter name with `self` inside the function body.
    ///
    /// Struct shorthand such as `Snapshot { item }` becomes `Snapshot { item: self }` so the field
    /// name does not accidentally change.
    ///
    /// ```rust
    /// struct Item;
    /// struct Snapshot {
    ///     item: Item,
    /// }
    ///
    /// fn snapshot(item: Item) -> Snapshot {
    ///     Snapshot { item }
    /// }
    /// ```
    fn rewrite_binding_uses(&mut self) -> Option<()> {
        let binding_id = self.candidate.migration.binding.id?;
        for use_ in self
            .references
            .binding_uses
            .get(&binding_id)
            .into_iter()
            .flatten()
        {
            if !self.candidate.contains(use_.span) || !self.is_editable_in_candidate_file(use_.span)
            {
                return None;
            }
            let replacement = use_
                .shorthand_field
                .map_or_else(|| "self".to_owned(), |field| format!("{field}: self"));
            self.internal_edits.push(use_.span, replacement);
        }
        Some(())
    }

    /// Rewrites calls and function values to use the method's fully qualified path.
    ///
    /// A reference outside the candidate's source file makes the whole migration warning-only
    /// because one-file suggestions must not leave another file broken.
    ///
    /// A call such as `inspect(&item)` becomes `crate::Item::inspect(&item)`. Using the complete
    /// method name also preserves places where the old function was stored as a function value.
    fn rewrite_function_uses(&mut self) -> Option<()> {
        let qualified_method = self.candidate.qualified_method_path(self.cx);
        for span in self
            .references
            .function_uses
            .get(&self.candidate.function.def_id)
            .into_iter()
            .flatten()
        {
            if !self.is_editable_in_candidate_file(*span) {
                return None;
            }
            if self.candidate.contains(*span) {
                self.internal_edits.push(*span, qualified_method.clone());
            } else {
                self.external_edits.push(MigrationEdit {
                    span: *span,
                    replacement: qualified_method.clone(),
                });
            }
        }
        Some(())
    }

    /// Builds the complete edit set, or declines when any part of the move is uncertain.
    ///
    /// Each stage must succeed before the suggestion is marked as safe for automatic application.
    fn build(mut self) -> Option<Vec<MigrationEdit>> {
        self.check_whole_migration_is_safe()?;
        let impl_generics = self.rewrite_receiver()?;
        self.rewrite_binding_uses()?;
        self.rewrite_function_uses()?;
        self.finish(&impl_generics)
    }
}

// -----------------------------------------------------------------------------
// Violation: Method relocation diagnostic
// -----------------------------------------------------------------------------

/// Whether a candidate can reuse its authored function name as a method.
#[derive(Clone, Copy)]
enum ViolationMigrationAvailability {
    /// The destination method name is free.
    Available,
    /// An existing method already occupies the destination name.
    NameCollision,
}

impl ViolationMigrationAvailability {
    /// Resolves whether a candidate's authored name is available on its destination type.
    fn for_candidate(cx: &LateContext<'_>, candidate: &Candidate) -> Self {
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
        // Render the stable diagnostic layers before moving migration edits.
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
                            .map(|edit| (edit.span, edit.replacement))
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
// MethodLikeFreeFunctions: Lint pass
// -----------------------------------------------------------------------------

/// Collects the information needed to find misplaced functions and safely move them.
#[derive(Default)]
struct MethodLikeFreeFunctions {
    /// Free functions whose first parameter identifies a same-module struct.
    candidates: Vec<Candidate>,
    /// Local parameter references that a receiver migration must rewrite.
    binding_uses: HashMap<HirId, Vec<CandidateBindingUse>>,
    /// Resolved free-function references that must become qualified method paths.
    function_uses: HashMap<LocalDefId, Vec<Span>>,
    /// Imported functions for which moving the definition would strand an alias.
    imported_functions: HashSet<LocalDefId>,
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Checks for free functions whose first parameter can be the receiver of an inherent method
    /// on a struct defined in the same module.
    ///
    /// ### Why is this bad?
    ///
    /// Keeping behavior on the type it belongs to makes that behavior easier to discover and keeps
    /// the module's free-function namespace focused on operations that do not belong to one type.
    ///
    /// For example, this free function behaves like part of `Document`'s interface:
    ///
    /// ```rust
    /// struct Document;
    ///
    /// fn render(document: &Document) {}
    /// ```
    ///
    /// Making the first parameter the receiver puts the operation where callers expect it:
    ///
    /// ```rust
    /// struct Document;
    ///
    /// impl Document {
    ///     fn render(&self) {}
    /// }
    /// ```
    pub METHOD_LIKE_FREE_FUNCTIONS,
    Warn,
    "enforces inherent methods for functions that operate on a same-module struct",
    MethodLikeFreeFunctions::default()
}

impl LateLintPass<'_> for MethodLikeFreeFunctions {
    /// Looks at each top-level item and remembers functions that may belong on a struct.
    ///
    /// Imports are recorded separately because moving an imported function could silently change
    /// what its alias means.
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        if self.record_function_import(item) {
            return;
        }

        let Some(candidate) = Candidate::discover(cx, item) else {
            return;
        };
        self.candidates.push(candidate);
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

    /// Reports every candidate after all possible references have been collected.
    ///
    /// Waiting until the end prevents a fix from overlooking a call that appears later in the
    /// source.
    fn check_crate_post(&mut self, cx: &LateContext<'_>) {
        // References are only complete after the entire crate has been visited. Waiting until now
        // lets a migration update every call site or decline the fix as one atomic decision.
        for candidate in &self.candidates {
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

    /// Records one source reference to a candidate's first-parameter binding.
    fn record_binding_use(&mut self, cx: &LateContext<'_>, expr: &Expr<'_>, binding_id: HirId) {
        // Preserve struct-shorthand context alongside the binding reference span.
        let shorthand_field = Self::shorthand_field(cx, expr);
        let binding_use = CandidateBindingUse {
            span: expr.span,
            shorthand_field,
        };

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
        candidate: &Candidate,
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
    fn emit_candidate(&self, cx: &LateContext<'_>, candidate: &Candidate) {
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
                receiver: candidate.receiver.semantics.kind.description(),
            },
        };

        // Capture the complete ownership and migration context before emitting.
        let violation = Violation {
            hir_id: candidate.function.hir_id,
            span: candidate.function.name_span,
            function_name: candidate.function.name,
            struct_name: candidate.receiver.struct_name,
            remediation,
        };

        // Emit only after every context-derived fact has crossed the violation boundary.
        violation.emit(cx);
    }
}
