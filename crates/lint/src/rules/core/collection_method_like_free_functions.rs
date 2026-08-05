extern crate rustc_abi;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_abi::ExternAbi;
use rustc_errors::DiagDecorator;
use rustc_hir::{BindingMode, HirId, Item, ItemKind, Mod, Mutability, PatKind, def_id::LocalDefId};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty};
use rustc_span::{Span, Symbol, sym};

// -----------------------------------------------------------------------------
// CollectionReceiver: Collection receiver forms
// -----------------------------------------------------------------------------

/// The method receiver that preserves how the original collection was passed.
#[derive(Clone, Copy)]
enum CollectionReceiver {
    /// Collection is passed by value through an immutable binding.
    Owned,
    /// Collection is passed by value through a mutable binding.
    MutOwned,
    /// Collection is shared through an immutable reference.
    Shared,
    /// Collection is borrowed through a mutable reference.
    Mutable,
}

/// Supported collection receiver together with its direct element type.
struct CollectionReceiverType<'tcx> {
    /// Receiver syntax preserving the function parameter's ownership contract.
    receiver: CollectionReceiver,
    /// Semantic type stored directly in the collection.
    element: Ty<'tcx>,
}

impl CollectionReceiver {
    /// Finds supported vector and slice shapes and returns their direct element type.
    fn discover<'tcx>(
        cx: &LateContext<'tcx>,
        input: Ty<'tcx>,
        mutable_binding: bool,
    ) -> Option<CollectionReceiverType<'tcx>> {
        if let Some(element) = Self::vec_element_type(cx, input) {
            let receiver = if mutable_binding {
                Self::MutOwned
            } else {
                Self::Owned
            };
            return Some(CollectionReceiverType { receiver, element });
        }

        let ty::Ref(_, collection, mutability) = input.kind() else {
            return None;
        };
        let element = Self::vec_element_type(cx, *collection).or_else(|| {
            let ty::Slice(element) = collection.kind() else {
                return None;
            };
            Some(*element)
        })?;
        let receiver = match mutability {
            Mutability::Not => Self::Shared,
            Mutability::Mut => Self::Mutable,
        };
        Some(CollectionReceiverType { receiver, element })
    }

    /// Returns the element type when a type is the standard library's `Vec`.
    fn vec_element_type<'tcx>(cx: &LateContext<'tcx>, vector: Ty<'tcx>) -> Option<Ty<'tcx>> {
        let ty::Adt(vector_def, arguments) = vector.kind() else {
            return None;
        };
        cx.tcx
            .is_diagnostic_item(sym::Vec, vector_def.did())
            .then(|| arguments.type_at(0))
    }

    /// Describes the receiver agents should use to retain the original ownership contract.
    const fn description(self) -> &'static str {
        match self {
            Self::Owned => "`self`",
            Self::MutOwned => "`mut self`",
            Self::Shared => "`&self`",
            Self::Mutable => "`&mut self`",
        }
    }
}

// -----------------------------------------------------------------------------
// Candidate: Candidate discovery and wrapper state
// -----------------------------------------------------------------------------

/// Whether the canonical wrapper can receive the misplaced function.
enum CandidateWrapperState {
    /// No item occupies the canonical wrapper name.
    Missing,
    /// Existing wrapper has the canonical named vector field.
    Compatible,
    /// The canonical name exists but does not represent the expected wrapper.
    Conflicting,
}

/// One free function whose first parameter represents a collection that needs a domain wrapper.
struct Candidate {
    /// Function HIR node on which the diagnostic is emitted.
    hir_id: HirId,
    /// Free-function name proposed for the wrapper method.
    function_name: Symbol,
    /// Identifier span used as the primary diagnostic location.
    function_name_span: Span,
    /// First-parameter span describing the collection contract.
    parameter_span: Span,
    /// Local definition of the collection element struct.
    element_def_id: LocalDefId,
    /// Whether the element type requires generic wrapper design decisions.
    has_element_parameters: bool,
    /// Displayable semantic element type including generic arguments.
    element_type: String,
    /// Element struct name shown in diagnostics.
    element_name: Symbol,
    /// Canonical `<Element>List` wrapper name.
    wrapper_name: Symbol,
    /// Method receiver preserving collection ownership and mutability.
    receiver: CollectionReceiver,
}

impl Candidate {
    /// Recognizes a free function whose first parameter is a supported collection of a local struct.
    ///
    /// The compiler's understanding of the type is used here, so aliases such as `type Items =
    /// Vec<Item>` are treated the same as spelling `Vec<Item>` directly.
    fn discover(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        let ItemKind::Fn {
            sig,
            ident,
            body,
            has_body: true,
            ..
        } = item.kind
        else {
            return None;
        };

        if sig.header.abi != ExternAbi::Rust {
            return None;
        }

        let function_def_id = item.owner_id.def_id;
        let signature = cx.tcx.fn_sig(function_def_id).instantiate_identity();
        let first_type = *signature.inputs().skip_binder().first()?;
        let body = cx.tcx.hir_body(body);
        let parameter = body.params.first()?;
        let mutable_binding = matches!(
            parameter.pat.kind,
            PatKind::Binding(BindingMode(_, Mutability::Mut), ..)
        );
        let receiver_type = CollectionReceiver::discover(cx, first_type, mutable_binding)?;

        let ty::Adt(element, _) = receiver_type.element.kind() else {
            return None;
        };
        let element_def_id = element
            .is_struct()
            .then(|| element.did().as_local())
            .flatten()?;
        if cx
            .tcx
            .def_span(element_def_id)
            .in_external_macro(cx.sess().source_map())
        {
            return None;
        }

        let element_name = cx.tcx.item_name(element_def_id.to_def_id());
        let wrapper_name = Symbol::intern(&format!("{element_name}List"));

        // Retain the semantic receiver and names needed for later wrapper guidance.
        Some(Self {
            hir_id: item.hir_id(),
            function_name: ident.name,
            function_name_span: ident.span,
            parameter_span: parameter.span,
            element_def_id,
            has_element_parameters: !cx.tcx.generics_of(element_def_id).own_params.is_empty(),
            element_type: receiver_type.element.to_string(),
            element_name,
            wrapper_name,
            receiver: receiver_type.receiver,
        })
    }

    /// Returns the local struct stored directly in a `Vec`, ignoring its generic arguments.
    fn vector_element(cx: &LateContext<'_>, vector: Ty<'_>) -> Option<LocalDefId> {
        let ty::Adt(vector_def, arguments) = vector.kind() else {
            return None;
        };
        if !cx.tcx.is_diagnostic_item(sym::Vec, vector_def.did()) {
            return None;
        }
        let ty::Adt(element, _) = arguments.type_at(0).kind() else {
            return None;
        };
        element
            .is_struct()
            .then(|| element.did().as_local())
            .flatten()
    }

    /// Returns whether an item occupies the canonical name in Rust's type namespace.
    ///
    /// Functions and values may legally share this spelling with a struct, so they must not be
    /// mistaken for wrapper conflicts.
    fn occupies_wrapper_name(&self, item: &Item<'_>) -> bool {
        if item
            .kind
            .ident()
            .is_none_or(|ident| ident.name != self.wrapper_name)
        {
            return false;
        }

        match item.kind {
            ItemKind::ExternCrate(..)
            | ItemKind::Mod(..)
            | ItemKind::TyAlias(..)
            | ItemKind::Enum(..)
            | ItemKind::Struct(..)
            | ItemKind::Union(..)
            | ItemKind::Trait(..)
            | ItemKind::TraitAlias(..) => true,
            ItemKind::Use(path, _) => path.res.type_ns.is_some(),
            _ => false,
        }
    }

    /// Determines whether the canonical wrapper is absent, usable, or occupied by another type.
    fn wrapper_state<'tcx>(
        &self,
        cx: &LateContext<'tcx>,
        module_items: &[&'tcx Item<'tcx>],
    ) -> CandidateWrapperState {
        let Some(item) = module_items
            .iter()
            .find(|item| self.occupies_wrapper_name(item))
        else {
            return CandidateWrapperState::Missing;
        };

        let ItemKind::Struct(_, _, fields) = item.kind else {
            return CandidateWrapperState::Conflicting;
        };

        // Confirm that the existing wrapper stores the expected element vector in `items`.
        let items = Symbol::intern("items");
        let has_expected_field = fields.fields().iter().any(|field| {
            field.ident.name == items
                && Self::vector_element(cx, cx.tcx.type_of(field.def_id).instantiate_identity())
                    .is_some_and(|element| element == self.element_def_id)
        });
        if has_expected_field {
            CandidateWrapperState::Compatible
        } else {
            CandidateWrapperState::Conflicting
        }
    }

    /// Returns editable items from the element struct's module, where its wrapper must live.
    fn element_module_items<'tcx>(&self, cx: &LateContext<'tcx>) -> Vec<&'tcx Item<'tcx>> {
        let source_map = cx.sess().source_map();
        let module = cx.tcx.parent_module_from_def_id(self.element_def_id);
        let module_items = cx.tcx.hir_module_items(module);

        // Resolve free items and discard code produced by external macros.
        let authored_items = module_items
            .free_items()
            .map(|item_id| cx.tcx.hir_item(item_id));
        authored_items
            .filter(|item| !item.span.in_external_macro(source_map))
            .collect()
    }

    /// Reports the violation and gives a concrete wrapper recipe without pretending it is a safe
    /// automatic rewrite.
    fn emit(&self, cx: &LateContext<'_>) {
        // Resolve wrapper compatibility and the element's owning source file.
        let module_items = self.element_module_items(cx);
        let wrapper = self.wrapper_state(cx, &module_items);
        let source_map = cx.sess().source_map();
        let element_file = source_map.span_to_filename(cx.tcx.def_span(self.element_def_id));
        let element_file = element_file.short();

        // Tailor one wrapper recipe to the namespace state discovered above.
        cx.tcx.emit_node_span_lint(
            COLLECTION_METHOD_LIKE_FREE_FUNCTIONS,
            self.hir_id,
            self.function_name_span,
            DiagDecorator(|diag| {
                diag.primary_message(format!(
                    "free function `{}` should be a method on `{}`",
                    self.function_name, self.wrapper_name
                ));
                diag.span_label(
                    self.parameter_span,
                    format!("this collection of `{}` needs a named wrapper", self.element_name),
                );

                match wrapper {
                    CandidateWrapperState::Missing if self.has_element_parameters => diag.help(format!(
                        "beside `{}` in `{element_file}`, define a generic `{}` wrapper that preserves its parameters and stores `Vec<{}>` in `items`, then implement `{}` there with {}",
                        self.element_name,
                        self.wrapper_name,
                        self.element_type,
                        self.function_name,
                        self.receiver.description()
                    )),
                    CandidateWrapperState::Missing => diag.help(format!(
                        "beside `{}` in `{element_file}`, define `struct {} {{ items: Vec<{}> }}` and implement `{}` there with {}",
                        self.element_name,
                        self.wrapper_name,
                        self.element_type,
                        self.function_name,
                        self.receiver.description()
                    )),
                    CandidateWrapperState::Compatible => diag.help(format!(
                        "move `{}` into the existing `impl {}` block beside `{}` in `{element_file}` and use {}",
                        self.function_name,
                        self.wrapper_name,
                        self.element_name,
                        self.receiver.description()
                    )),
                    CandidateWrapperState::Conflicting => diag.help(format!(
                        "`{}` already names another type beside `{}` in `{element_file}`; choose a dedicated wrapper there with an `items: Vec<{}>` field and implement `{}` with {}",
                        self.wrapper_name,
                        self.element_name,
                        self.element_name,
                        self.function_name,
                        self.receiver.description()
                    )),
                };
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// CollectionMethodLikeFreeFunctions: Lint pass
// -----------------------------------------------------------------------------

/// Late lint pass that relocates collection-shaped free functions to domain wrappers.
struct CollectionMethodLikeFreeFunctions;

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Checks for free functions whose first parameter is a vector or slice of a struct defined in
    /// the same crate.
    ///
    /// ### Why is this bad?
    ///
    /// A collection of domain values usually has behavior of its own. Giving that collection a
    /// name keeps its behavior discoverable and prevents unrelated free functions from becoming
    /// the collection's informal interface.
    ///
    /// For example, this function leaves the collection without a home for its behavior:
    ///
    /// ```rust
    /// struct Item;
    ///
    /// fn inspect(items: &[Item]) {}
    /// ```
    ///
    /// A small wrapper makes the intended interface explicit:
    ///
    /// ```rust
    /// struct ItemList {
    ///     items: Vec<Item>,
    /// }
    ///
    /// impl ItemList {
    ///     fn inspect(&self) {}
    /// }
    /// ```
    pub COLLECTION_METHOD_LIKE_FREE_FUNCTIONS,
    Warn,
    "enforces wrapper methods for vectors and slices of local structs",
    CollectionMethodLikeFreeFunctions
}

impl<'tcx> LateLintPass<'tcx> for CollectionMethodLikeFreeFunctions {
    /// Checks complete modules so wrapper discovery and function discovery see the same namespace.
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, _: HirId) {
        let source_map = cx.sess().source_map();

        // Resolve authored items once so every candidate sees the same namespace.
        let resolved = module
            .item_ids
            .iter()
            .map(|item_id| cx.tcx.hir_item(*item_id));
        let items = resolved
            .filter(|item| !item.span.in_external_macro(source_map))
            .collect::<Vec<_>>();

        for item in &items {
            let Some(candidate) = Candidate::discover(cx, item) else {
                continue;
            };
            candidate.emit(cx);
        }
    }
}
