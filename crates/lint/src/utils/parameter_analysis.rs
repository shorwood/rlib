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
use rustc_middle::ty::{self, Ty};
use rustc_span::def_id::LocalDefId;
use rustc_span::{Span, Symbol};

// -----------------------------------------------------------------------------
// Parameter: Semantic function parameter analysis
// -----------------------------------------------------------------------------

/// Primitive families whose values remain interchangeable at a call site.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ParameterKind {
    /// One signed integer representation.
    SignedInteger {
        /// Exact compiler-resolved signed integer type.
        representation: ty::IntTy,
    },
    /// One unsigned integer representation.
    UnsignedInteger {
        /// Exact compiler-resolved unsigned integer type.
        representation: ty::UintTy,
    },
    /// One floating-point representation.
    Float {
        /// Exact compiler-resolved floating-point type.
        representation: ty::FloatTy,
    },
    /// Rust character values.
    Character,
    /// Owned or borrowed UTF-8 text.
    Text,
}

impl ParameterKind {
    /// Constructs one exact signed-integer family.
    const fn signed(representation: ty::IntTy) -> Self {
        Self::SignedInteger { representation }
    }

    /// Constructs one exact unsigned-integer family.
    const fn unsigned(representation: ty::UintTy) -> Self {
        Self::UnsignedInteger { representation }
    }

    /// Constructs one exact floating-point family.
    const fn float(representation: ty::FloatTy) -> Self {
        Self::Float { representation }
    }

    /// Describes this family in a diagnostic without exposing compiler terminology.
    pub fn description(self) -> String {
        match self {
            Self::SignedInteger { representation } => {
                format!("`{representation:?}`").to_lowercase()
            }
            Self::UnsignedInteger { representation } => {
                format!("`{representation:?}`").to_lowercase()
            }
            Self::Float { representation } => format!("`{representation:?}`").to_lowercase(),
            Self::Character => "`char`".to_owned(),
            Self::Text => "textual".to_owned(),
        }
    }
}

/// One simple authored parameter from a function-like declaration.
#[derive(Clone)]
pub struct Parameter {
    /// HIR identity used to recognize binding references.
    pub hir_id: HirId,
    /// Identifier source range used for labels.
    pub span: Span,
    /// Authored binding name.
    pub name: Symbol,
    /// Whether the resolved parameter type is exactly `bool`.
    pub is_boolean: bool,
    /// Interchangeable primitive family, excluding booleans.
    pub interchangeable: Option<ParameterKind>,
}

/// One reportable family of interchangeable parameters.
pub struct ParameterGroup<'signature> {
    /// Shared semantic representation.
    pub kind: ParameterKind,
    /// Parameters carrying distinct roles through names alone.
    pub parameters: Vec<&'signature Parameter>,
}

/// Conventional role families that already communicate one shared domain.
const PARAMETER_CONVENTIONAL_ROLE_SETS: &[&[&str]] = &[
    &["left", "right"],
    &["lhs", "rhs"],
    &["first", "second"],
    &["new", "old"],
    &["source", "target"],
    &["end", "start"],
    &["max", "min"],
    &["lower", "upper"],
    &["x", "y"],
    &["x", "y", "z"],
    &["column", "row"],
    &["height", "width"],
    &["depth", "height", "width"],
];

/// Names describing ordinary text rather than a stable domain concept.
const PARAMETER_GENERIC_TEXT_ROLES: &[&str] = &[
    "contents",
    "input",
    "label",
    "message",
    "pattern",
    "replacement",
    "source",
    "text",
    "value",
];

/// Conventional operations whose textual arguments are deliberately generic.
const PARAMETER_GENERIC_TEXT_OPERATIONS: &[&str] = &["format", "join", "replace", "split", "write"];

/// Weak names that do not establish distinct semantic roles.
const PARAMETER_WEAK_ROLES: &[&str] = &["a", "arg", "argument", "b", "item", "thing", "value"];

/// Returns whether a role belongs to ordinary text-transformation vocabulary.
fn parameter_role_is_generic(name: &str) -> bool {
    PARAMETER_GENERIC_TEXT_ROLES.contains(&name) || matches!(name, "delimiter" | "separator")
}

/// Recognizes conventional role sets independently from compiler HIR records.
fn parameter_role_names_are_conventional(
    function: &str,
    kind: ParameterKind,
    names: &[String],
) -> bool {
    // Accept established symmetric, coordinate, range, and dimension vocabularies.
    if PARAMETER_CONVENTIONAL_ROLE_SETS
        .iter()
        .any(|roles| names == *roles)
    {
        return true;
    }
    if kind != ParameterKind::Text {
        return false;
    }

    // Recognize ordinary text roles independently from operation names.
    let all_roles_are_generic = names
        .iter()
        .all(|name| PARAMETER_GENERIC_TEXT_ROLES.contains(&name.as_str()));

    // Recognize generic roles attached to conventional text transformations.
    let operation_is_generic = PARAMETER_GENERIC_TEXT_OPERATIONS
        .iter()
        .any(|operation| function.contains(operation));
    let operation_roles_are_generic = names.iter().all(|name| parameter_role_is_generic(name));
    all_roles_are_generic || (operation_is_generic && operation_roles_are_generic)
}

/// Classifies exact numeric primitive representations.
fn parameter_numeric_kind(ty: Ty<'_>) -> Option<ParameterKind> {
    match ty.kind() {
        ty::Int(representation) => Some(ParameterKind::signed(*representation)),
        ty::Uint(representation) => Some(ParameterKind::unsigned(*representation)),
        ty::Float(representation) => Some(ParameterKind::float(*representation)),
        _ => None,
    }
}

/// Classifies scalar primitive and directly borrowed textual representations.
fn parameter_scalar_kind(ty: Ty<'_>) -> Option<ParameterKind> {
    if let Some(kind) = parameter_numeric_kind(ty) {
        return Some(kind);
    }
    match ty.kind() {
        ty::Char => Some(ParameterKind::Character),
        ty::Str => Some(ParameterKind::Text),
        _ => None,
    }
}

/// Classifies standard textual ADTs after resolving re-exported definition paths.
fn parameter_adt_kind(
    cx: &LateContext<'_>,
    def_id: rustc_span::def_id::DefId,
    arguments: ty::GenericArgsRef<'_>,
) -> Option<ParameterKind> {
    let path = cx.tcx.def_path_str(def_id);
    if path.ends_with("::string::String") {
        return Some(ParameterKind::Text);
    }
    if !path.ends_with("::boxed::Box") && !path.ends_with("::borrow::Cow") {
        return None;
    }
    arguments
        .types()
        .any(|argument| matches!(argument.kind(), ty::Str))
        .then_some(ParameterKind::Text)
}

/// Classifies aliases and references by their interchangeable representation.
pub fn parameter_interchangeable_kind(cx: &LateContext<'_>, ty: Ty<'_>) -> Option<ParameterKind> {
    // References do not distinguish semantic roles at the call site.
    let ty = ty.peel_refs();

    // Resolve scalar values before inspecting standard-library text containers.
    if let Some(kind) = parameter_scalar_kind(ty) {
        return Some(kind);
    }
    let ty::Adt(definition, arguments) = ty.kind() else {
        return None;
    };
    parameter_adt_kind(cx, definition.did(), arguments)
}

/// One authored function or method signature with simple named parameters.
#[derive(Clone)]
pub struct ParameterSignature {
    /// Definition identity used for semantic type queries.
    pub def_id: LocalDefId,
    /// HIR identity on which function-level diagnostics are emitted.
    pub hir_id: HirId,
    /// Authored function or method name.
    pub name: Symbol,
    /// Name source range used as the primary diagnostic site.
    pub span: Span,
    /// Direct parameters excluding `self`.
    pub parameters: Vec<Parameter>,
}

impl ParameterSignature {
    /// Collects a function or provided method under an authored Rust API boundary.
    pub fn from_body(
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
    pub fn from_required_trait(cx: &LateContext<'_>, item: &TraitItem<'_>) -> Option<Self> {
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
            interchangeable: parameter_interchangeable_kind(cx, ty),
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
            if ident.name == rustc_span::symbol::kw::SelfLower {
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
            if ident.name == rustc_span::symbol::kw::SelfLower {
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
        parameter_role_names_are_conventional(function.as_str(), kind, &names)
    }

    /// Requires precise authored names before inferring distinct domains.
    fn roles_are_distinct(parameters: &[&Parameter]) -> bool {
        let mut unique_names = HashSet::new();
        for parameter in parameters {
            let name = parameter.name.as_str();
            if PARAMETER_WEAK_ROLES.contains(&name) {
                return false;
            }
            unique_names.insert(name);
        }
        unique_names.len() >= 2
    }

    /// Returns direct boolean parameters in source order.
    pub fn boolean_parameters(&self) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|parameter| parameter.is_boolean)
            .collect()
    }

    /// Groups interchangeable non-boolean primitive parameters by representation.
    pub fn ambiguous_groups(&self) -> Vec<ParameterGroup<'_>> {
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
            if parameters.len() < 2
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
    pub fn has_exact_boolean_setter(&self) -> bool {
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

/// Returns whether a resolved type belongs to the textual family.
pub fn parameter_type_is_textual(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    parameter_interchangeable_kind(cx, ty) == Some(ParameterKind::Text)
}

#[cfg(test)]
mod tests {
    use super::{ParameterKind, parameter_role_names_are_conventional, ty};

    #[test]
    fn recognizes_same_domain_parameter_roles() {
        assert!(parameter_role_names_are_conventional(
            "point",
            ParameterKind::Float {
                representation: ty::FloatTy::F64,
            },
            &["x".to_owned(), "y".to_owned()],
        ));
        assert!(parameter_role_names_are_conventional(
            "replace",
            ParameterKind::Text,
            &[
                "pattern".to_owned(),
                "replacement".to_owned(),
                "source".to_owned(),
            ],
        ));
    }

    #[test]
    fn retains_distinct_domain_roles() {
        assert!(!parameter_role_names_are_conventional(
            "authenticate",
            ParameterKind::Text,
            &[
                "organization".to_owned(),
                "token".to_owned(),
                "user".to_owned(),
            ],
        ));
    }
}
