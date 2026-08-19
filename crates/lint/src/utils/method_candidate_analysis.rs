extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_hir::def_id::LocalDefId;
use rustc_hir::{
    GenericParamKind, Generics, HirId, Item, ItemKind, Mutability, Param, PatKind, Ty as HirTy,
    TyKind,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_span::{Span, Symbol};

use super::free_function_analysis::FreeFunctionExt;

#[path = "method_migration.rs"]
pub mod migration;

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
// CandidateComponent: Candidate syntax and migration inputs
// -----------------------------------------------------------------------------

/// Simple binding extracted from a `candidate` function's first parameter.
struct CandidateComponentBinding {
    /// HIR identity used to find every body reference to the binding.
    id: Option<HirId>,
    /// Source symbol replaced by `self` during migration.
    name: Option<Symbol>,
}

/// Function generics that may safely move to a generated impl header.
struct CandidateComponentGenerics {
    /// Complete generic-parameter span to move, when present.
    span: Option<Span>,
    /// Whether generic syntax permits a mechanical migration.
    is_safe: bool,
}

/// Source identity and spans of a `candidate` free function.
struct CandidateComponentFunction {
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

/// Semantic receiver identity and syntax of a `candidate` free function.
struct CandidateComponentReceiver {
    /// First-parameter span replaced by receiver syntax.
    parameter_span: Span,
    /// Nominal receiver type span reused in the impl header.
    receiver_type_span: Span,
    /// Ownership form and local struct that should own the method.
    semantics: ReceiverSemantics,
    /// Struct name shown in user-facing guidance.
    struct_name: Symbol,
}

/// Syntax-safety facts governing a `candidate`'s automatic migration.
struct CandidateComponentMigration {
    /// Generic parameter span moved from the function to the impl.
    impl_generics_span: Option<Span>,
    /// Mechanically replaceable first-parameter binding.
    binding: CandidateComponentBinding,
    /// Whether all candidate-local syntax is safe to migrate.
    is_suggestible: bool,
}

// -----------------------------------------------------------------------------
// MethodCandidate: Discovered method relocation
// -----------------------------------------------------------------------------

/// A free function that belongs on a struct according to the rule.
pub struct MethodCandidate {
    /// Free-function identity and complete source span.
    function: CandidateComponentFunction,
    /// Receiver syntax and owning struct identity.
    receiver: CandidateComponentReceiver,
    /// Syntax-safety facts governing automatic migration.
    migration: CandidateComponentMigration,
}

impl MethodCandidate {
    /// Turns a free function into a `candidate` when its first parameter is a same-module struct.
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
    pub(crate) fn discover(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Require an authored free function body before extracting its syntax.
        if !item.is_authored_rust_free_function(cx) {
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

        let def_id = item.owner_id.def_id;

        // Resolve the semantic receiver and require its struct to share the module.
        let semantic_receiver = Self::semantic_receiver(cx, def_id)?;

        // Several peer values of the same type do not identify one clear behavioral subject.
        if Self::has_same_type_peer(cx, def_id, semantic_receiver.struct_def_id) {
            return None;
        }

        // A struct in another module should own behavior through an explicit public abstraction.
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
        let candidate_generics = Self::movable_generics(cx, generics, def_id);

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
        let function = CandidateComponentFunction {
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
        let receiver = CandidateComponentReceiver {
            parameter_span: parameter.span,
            receiver_type_span: receiver_syntax.span,
            semantics: semantic_receiver,
            struct_name,
        };

        // Retain only the syntax-safety facts needed by automatic migration.
        let migration = CandidateComponentMigration {
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
            // Other first-parameter shapes cannot become an inherent receiver.
            _ => return None,
        };

        // Require the resolved receiver target to be a local struct.
        // Non-aggregate receiver targets cannot own inherent methods.
        let ty::Adt(adt, _) = receiver_type.kind() else {
            return None;
        };

        // Enums and unions are outside this struct-owned method policy.
        if !adt.is_struct() {
            return None;
        }
        Some(ReceiverSemantics {
            kind,
            struct_def_id: adt.did().as_local()?,
        })
    }

    /// Returns whether a later parameter has the same nominal struct type as the candidate.
    fn has_same_type_peer(
        cx: &LateContext<'_>,
        function_def_id: LocalDefId,
        receiver_def_id: LocalDefId,
    ) -> bool {
        let signature = cx.tcx.fn_sig(function_def_id).instantiate_identity();
        let inputs = signature.inputs().skip_binder();
        inputs
            .iter()
            .skip(1)
            .copied()
            .any(|input| {
                let nominal = match input.kind() {
                    ty::Ref(_, inner, _) => *inner,
                    _ => input,
                };
                matches!(nominal.kind(), ty::Adt(adt, _) if adt.is_struct() && adt.did().as_local() == Some(receiver_def_id))
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
    const fn simple_binding(parameter: &Param<'_>) -> CandidateComponentBinding {
        let (id, name) = match parameter.pat.kind {
            PatKind::Binding(_, binding_id, binding, None) => {
                (Some(binding_id), Some(binding.name))
            }
            // Destructuring has no single name that can be replaced by `self` throughout the body.
            _ => (None, None),
        };
        CandidateComponentBinding { id, name }
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
        function_def_id: LocalDefId,
    ) -> CandidateComponentGenerics {
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

        // Without explicit type parameters, only safely movable implicit syntax matters.
        if explicit_type_params.is_empty() {
            // Reject implicit parameters that cannot move to an impl unchanged.
            let has_unsupported_parameter = generics
                .params
                .iter()
                .any(Self::is_unsupported_implicit_parameter);

            // Preserve warnings while withholding unsafe generic source movement.
            return CandidateComponentGenerics {
                span: None,
                is_safe: !has_unsupported_parameter,
            };
        }

        // A single unbounded type parameter can move wholesale to the impl. Bounds, defaults, and
        // multiple parameters may belong on either the impl or method, which is an API decision.
        let signature = cx.tcx.fn_sig(function_def_id).instantiate_identity();
        let first_type = *signature
            .inputs()
            .skip_binder()
            .first()
            .expect("method candidates always have a first parameter");
        let receiver_type = match first_type.kind() {
            ty::Ref(_, inner, _) => *inner,
            _ => first_type,
        };
        let parameter_name = explicit_type_params[0].name.ident().name;
        let receiver_mentions_parameter = receiver_type.walk().any(|argument| {
            argument.as_type().is_some_and(
                |ty| matches!(ty.kind(), ty::Param(param) if param.name == parameter_name),
            )
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

        CandidateComponentGenerics {
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

    /// Checks whether the struct already has an item with the proposed method name.
    ///
    /// A collision needs a naming decision from the author and therefore cannot be fixed
    /// automatically.
    pub(crate) fn has_method_collision(&self, cx: &LateContext<'_>) -> bool {
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

    /// Returns the free function's local definition identity.
    pub(crate) const fn definition_id(&self) -> LocalDefId {
        self.function.def_id
    }

    /// Returns the HIR node used to anchor the lint level.
    pub(crate) const fn hir_id(&self) -> HirId {
        self.function.hir_id
    }

    /// Returns the function identifier span used by the primary diagnostic.
    pub(crate) const fn name_span(&self) -> Span {
        self.function.name_span
    }

    /// Returns the authored free-function name.
    pub(crate) const fn function_name(&self) -> Symbol {
        self.function.name
    }

    /// Returns the same-module struct that owns the operation.
    pub(crate) const fn struct_name(&self) -> Symbol {
        self.receiver.struct_name
    }

    /// Describes the receiver syntax needed for a manual relocation.
    pub(crate) const fn receiver_description(&self) -> &'static str {
        self.receiver.semantics.kind.description()
    }

    /// Returns whether a source range is part of this function.
    fn contains(&self, span: Span) -> bool {
        self.function.item_span.lo() <= span.lo() && span.hi() <= self.function.item_span.hi()
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
pub struct MethodCandidateBindingUse {
    /// Source span occupied by this reference to the parameter binding.
    span: Span,
    /// Field name preserved when the reference appears as struct shorthand.
    shorthand_field: Option<Symbol>,
}

impl MethodCandidateBindingUse {
    /// Captures one binding reference and its optional struct-shorthand field.
    pub(crate) const fn new(span: Span, shorthand_field: Option<Symbol>) -> Self {
        Self {
            span,
            shorthand_field,
        }
    }
}

// -----------------------------------------------------------------------------
