extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use convert_case::{Case, Casing};
use rustc_hir::{HirId, Item, ItemKind, PatKind};
use rustc_lint::LateContext;
use rustc_middle::ty::{self, FieldDef, GenericArgsRef, Ty};
use rustc_span::def_id::DefId;
use rustc_span::{Span, Symbol};

// -----------------------------------------------------------------------------
// Component: Leptos component property analysis
// -----------------------------------------------------------------------------

/// One authored component property with its resolved semantic type.
pub struct ComponentProp<'tcx> {
    /// Component function node used to honor lint levels written on `#[component]`.
    pub owner: HirId,
    /// Authored component span used to recover property-level source annotations.
    pub owner_span: Span,
    /// Authored parameter binding used to analyze how the capability flows through the component.
    pub binding: HirId,
    /// Authored property name.
    pub name: Symbol,
    /// Property identifier mapped back through the component macro.
    pub span: Span,
    /// Property type after aliases and component generics have been instantiated.
    pub ty: Ty<'tcx>,
}

/// Semantic analysis shared by lints governing Leptos component properties.
pub struct ComponentProps;

impl ComponentProps {
    /// Returns the props represented by a generated implementation of Leptos's `Props` trait.
    pub fn from_impl<'tcx>(
        cx: &LateContext<'tcx>,
        item: &Item<'tcx>,
    ) -> Option<Vec<ComponentProp<'tcx>>> {
        // Resolve only implementations of the semantic `Leptos::Props` contract.
        let ItemKind::Impl(_) = item.kind else {
            return None;
        };

        // Verify the implemented trait by its resolved crate and item identities.
        let trait_ref = cx
            .tcx
            .impl_opt_trait_ref(item.owner_id.def_id)?
            .instantiate_identity();
        if cx.tcx.crate_name(trait_ref.def_id.krate).as_str() != "leptos"
            || cx.tcx.item_name(trait_ref.def_id).as_str() != "Props"
        {
            return None;
        }

        // Recover the generated props type and its concrete field arguments.
        let self_ty = cx.tcx.type_of(item.owner_id.def_id).instantiate_identity();
        let ty::Adt(definition, arguments) = self_ty.kind() else {
            return None;
        };

        // Match the generated type to the authored component function.
        let component_name = cx
            .tcx
            .item_name(definition.did())
            .as_str()
            .strip_suffix("Props")?
            .to_owned();

        // Resolve the authored function body and align generated fields with its parameters.
        let owner = Self::component_owner(cx, definition.did(), component_name.as_str())?;
        let ItemKind::Fn { body, .. } = owner.kind else {
            return None;
        };
        let parameters = cx.tcx.hir_body(body).params;
        if definition.all_fields().count() != parameters.len() {
            return None;
        }

        // Preserve every authored prop name, span, and resolved semantic type.
        let mut properties = Vec::with_capacity(parameters.len());
        for (field, parameter) in definition.all_fields().zip(parameters) {
            // Require direct authored bindings so generated fields cannot be misaligned.
            let PatKind::Binding(_, binding, _, None) = parameter.pat.kind else {
                return None;
            };

            properties.push(Self::property(cx, owner, field, arguments, binding));
        }
        Some(properties)
    }

    /// Returns whether a prop directly or through an accepted wrapper carries boolean state.
    pub fn carries_boolean(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
        // Accept a direct boolean before attempting to inspect wrapper identity.
        if ty.is_bool() {
            return true;
        }
        let ty::Adt(definition, arguments) = ty.kind() else {
            return false;
        };

        // Descend only through wrappers whose first argument is represented state.
        if !Self::is_boolean_wrapper(cx, definition.did()) {
            return false;
        }
        arguments
            .types()
            .next()
            .is_some_and(|inner| Self::carries_boolean(cx, inner))
    }

    /// Builds one property after generated-to-authored field alignment has been proven.
    fn property<'tcx>(
        cx: &LateContext<'tcx>,
        owner: &Item<'tcx>,
        field: &FieldDef,
        arguments: GenericArgsRef<'tcx>,
        binding: HirId,
    ) -> ComponentProp<'tcx> {
        // Prefer the authored identifier span while retaining generated-span fallback.
        let span = cx
            .tcx
            .def_ident_span(field.did)
            .unwrap_or_else(|| cx.tcx.def_span(field.did));

        // Pair authored identity with the generated field's resolved semantic type.
        let ty = field.ty(cx.tcx, arguments);

        // Assemble the stable property record consumed by policy lints.
        ComponentProp {
            owner: owner.hir_id(),
            owner_span: owner.span,
            binding,

            // Retain authored-facing metadata beside semantic type evidence.
            name: field.name,
            span,
            ty,
        }
    }

    /// Finds the generated body function that retains attributes from the authored component.
    fn component_owner<'tcx>(
        cx: &LateContext<'tcx>,
        props_definition: DefId,
        component_name: &str,
    ) -> Option<&'tcx Item<'tcx>> {
        let body_name = format!("__component_{}", component_name.to_case(Case::Snake));
        let module = cx
            .tcx
            .parent_module_from_def_id(props_definition.as_local()?)
            .to_local_def_id();
        cx.tcx.hir_free_items().find_map(|item_id| {
            let item = cx.tcx.hir_item(item_id);
            let ItemKind::Fn { .. } = item.kind else {
                return None;
            };
            (cx.tcx
                .parent_module_from_def_id(item.owner_id.def_id)
                .to_local_def_id()
                == module
                && item.kind.ident()?.name.as_str() == body_name)
                .then_some(item)
        })
    }

    /// Recognizes only wrappers whose first type argument is the represented value.
    fn is_boolean_wrapper(cx: &LateContext<'_>, def_id: DefId) -> bool {
        let crate_symbol = cx.tcx.crate_name(def_id.krate);
        let item_symbol = cx.tcx.item_name(def_id);
        let crate_name = crate_symbol.as_str();
        let item_name = item_symbol.as_str();
        (crate_name == "core" && item_name == "Option")
            || (crate_name == "reactive_graph"
                && ["Signal", "ReadSignal", "RwSignal", "MaybeSignal"].contains(&item_name))
    }
}
