extern crate rustc_abi;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_abi::ExternAbi;
use rustc_hir::def::DefKind;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, HirId, PatKind, TraitFn, TraitItem, TraitItemKind};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::Ty;
use rustc_span::def_id::LocalDefId;
use rustc_span::symbol::kw;
use rustc_span::{Span, Symbol};

use super::parameter_kind::{ParameterKind, ParameterTypeExt};
use super::{identifier_case, parameter_role};

/// Smallest family that can contain interchangeable parameter roles.
const MIN_AMBIGUOUS_PARAMETER_COUNT: usize = 2;

// -----------------------------------------------------------------------------
// Parameter: Semantic function parameter analysis
// -----------------------------------------------------------------------------
/// One simple authored parameter from a function-like declaration.
#[derive(Clone)]
pub struct Parameter {
    /// HIR identity used to recognize binding references.
    pub(super) hir_id: HirId,
    /// Identifier source range used for labels.
    pub(crate) span: Span,
    /// Authored binding name.
    pub(crate) name: Symbol,
    /// Whether the resolved parameter type is exactly `bool`.
    is_boolean: bool,
    /// Interchangeable primitive family, excluding booleans.
    pub(super) interchangeable: Option<ParameterKind>,
}

/// One reportable family of interchangeable parameters.
pub struct ParameterGroup<'signature> {
    /// Shared semantic representation.
    pub(crate) kind: ParameterKind,
    /// Parameters carrying distinct roles through names alone.
    pub(crate) parameters: Vec<&'signature Parameter>,
}
/// One authored function or method signature with simple named parameters.
#[derive(Clone)]
pub struct ParameterSignature {
    /// Definition identity used for semantic type queries.
    pub(crate) def_id: LocalDefId,
    /// HIR identity on which function-level diagnostics are emitted.
    pub(crate) hir_id: HirId,
    /// Authored function or method name.
    pub(super) name: Symbol,
    /// Name source range used as the primary diagnostic site.
    pub(super) span: Span,
    /// Direct parameters excluding `self`.
    pub(super) parameters: Vec<Parameter>,
}

impl ParameterSignature {
    /// Collects a function or provided method under an authored Rust API boundary.
    pub(crate) fn from_body(
        cx: &LateContext<'_>,
        kind: FnKind<'_>,
        body: &Body<'_>,
        def_id: LocalDefId,
    ) -> Option<Self> {
        // Resolve an ordinary Rust function name before inspecting its parameters.
        let name = match kind {
            FnKind::ItemFn(ident, _, header) if header.abi == ExternAbi::Rust => ident,
            FnKind::Method(ident, signature) if signature.header.abi == ExternAbi::Rust => ident,
            _ => return None,
        };

        // Exclude repeated trait implementations and source owned by external generators.
        if Self::belongs_to_trait_impl(cx, def_id)
            || name.span.in_external_macro(cx.sess().source_map())
            || !identifier_case::is_snake(name.name.as_str())
            || Self::is_framework_generated(cx, name, def_id)
        {
            return None;
        }

        // Resolve semantic inputs and retain only simple named authored parameters.
        let types = Self::input_types(cx, def_id);
        let parameters = Self::body_parameters(cx, body, types);

        // Preserve the complete signature identity for later crate-wide analysis.
        Some(Self {
            def_id,
            hir_id: cx.tcx.local_def_id_to_hir_id(def_id),
            name: name.name,
            span: name.span,
            parameters,
        })
    }

    /// Collects a required local trait method, which has names but no HIR body.
    pub(crate) fn from_required_trait(cx: &LateContext<'_>, item: &TraitItem<'_>) -> Option<Self> {
        // Require an authored required method using the ordinary Rust ABI.
        let TraitItemKind::Fn(signature, TraitFn::Required(names)) = item.kind else {
            return None;
        };
        if signature.header.abi != ExternAbi::Rust
            || item.span.in_external_macro(cx.sess().source_map())
        {
            return None;
        }

        // Resolve semantic inputs and retain available authored parameter names.
        let def_id = item.owner_id.def_id;
        let types = Self::input_types(cx, def_id);
        let parameters = Self::required_parameters(cx, item, names, types);

        // Preserve the declaration as the one diagnostic owner for every implementation.
        Some(Self {
            def_id,
            hir_id: item.hir_id(),
            name: item.ident.name,
            span: item.ident.span,
            parameters,
        })
    }

    /// Returns whether a callable boundary is framework glue rather than authored API syntax.
    fn is_framework_generated(
        cx: &LateContext<'_>,
        name: rustc_span::Ident,
        def_id: LocalDefId,
    ) -> bool {
        // Prefer semantic generated names that remain stable across source remapping.
        let semantic_name = cx.tcx.item_name(def_id.to_def_id());
        if name.name.as_str().starts_with("__component_")
            || semantic_name.as_str().starts_with("__component_")
        {
            return true;
        }

        // Attribute macros may preserve PascalCase call-site syntax on generated functions.
        let source_map = cx.sess().source_map();
        let pascal_call_site = source_map
            .span_to_snippet(name.span)
            .is_ok_and(|source| identifier_case::is_pascal(source.trim()));

        // Retain the authored component attribute as independent generation evidence.
        let component_definition = source_map
            .span_to_snippet(cx.tcx.def_span(def_id))
            .is_ok_and(|source| source.contains("#[component]"));
        pascal_call_site || component_definition
    }

    /// Resolves instantiated function inputs without late-bound binder syntax.
    fn input_types<'tcx>(cx: &LateContext<'tcx>, def_id: LocalDefId) -> Vec<Ty<'tcx>> {
        let signature = cx.tcx.fn_sig(def_id).instantiate_identity();
        let signature = signature.skip_binder();
        signature.inputs().to_vec()
    }

    /// Returns whether this definition is a method inside a trait implementation.
    fn belongs_to_trait_impl(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
        cx.tcx.opt_local_parent(def_id).is_some_and(|parent| {
            matches!(cx.tcx.def_kind(parent), DefKind::Impl { of_trait: true })
        })
    }

    /// Builds one parameter record from its resolved type.
    fn parameter(
        cx: &LateContext<'_>,
        hir_id: HirId,
        span: Span,
        name: Symbol,
        ty: Ty<'_>,
    ) -> Parameter {
        Parameter {
            hir_id,
            span,
            name,
            is_boolean: ty.is_bool(),
            interchangeable: ty.interchangeable_kind(cx),
        }
    }

    /// Collects simple bindings from a function body in source order.
    fn body_parameters<'tcx>(
        cx: &LateContext<'tcx>,
        body: &Body<'_>,
        types: Vec<Ty<'tcx>>,
    ) -> Vec<Parameter> {
        let mut parameters = Vec::new();
        for (parameter, ty) in body.params.iter().zip(types) {
            let PatKind::Binding(_, hir_id, ident, None) = parameter.pat.kind else {
                continue;
            };
            if ident.name == kw::SelfLower {
                continue;
            }
            parameters.push(Self::parameter(cx, hir_id, ident.span, ident.name, ty));
        }
        parameters
    }

    /// Collects available names from a required trait method declaration.
    fn required_parameters<'tcx>(
        cx: &LateContext<'tcx>,
        item: &TraitItem<'_>,
        names: &[Option<rustc_span::Ident>],
        types: Vec<Ty<'tcx>>,
    ) -> Vec<Parameter> {
        let mut parameters = Vec::new();
        for (name, ty) in names.iter().zip(types) {
            // Ignore omitted names and the receiver before recording authored bindings.
            let Some(ident) = name else { continue };
            if ident.name == kw::SelfLower {
                continue;
            }

            // Pair each surviving authored name with its resolved semantic type.
            parameters.push(Self::parameter(
                cx,
                item.hir_id(),
                ident.span,
                ident.name,
                ty,
            ));
        }
        parameters
    }

    /// Recognizes conventional same-domain roles that need no distinct newtypes.
    fn roles_are_conventional(
        function: Symbol,
        kind: ParameterKind,
        parameters: &[&Parameter],
    ) -> bool {
        let mut names = parameters
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect::<Vec<_>>();
        names.sort_unstable();
        parameter_role::parameter_role_names_are_conventional(function.as_str(), kind, &names)
    }

    /// Requires precise authored names before inferring distinct domains.
    fn roles_are_distinct(parameters: &[&Parameter]) -> bool {
        let mut unique_names = HashSet::new();
        for parameter in parameters {
            let name = parameter.name.as_str();
            if parameter_role::parameter_role_is_weak(name) {
                return false;
            }
            unique_names.insert(name);
        }
        unique_names.len() >= MIN_AMBIGUOUS_PARAMETER_COUNT
    }

    /// Returns the number of direct authored parameters excluding `self`.
    #[cfg(feature = "bon")]
    pub(crate) const fn parameter_count(&self) -> usize {
        self.parameters.len()
    }

    /// Returns the authored function or method name span.
    #[cfg(feature = "bon")]
    pub(crate) const fn name_span(&self) -> Span {
        self.span
    }

    /// Returns direct boolean parameters in source order.
    pub(crate) fn boolean_parameters(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|parameter| parameter.is_boolean)
            .collect()
    }

    /// Groups interchangeable non-boolean primitive parameters by representation.
    pub(crate) fn ambiguous_groups(&self) -> Vec<ParameterGroup<'_>> {
        let mut grouped = HashMap::<ParameterKind, Vec<&Parameter>>::new();
        for parameter in &self.parameters {
            let Some(kind) = parameter.interchangeable else {
                continue;
            };
            grouped.entry(kind).or_default().push(parameter);
        }

        // Retain only precise, nonconventional role families.
        let mut groups = Vec::new();
        for (kind, parameters) in grouped {
            if parameters.len() < MIN_AMBIGUOUS_PARAMETER_COUNT
                || !Self::roles_are_distinct(&parameters)
                || Self::roles_are_conventional(self.name, kind, &parameters)
            {
                continue;
            }
            groups.push(ParameterGroup { kind, parameters });
        }
        groups.sort_by_key(|group| group.parameters[0].span.lo());
        groups
    }

    /// Returns whether one boolean parameter forms `set_property(property)` exactly.
    pub(crate) fn has_exact_boolean_setter(&self) -> bool {
        let booleans = self.boolean_parameters();
        if booleans.len() != 1 {
            return false;
        }
        self.name
            .as_str()
            .strip_prefix("set_")
            .is_some_and(|property| property == booleans[0].name.as_str())
    }
}
