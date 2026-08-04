extern crate rustc_abi;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_abi::ExternAbi;
use rustc_errors::{Applicability, DiagDecorator};
use rustc_hir::{
    Expr, ExprKind, GenericParamKind, Generics, HirId, Item, ItemKind, Mutability, Node, Param,
    PatKind, Ty as HirTy, TyKind,
    def::{DefKind, Res},
    def_id::LocalDefId,
};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::{Pos, Span, Symbol};

// -----------------------------------------------------------------------------
// Receiver: Receiver analysis
// -----------------------------------------------------------------------------

/// The form of `self` that preserves how the first parameter is passed.
#[derive(Clone, Copy)]
enum ReceiverKind {
    Value,
    Ref(Mutability),
}

impl ReceiverKind {
    /// Describes the receiver form in the help shown to the user.
    ///
    /// For example, `item: Item`, `item: &Item`, and `item: &mut Item` become `self`, `&self`, and
    /// `&mut self`, respectively.
    fn description(self) -> &'static str {
        match self {
            Self::Value => "`self`",
            Self::Ref(Mutability::Not) => "`&self`",
            Self::Ref(Mutability::Mut) => "`&mut self`",
        }
    }
}

struct ReceiverSemantics {
    kind: ReceiverKind,
    struct_def_id: LocalDefId,
}

struct ReceiverSyntax {
    span: Span,
    is_direct: bool,
}

// -----------------------------------------------------------------------------
// Candidate: Candidate discovery and collected state
// -----------------------------------------------------------------------------

struct CandidateBinding {
    id: Option<HirId>,
    name: Option<Symbol>,
}

struct CandidateGenerics {
    span: Option<Span>,
    is_safe: bool,
}

/// A free function that belongs on a struct according to the rule.
struct Candidate {
    def_id: LocalDefId,
    hir_id: HirId,
    name: Symbol,
    name_span: Span,
    item_span: Span,
    parameter_span: Span,
    receiver_type_span: Span,
    impl_generics_span: Option<Span>,
    binding_id: Option<HirId>,
    binding_name: Option<Symbol>,
    receiver_kind: ReceiverKind,
    struct_def_id: LocalDefId,
    struct_name: Symbol,
    is_suggestible: bool,
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
        let ItemKind::Fn {
            sig,
            ident,
            generics,
            body,
            has_body: true,
        } = item.kind
        else {
            return None;
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

        let semantic_receiver = Self::semantic_receiver(cx, def_id)?;
        if !Self::shares_module_with_struct(cx, def_id, semantic_receiver.struct_def_id) {
            return None;
        }

        let body = cx.tcx.hir_body(body);
        let parameter = body.params.first()?;
        let binding = Self::simple_binding(parameter);
        let receiver_syntax = Self::direct_receiver_type(
            cx,
            &sig.decl.inputs[0],
            semantic_receiver.kind,
            semantic_receiver.struct_def_id,
        );
        let candidate_generics = Self::movable_generics(cx, generics, receiver_syntax.span);

        // Warnings use semantic types and are intentionally broad. Suggestions require editable,
        // private source whose syntax can be moved without guessing about an API contract.
        let is_suggestible = item.vis_span.is_empty()
            && cx.tcx.hir_attrs(item.hir_id()).is_empty()
            && binding.id.is_some()
            && !item.span.from_expansion()
            && !parameter.span.from_expansion()
            && !cx
                .tcx
                .def_span(semantic_receiver.struct_def_id)
                .from_expansion()
            && receiver_syntax.is_direct
            && candidate_generics.is_safe;

        Some(Self {
            def_id,
            hir_id: item.hir_id(),
            name: ident.name,
            name_span: ident.span,
            item_span: item.span,
            parameter_span: parameter.span,
            receiver_type_span: receiver_syntax.span,
            impl_generics_span: candidate_generics.span,
            binding_id: binding.id,
            binding_name: binding.name,
            receiver_kind: semantic_receiver.kind,
            struct_def_id: semantic_receiver.struct_def_id,
            struct_name: cx
                .tcx
                .item_name(semantic_receiver.struct_def_id.to_def_id()),
            is_suggestible,
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
    fn simple_binding(parameter: &Param<'_>) -> CandidateBinding {
        match parameter.pat.kind {
            PatKind::Binding(_, binding_id, binding, None) => CandidateBinding {
                id: Some(binding_id),
                name: Some(binding.name),
            },
            // Destructuring has no single name that can be replaced by `self` throughout the body.
            _ => CandidateBinding {
                id: None,
                name: None,
            },
        }
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
        receiver_kind: ReceiverKind,
        struct_def_id: LocalDefId,
    ) -> ReceiverSyntax {
        let receiver_type = match (receiver_kind, declared_type.kind) {
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
        let is_direct_struct = matches!(
            receiver_type.kind,
            TyKind::Path(qpath)
                if matches!(
                    cx.qpath_res(&qpath, receiver_type.hir_id),
                    Res::Def(DefKind::Struct, def_id) if def_id == struct_def_id.to_def_id()
                )
        );
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
            let has_unsupported_parameter = generics.params.iter().any(|param| {
                !matches!(
                    param.kind,
                    GenericParamKind::Lifetime { .. }
                        | GenericParamKind::Type {
                            synthetic: true,
                            ..
                        }
                ) && !param.span.is_empty()
            });
            return CandidateGenerics {
                span: None,
                is_safe: !has_unsupported_parameter,
            };
        }

        // A single unbounded type parameter can move wholesale to the impl. Bounds, defaults, and
        // multiple parameters may belong on either the impl or method, which is an API decision.
        let can_move = explicit_type_params.len() == 1
            && generics.predicates.is_empty()
            && generics.params.iter().all(|param| {
                param.span.is_empty()
                    || matches!(
                        param.kind,
                        GenericParamKind::Type {
                            default: None,
                            synthetic: false
                        }
                    )
            })
            && cx
                .sess()
                .source_map()
                .span_to_snippet(receiver_type_span)
                .is_ok_and(|receiver| {
                    receiver.contains(explicit_type_params[0].name.ident().name.as_str())
                });

        CandidateGenerics {
            span: can_move.then_some(generics.span),
            is_safe: can_move,
        }
    }

    /// Returns whether a source range is part of this function.
    fn contains(&self, span: Span) -> bool {
        self.item_span.lo() <= span.lo() && span.hi() <= self.item_span.hi()
    }

    /// Checks whether the struct already has an item with the proposed method name.
    ///
    /// A collision needs a naming decision from the author and therefore cannot be fixed
    /// automatically.
    fn has_method_collision(&self, cx: &LateContext<'_>) -> bool {
        cx.tcx
            .inherent_impls(self.struct_def_id)
            .iter()
            .flat_map(|impl_id| cx.tcx.associated_items(*impl_id).in_definition_order())
            .any(|item| item.name() == self.name)
    }

    /// Builds an unambiguous path such as `crate::module::Struct::method` for rewritten call sites.
    ///
    /// For example, a function value written as `let callback = inspect;` can become
    /// `let callback = crate::Item::inspect;` without depending on imports at that location.
    fn qualified_method_path(&self, cx: &LateContext<'_>) -> String {
        let path = cx.tcx.def_path_str(self.struct_def_id.to_def_id());
        let struct_path = path.split_once("::").map_or_else(
            || format!("crate::{path}"),
            |(_, rest)| format!("crate::{rest}"),
        );
        format!("{struct_path}::{}", self.name)
    }
}

/// One place where the first parameter's name is used inside the function body.
#[derive(Clone, Copy)]
struct CandidateBindingUse {
    span: Span,
    shorthand_field: Option<Symbol>,
}

// -----------------------------------------------------------------------------
// Migration: Conservative automatic migration
// -----------------------------------------------------------------------------

/// One replacement made inside a larger source range.
struct MigrationEdit {
    span: Span,
    replacement: String,
}

/// A group of replacements made inside one larger source range.
#[derive(Default)]
struct MigrationEdits(Vec<MigrationEdit>);

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

/// Builds all edits needed to move one free function without leaving broken references behind.
struct MigrationBuilder<'rule, 'cx, 'tcx> {
    cx: &'cx LateContext<'tcx>,
    candidate: &'rule Candidate,
    candidates: &'rule [Candidate],
    binding_uses: &'rule HashMap<HirId, Vec<CandidateBindingUse>>,
    function_uses: &'rule HashMap<LocalDefId, Vec<Span>>,
    imported_functions: &'rule HashSet<LocalDefId>,
    internal_edits: MigrationEdits,
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
        Self {
            cx,
            candidate,
            candidates,
            binding_uses,
            function_uses,
            imported_functions,
            internal_edits: MigrationEdits::default(),
            external_edits: Vec::new(),
        }
    }

    /// Rejects moves that cannot be applied as one complete, non-overlapping change.
    ///
    /// This covers imports, generic call syntax, and interactions with other candidates that
    /// Rustfix would otherwise try to edit at the same time.
    fn check_whole_migration_is_safe(&self) -> Option<()> {
        if !self.candidate.is_suggestible
            || self.imported_functions.contains(&self.candidate.def_id)
        {
            return None;
        }

        // Generic call paths may carry turbofish arguments. Rewriting those correctly needs more
        // than replacing the resolved function path, so generic migrations currently stay local.
        if self.candidate.impl_generics_span.is_some()
            && self.function_uses.contains_key(&self.candidate.def_id)
        {
            return None;
        }

        // Rustfix applies all machine suggestions together. If this function refers to another
        // candidate, moving both would produce overlapping whole-item edits. Keep the caller as a
        // warning-only case and let the callee safely rewrite the reference inside it.
        let overlaps_another_migration = self.candidates.iter().any(|other| {
            other.def_id != self.candidate.def_id
                && self
                    .function_uses
                    .get(&other.def_id)
                    .is_some_and(|uses| uses.iter().any(|span| self.candidate.contains(*span)))
        });
        (!overlaps_another_migration).then_some(())
    }

    /// Reads the original source text covered by a compiler source range.
    fn snippet(&self, span: Span) -> Option<String> {
        self.cx.sess().source_map().span_to_snippet(span).ok()
    }

    /// Replaces the first parameter with the appropriate `self` spelling.
    ///
    /// It also moves a simple generic parameter to the `impl` and preserves explicit lifetimes and
    /// mutability where their meaning is clear.
    fn rewrite_receiver(&mut self) -> Option<String> {
        let binding_name = self.candidate.binding_name?;
        let parameter = self.snippet(self.candidate.parameter_span)?;
        let pattern = parameter.split_once(':')?.0.trim();
        if pattern != binding_name.as_str() && pattern != format!("mut {binding_name}") {
            return None;
        }

        // `mut binding: &T` permits reassigning the reference itself. `&mut self` only permits
        // mutating the referent, so that spelling cannot be migrated without semantic analysis.
        if matches!(self.candidate.receiver_kind, ReceiverKind::Ref(_))
            && pattern.starts_with("mut ")
        {
            return None;
        }

        let receiver = match self.candidate.receiver_kind {
            ReceiverKind::Value if pattern.starts_with("mut ") => "mut self".to_owned(),
            ReceiverKind::Value => "self".to_owned(),
            ReceiverKind::Ref(_) => {
                let inner = self.snippet(self.candidate.receiver_type_span)?;
                let offset = parameter.rfind(&inner)?;
                format!(
                    "{}self{}",
                    &parameter[..offset],
                    &parameter[offset + inner.len()..]
                )
                .split_once(':')?
                .1
                .trim()
                .to_owned()
            }
        };
        self.internal_edits
            .push(self.candidate.parameter_span, receiver);

        self.candidate.impl_generics_span.map_or_else(
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
        let mut function = self.snippet(self.candidate.item_span)?;
        self.internal_edits
            .apply_to(&mut function, self.candidate.item_span)?;
        let self_type = self.snippet(self.candidate.receiver_type_span)?;
        let moved = format!("impl{impl_generics} {self_type} {{\n{function}\n}}");

        let mut edits = vec![MigrationEdit {
            span: self.candidate.item_span,
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
                == source_map.span_to_filename(self.candidate.item_span)
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
        let binding_id = self.candidate.binding_id?;
        for use_ in self.binding_uses.get(&binding_id).into_iter().flatten() {
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
            .function_uses
            .get(&self.candidate.def_id)
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
// MethodLikeFreeFunctions: Lint pass and diagnostics
// -----------------------------------------------------------------------------

/// Collects the information needed to find misplaced functions and safely move them.
#[derive(Default)]
struct MethodLikeFreeFunctions {
    candidates: Vec<Candidate>,
    binding_uses: HashMap<HirId, Vec<CandidateBindingUse>>,
    function_uses: HashMap<LocalDefId, Vec<Span>>,
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

        if let Some(candidate) = Candidate::discover(cx, item) {
            self.candidates.push(candidate);
        }
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
                let shorthand_field =
                    cx.tcx
                        .hir_parent_iter(expr.hir_id)
                        .next()
                        .and_then(|(_, node)| match node {
                            Node::ExprField(field) if field.is_shorthand => Some(field.ident.name),
                            _ => None,
                        });
                self.binding_uses
                    .entry(binding_id)
                    .or_default()
                    .push(CandidateBindingUse {
                        span: expr.span,
                        shorthand_field,
                    });
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
    /// Emits the warning and includes a complete migration only when every edit is known to be safe.
    ///
    /// When no automatic migration is available, the help still explains the intended method form
    /// or the naming collision that requires a manual choice.
    fn emit_candidate(&self, cx: &LateContext<'_>, candidate: &Candidate) {
        let has_collision = candidate.has_method_collision(cx);
        let migration = (!has_collision)
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
            .flatten();

        cx.tcx.emit_node_span_lint(
            METHOD_LIKE_FREE_FUNCTIONS,
            candidate.hir_id,
            candidate.name_span,
            DiagDecorator(|diag| {
                diag.primary_message(format!(
                    "free function `{}` should be an inherent method on `{}`",
                    candidate.name, candidate.struct_name
                ));
                if let Some(edits) = migration {
                    diag.multipart_suggestion(
                        "move the function into an inherent impl and update its uses",
                        edits
                            .into_iter()
                            .map(|edit| (edit.span, edit.replacement))
                            .collect(),
                        Applicability::MachineApplicable,
                    );
                } else if has_collision {
                    diag.help(format!(
                        "remove this wrapper or choose a name other than the existing `{}` method",
                        candidate.name
                    ));
                } else {
                    diag.help(format!(
                        "move `{}` into an `impl {}` block and replace its first parameter with {}",
                        candidate.name,
                        candidate.struct_name,
                        candidate.receiver_kind.description()
                    ));
                }
            }),
        );
    }
}
