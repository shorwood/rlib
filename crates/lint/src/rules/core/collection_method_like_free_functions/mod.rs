extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::def_id::LocalDefId;
use rustc_hir::{BindingMode, HirId, Item, ItemKind, Mod, Mutability, PatKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty::{self, Ty, TypeVisitableExt};
use rustc_span::{Span, Symbol, sym};

use crate::utils::diagnostic::LateViolation;
use crate::utils::free_function_analysis::FreeFunctionExt;

// -----------------------------------------------------------------------------
// Collection: Collection receiver forms
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
/// Mutability of an owned collection's parameter binding.
#[derive(Clone, Copy)]
enum CollectionBindingMutability {
    /// The binding is immutable.
    Immutable,
    /// The binding is explicitly mutable.
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
        binding_mutability: CollectionBindingMutability,
    ) -> Option<CollectionReceiverType<'tcx>> {
        // Prefer an owned vector receiver while preserving binding mutability.
        if let Some(element) = Self::vec_element_type(cx, input) {
            let receiver = if matches!(binding_mutability, CollectionBindingMutability::Mutable) {
                Self::MutOwned
            } else {
                Self::Owned
            };
            return Some(CollectionReceiverType { receiver, element });
        }

        // Resolve borrowed vectors and slices to their direct element type.
        let ty::Ref(_, collection, mutability) = input.kind() else {
            return None;
        };
        let element =
            Self::vec_element_type(cx, *collection).or_else(|| match collection.kind() {
                ty::Slice(element) => Some(*element),
                _ => None,
            })?;

        // Preserve the reference mutability in the eventual method receiver.
        let receiver = match mutability {
            Mutability::Not => Self::Shared,
            Mutability::Mut => Self::Mutable,
        };
        Some(CollectionReceiverType { receiver, element })
    }

    /// Returns the element type when a type is the standard library's `Vec`.
    fn vec_element_type<'tcx>(cx: &LateContext<'tcx>, vector: Ty<'tcx>) -> Option<Ty<'tcx>> {
        // Non-ADT types cannot resolve to the standard vector diagnostic item.
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
#[derive(Clone, Copy)]
enum CandidateWrapperState {
    /// No item occupies the canonical wrapper name.
    Missing,
    /// Existing wrapper has the canonical named vector field.
    Compatible,
    /// The canonical name exists but does not represent the expected wrapper.
    Conflicting,
}

/// Free-function identity and source locations retained for wrapper guidance.
struct CandidateFunction {
    /// Function HIR node on which the diagnostic is emitted.
    hir_id: HirId,
    /// Free-function name proposed for the wrapper method.
    name: Symbol,
    /// Identifier span used as the primary diagnostic location.
    name_span: Span,
    /// First-parameter span describing the collection contract.
    parameter_span: Span,
}

/// Local collection element identity and display names retained for wrapper guidance.
struct CandidateElement<'tcx> {
    /// Local definition of the collection element struct.
    def_id: LocalDefId,
    /// Whether the element type requires generic wrapper design decisions.
    has_parameters: bool,
    /// Displayable semantic element type including generic arguments.
    ty: Ty<'tcx>,
    /// Element struct name shown in diagnostics.
    name: Symbol,
}

/// One free function whose first parameter represents a collection that needs a domain wrapper.
struct Candidate<'tcx> {
    /// Free-function identity and source locations.
    function: CandidateFunction,
    /// Local collection element identity and display names.
    element: CandidateElement<'tcx>,
    /// Canonical `<Element>List` wrapper name.
    wrapper_name: Symbol,
    /// Method receiver preserving collection ownership and mutability.
    receiver: CollectionReceiver,
}

impl<'tcx> Candidate<'tcx> {
    /// Recognizes a free function whose first parameter is a supported collection of a local
    /// struct.
    ///
    /// The compiler's understanding of the type is used here, so aliases such as `type Items =
    /// Vec<Item>` are treated the same as spelling `Vec<Item>` directly.
    fn discover(cx: &LateContext<'tcx>, item: &Item<'tcx>) -> Option<Self> {
        // Require an authored free function body before extracting its syntax.
        if !item.is_authored_rust_free_function(cx) {
            return None;
        }

        // Extract the signature and body from the validated function.
        let ItemKind::Fn { body, .. } = item.kind else {
            return None;
        };
        let ident = item.kind.ident()?;

        // Resolve the first parameter's collection receiver and element semantics.
        let function_def_id = item.owner_id.def_id;
        let signature = cx.tcx.fn_sig(function_def_id).instantiate_identity();
        let first_type = *signature.inputs().skip_binder().first()?;
        let body = cx.tcx.hir_body(body);
        let parameter = body.params.first()?;

        // Preserve mutable owned bindings when classifying the receiver contract.
        let binding_mutability = match parameter.pat.kind {
            PatKind::Binding(BindingMode(_, Mutability::Mut), ..) => {
                CollectionBindingMutability::Mutable
            }
            _ => CollectionBindingMutability::Immutable,
        };
        let receiver_type = CollectionReceiver::discover(cx, first_type, binding_mutability)?;

        // Require a directly stored local struct element under source control.
        let ty::Adt(element, _) = receiver_type.element.kind() else {
            return None;
        };
        let element_def_id = element
            .is_struct()
            .then(|| element.did().as_local())
            .flatten()?;

        // Reject element definitions generated outside the linted crate's source.
        if cx
            .tcx
            .def_span(element_def_id)
            .in_external_macro(cx.sess().source_map())
        {
            return None;
        }

        // Derive canonical element and wrapper names for concrete guidance.
        let element_name = cx.tcx.item_name(element_def_id.to_def_id());
        let wrapper_name = Symbol::intern(&format!("{element_name}List"));

        // Group the function identity independently from the element type identity.
        let function = CandidateFunction {
            hir_id: item.hir_id(),
            name: ident.name,
            name_span: ident.span,
            parameter_span: parameter.span,
        };

        // Retain element facts needed for wrapper discovery and diagnostic recipes.
        let element = CandidateElement {
            def_id: element_def_id,
            has_parameters: !cx.tcx.generics_of(element_def_id).own_params.is_empty(),
            ty: receiver_type.element,
            name: element_name,
        };

        // Combine the grouped facts with the canonical wrapper receiver.
        Some(Self {
            function,
            element,
            wrapper_name,
            receiver: receiver_type.receiver,
        })
    }

    /// Returns the local struct stored directly in a `Vec`, ignoring its generic arguments.
    fn vector_element(cx: &LateContext<'tcx>, vector: Ty<'tcx>) -> Option<Ty<'tcx>> {
        // Require the standard vector diagnostic item.
        let ty::Adt(vector_def, arguments) = vector.kind() else {
            return None;
        };

        // An arbitrary ADT does not establish the vector ownership contract this lint models.
        if !cx.tcx.is_diagnostic_item(sym::Vec, vector_def.did()) {
            return None;
        }

        // Resolve a directly stored local struct element.
        let ty::Adt(element, _) = arguments.type_at(0).kind() else {
            return None;
        };
        (element.is_struct() && element.did().is_local()).then_some(arguments.type_at(0))
    }

    /// Compares generic argument lists while preserving parameter-position equivalence.
    fn same_generic_arguments<'analysis>(
        expected: ty::GenericArgsRef<'analysis>,
        actual: ty::GenericArgsRef<'analysis>,
    ) -> bool {
        expected.len() == actual.len()
            && expected
                .iter()
                .zip(actual.iter())
                .all(
                    |(expected, actual)| match (expected.as_type(), actual.as_type()) {
                        (Some(expected), Some(actual)) => Self::same_type_shape(expected, actual),
                        (Some(_), None) | (None, Some(_)) => false,
                        (None, None) => match (expected.as_const(), actual.as_const()) {
                            (Some(expected), Some(actual)) => {
                                match (expected.kind(), actual.kind()) {
                                    (
                                        ty::ConstKind::Param(expected),
                                        ty::ConstKind::Param(actual),
                                    ) => expected.index == actual.index,
                                    _ => expected == actual,
                                }
                            }
                            (Some(_), None) | (None, Some(_)) => false,
                            (None, None) => {
                                expected == actual || (expected.has_param() && actual.has_param())
                            }
                        },
                    },
                )
    }

    /// Compares generic instantiations while treating same-position parameters as alpha-equivalent.
    fn same_type_shape<'analysis>(expected: Ty<'analysis>, actual: Ty<'analysis>) -> bool {
        match (expected.kind(), actual.kind()) {
            (ty::Param(expected), ty::Param(actual)) => expected.index == actual.index,
            (
                ty::Adt(expected_definition, expected_arguments),
                ty::Adt(actual_definition, actual_arguments),
            ) => {
                expected_definition.did() == actual_definition.did()
                    && Self::same_generic_arguments(expected_arguments, actual_arguments)
            }
            (ty::Ref(_, expected, expected_mutability), ty::Ref(_, actual, actual_mutability)) => {
                expected_mutability == actual_mutability
                    && Self::same_type_shape(*expected, *actual)
            }
            (ty::Slice(expected), ty::Slice(actual)) => Self::same_type_shape(*expected, *actual),
            (ty::Tuple(expected), ty::Tuple(actual)) => {
                expected.len() == actual.len()
                    && expected
                        .iter()
                        .zip(actual.iter())
                        .all(|(expected, actual)| Self::same_type_shape(expected, actual))
            }
            _ => expected == actual,
        }
    }

    /// Returns whether an item occupies the canonical name in Rust's type namespace.
    ///
    /// Functions and values may legally share this spelling with a struct, so they must not be
    /// mistaken for wrapper conflicts.
    fn occupies_wrapper_name(&self, item: &Item<'_>) -> bool {
        // Require an item with the canonical wrapper spelling.
        if item
            .kind
            .ident()
            .is_none_or(|ident| ident.name != self.wrapper_name)
        {
            return false;
        }

        // Classify concrete declarations occupying the type namespace.
        let is_concrete = matches!(
            item.kind,
            ItemKind::Mod(..) | ItemKind::Enum(..) | ItemKind::Struct(..) | ItemKind::Union(..)
        );

        // Classify abstract declarations and imports occupying the type namespace.
        let is_abstract = matches!(
            item.kind,
            ItemKind::ExternCrate(..)
                | ItemKind::TyAlias(..)
                | ItemKind::Trait(..)
                | ItemKind::TraitAlias(..)
        );

        // Include imports that resolve a name in the type namespace.
        let is_type_import =
            matches!(item.kind, ItemKind::Use(path, _) if path.res.type_ns.is_some());
        is_concrete || is_abstract || is_type_import
    }

    /// Determines whether the canonical wrapper is absent, usable, or occupied by another type.
    fn wrapper_state(
        &self,
        cx: &LateContext<'tcx>,
        module_items: &[&'tcx Item<'tcx>],
    ) -> CandidateWrapperState {
        // Resolve any declaration occupying the canonical wrapper name.
        let Some(item) = module_items
            .iter()
            .find(|item| self.occupies_wrapper_name(item))
        else {
            return CandidateWrapperState::Missing;
        };

        // A canonical name occupied by a non-struct cannot be reused as the collection wrapper.
        let ItemKind::Struct(_, _, fields) = item.kind else {
            return CandidateWrapperState::Conflicting;
        };

        // Confirm that the existing wrapper stores the expected element vector in `items`.
        let items = Symbol::intern("items");
        let has_expected_field = fields.fields().iter().any(|field| {
            field.ident.name == items
                && Self::vector_element(cx, cx.tcx.type_of(field.def_id).instantiate_identity())
                    .is_some_and(|element| Self::same_type_shape(self.element.ty, element))
        });
        if has_expected_field {
            CandidateWrapperState::Compatible
        } else {
            CandidateWrapperState::Conflicting
        }
    }

    /// Returns editable items from the element struct's module, where its wrapper must live.
    fn element_module_items(&self, cx: &LateContext<'tcx>) -> Vec<&'tcx Item<'tcx>> {
        // Resolve the element's owning module and its complete direct item set.
        let source_map = cx.sess().source_map();
        let module = cx.tcx.parent_module_from_def_id(self.element.def_id);
        let module_items = cx.tcx.hir_module_items(module);

        // Resolve free items and discard code produced by external macros.
        let authored_items = module_items
            .free_items()
            .map(|item_id| cx.tcx.hir_item(item_id));
        authored_items
            .filter(|item| !item.span.in_external_macro(source_map))
            .collect()
    }

    /// Describes a new generic wrapper beside a parameterized element type.
    fn generic_wrapper_remediation(&self, element_file: &str) -> String {
        format!(
            "beside `{}` in `{element_file}`, define a generic `{}` wrapper that preserves its parameters and stores `Vec<{}>` in `items`, then implement `{}` there with {}",
            self.element.name,
            self.wrapper_name,
            self.element.ty,
            self.function.name,
            self.receiver.description()
        )
    }

    /// Describes a new concrete wrapper beside a non-generic element type.
    fn concrete_wrapper_remediation(&self, element_file: &str) -> String {
        format!(
            "beside `{}` in `{element_file}`, define `struct {} {{ items: Vec<{}> }}` and implement `{}` there with {}",
            self.element.name,
            self.wrapper_name,
            self.element.ty,
            self.function.name,
            self.receiver.description()
        )
    }

    /// Describes moving the operation onto an existing compatible wrapper.
    fn compatible_wrapper_remediation(&self, element_file: &str) -> String {
        format!(
            "implement `{}` on the existing `{}` wrapper beside `{}` in `{element_file}` and use {}",
            self.function.name,
            self.wrapper_name,
            self.element.name,
            self.receiver.description()
        )
    }

    /// Describes choosing another wrapper when the canonical name is occupied incompatibly.
    fn conflicting_wrapper_remediation(&self, element_file: &str) -> String {
        format!(
            "`{}` already names another type beside `{}` in `{element_file}`; choose a dedicated wrapper there with an `items: Vec<{}>` field and implement `{}` with {}",
            self.wrapper_name,
            self.element.name,
            self.element.ty,
            self.function.name,
            self.receiver.description()
        )
    }

    /// Resolves a complete wrapper recipe without pretending it is a safe automatic rewrite.
    fn violation(&self, cx: &LateContext<'tcx>) -> Violation {
        // Resolve wrapper compatibility and the element's owning source file.
        let module_items = self.element_module_items(cx);
        let wrapper = self.wrapper_state(cx, &module_items);
        let source_map = cx.sess().source_map();
        let element_file = source_map.span_to_filename(cx.tcx.def_span(self.element.def_id));
        let element_file = element_file.short().to_string();

        // Tailor one wrapper recipe to the namespace state discovered above.
        let remediation_message = match wrapper {
            CandidateWrapperState::Missing if self.element.has_parameters => {
                self.generic_wrapper_remediation(&element_file)
            }
            CandidateWrapperState::Missing => self.concrete_wrapper_remediation(&element_file),
            CandidateWrapperState::Compatible => self.compatible_wrapper_remediation(&element_file),
            CandidateWrapperState::Conflicting => {
                self.conflicting_wrapper_remediation(&element_file)
            }
        };

        // Capture the three source anchors as one reusable location record.
        let source = ViolationSource {
            hir_id: self.function.hir_id,
            span: self.function.name_span,
            parameter_span: self.function.parameter_span,
        };

        // Combine the resolved source, domain names, and namespace-aware recipe.
        Violation {
            source,
            function_name: self.function.name,
            element_name: self.element.name,
            wrapper_name: self.wrapper_name,
            has_conflicting_wrapper: matches!(wrapper, CandidateWrapperState::Conflicting),
            remediation_message,
        }
    }
}

// -----------------------------------------------------------------------------
// Violation: Missing collection wrapper diagnostic
// -----------------------------------------------------------------------------

/// Source anchors retained for one collection-wrapper diagnostic.
struct ViolationSource {
    /// Function HIR node on which the diagnostic is emitted.
    hir_id: HirId,
    /// Function identifier span used as the primary diagnostic location.
    span: Span,
    /// Collection parameter span shown as supporting evidence.
    parameter_span: Span,
}

/// Complete collection-wrapper recommendation with resolved namespace and file context.
struct Violation {
    /// Source anchors required by the lint engine and supporting label.
    source: ViolationSource,
    /// Misplaced free-function name.
    function_name: Symbol,
    /// Local element struct name.
    element_name: Symbol,
    /// Canonical or proposed wrapper name.
    wrapper_name: Symbol,
    /// Whether that canonical name is already occupied by an incompatible type.
    has_conflicting_wrapper: bool,
    /// Namespace-aware wrapper construction or migration recipe.
    remediation_message: String,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        // A conflicting canonical name requires guidance that does not prescribe that name.
        if self.has_conflicting_wrapper {
            return Cow::Owned(format!(
                "free function `{}` needs a dedicated collection wrapper for `{}`",
                self.function_name, self.element_name
            ));
        }
        Cow::Owned(format!(
            "free function `{}` should be a method on `{}`",
            self.function_name, self.wrapper_name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "the collection of `{}` has domain behavior but no named type through which callers can discover or constrain that behavior",
            self.element_name
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(&self.remediation_message)
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.tcx.emit_node_span_lint(
            COLLECTION_METHOD_LIKE_FREE_FUNCTIONS,
            self.source.hir_id,
            self.source.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.span_label(
                    self.source.parameter_span,
                    format!(
                        "this collection of `{}` needs a named wrapper",
                        self.element_name
                    ),
                );
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// CollectionMethodLikeFreeFunctions: Domain collection ownership policy
// -----------------------------------------------------------------------------

/// Late lint pass that relocates collection-shaped free functions to domain wrappers.
struct CollectionMethodLikeFreeFunctions;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub COLLECTION_METHOD_LIKE_FREE_FUNCTIONS,
    Warn,
    "enforces wrapper methods for vectors and slices of local structs",
    CollectionMethodLikeFreeFunctions
}

impl<'tcx> LateLintPass<'tcx> for CollectionMethodLikeFreeFunctions {
    /// Checks complete modules so wrapper discovery and function discovery see the same namespace.
    fn check_mod(&mut self, cx: &LateContext<'tcx>, module: &'tcx Mod<'tcx>, _: HirId) {
        // Resolve the source map used to exclude externally generated declarations.
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
            candidate.violation(cx).emit(cx);
        }
    }
}
