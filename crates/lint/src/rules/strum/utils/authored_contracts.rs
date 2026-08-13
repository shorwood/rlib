extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_hir::def::{CtorOf, DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::{
    ConstItemRhs, Expr, ExprKind, ImplItem, ImplItemKind, Item, ItemKind, Node, Pat, PatExprKind,
    PatKind,
};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::symbol::sym;
use rustc_span::{Span, Symbol};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
/// Providers capable of generating flat unit-enum string parsers.
#[serde(rename_all = "snake_case")]
pub enum StringParserProvider {
    /// Represents the `StrumEnumString` case.
    StrumEnumString,
    /// Represents the `DeriveMoreFromStr` case.
    DeriveMoreFromStr,
}

impl StringParserProvider {
    /// Performs the `providers` operation for this value.
    pub(crate) fn providers(cx: &LateContext<'_>) -> Vec<Self> {
        let mut providers = vec![Self::StrumEnumString];
        if cx.tcx.sess.opts.externs.get("derive_more").is_some() {
            providers.push(Self::DeriveMoreFromStr);
        }
        providers
    }

    /// Performs the `selected` operation for this value.
    pub(crate) fn selected(cx: &LateContext<'_>, configured: Option<Self>) -> Option<Self> {
        let providers = Self::providers(cx);
        if providers.len() == 1 {
            return providers.first().copied();
        }
        configured.filter(|provider| providers.contains(provider))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
/// Providers capable of generating enum `Display` implementations.
#[serde(rename_all = "snake_case")]
pub enum DisplayProvider {
    /// Represents the `StrumDisplay` case.
    StrumDisplay,
    /// Represents the `DeriveMoreDisplay` case.
    DeriveMoreDisplay,
}

impl DisplayProvider {
    /// Performs the `providers` operation for this value.
    pub(crate) fn providers(cx: &LateContext<'_>) -> Vec<Self> {
        let mut providers = vec![Self::StrumDisplay];
        if cx.tcx.sess.opts.externs.get("derive_more").is_some() {
            providers.push(Self::DeriveMoreDisplay);
        }
        providers
    }

    /// Performs the `selected` operation for this value.
    pub(crate) fn selected(cx: &LateContext<'_>, configured: Option<Self>) -> Option<Self> {
        let providers = Self::providers(cx);
        if providers.len() == 1 {
            return providers.first().copied();
        }
        configured.filter(|provider| providers.contains(provider))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Static literal supported by Strum message or property metadata.
pub enum StaticValue {
    /// Stores the `item` value used by this analysis.
    Bool(
        /// Boolean value returned for the variant.
        bool,
    ),
    /// Stores the `item` value used by this analysis.
    Integer(
        /// Integer value returned for the variant.
        u128,
    ),
    /// Stores the `item` value used by this analysis.
    String(
        /// String value returned for the variant.
        String,
    ),
}

impl StaticValue {
    /// Performs the `analyze_static_value` step of the lint analysis.
    fn analyze_static_value(expression: &Expr<'_>) -> Option<Self> {
        // Prepare the values used by this stage.
        let ExprKind::Lit(literal) = peel_transparent(expression).kind else {
            return None;
        };

        // Classify the current analyze_candidate.
        match literal.node {
            rustc_ast::LitKind::Bool(value) => Some(Self::Bool(value)),
            rustc_ast::LitKind::Int(value, _) => Some(Self::Integer(value.get())),
            rustc_ast::LitKind::Str(value, _) => Some(Self::String(value.as_str().to_owned())),
            _ => None,
        }
    }
}

/// Complete authored variant-to-static-value method.
pub struct VariantValueFamily {
    /// Stores the `span` value used by this analysis.
    pub(crate) span: Span,
    /// Stores the `owner` value used by this analysis.
    pub(crate) owner: rustc_hir::HirId,
    /// Stores the `definition` value used by this analysis.
    definition: LocalDefId,
    /// Stores the `enum_def` value used by this analysis.
    pub(crate) enum_def: LocalDefId,
    /// Stores the `method_name` value used by this analysis.
    pub(crate) method_name: Symbol,
    /// Stores the `values` value used by this analysis.
    pub(crate) values: HashMap<LocalDefId, StaticValue>,
    /// Stores the `is_public` value used by this analysis.
    pub(crate) is_public: bool,
}

impl VariantValueFamily {
    /// Resolves one exhaustive receiver match returning only static literals.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() {
            return None;
        }
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };

        // Reject inputs that do not satisfy this stage.
        if signature.decl.inputs.len() != 1 {
            return None;
        }
        let enum_def = enclosing_inherent_enum(cx, item.hir_id())?;
        let body = cx.tcx.hir_body(body_id);
        let receiver = binding_id(body.params.first()?.pat)?;
        let expression = peel_transparent(body.value);

        // Prepare the values used by this stage.
        let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
            return None;
        };
        if !is_local_path(cx, scrutinee, receiver) {
            return None;
        }
        let mut values = HashMap::new();

        // Process the candidates handled by this stage.
        for arm in arms {
            if arm.guard.is_some() {
                return None;
            }
            let variant = ignored_variant_pattern(cx, arm.pat)?;
            if owning_enum(cx, variant) != Some(enum_def)
                || values
                    .insert(variant, StaticValue::analyze_static_value(arm.body)?)
                    .is_some()
            {
                return None;
            }
        }

        // Prepare the values used by this stage.
        let definition = cx.tcx.adt_def(enum_def.to_def_id());
        let expected = definition
            .variants()
            .iter()
            .filter_map(|variant| variant.def_id.as_local())
            .collect::<HashSet<_>>();

        // Perform the next step of the analysis.
        (values.len() == expected.len()
            && expected.iter().all(|variant| values.contains_key(variant)))
        .then(|| Self {
            span: expression.span.source_callsite(),
            owner: item.hir_id(),
            definition: item.owner_id.def_id,
            enum_def,
            method_name: item.ident.name,
            values,
            is_public: cx.tcx.visibility(item.owner_id.def_id).is_public(),
        })
    }

    /// Returns whether the function's exact output is a static string reference.
    pub(crate) fn returns_static_str(&self, cx: &LateContext<'_>) -> bool {
        let output = cx
            .tcx
            .fn_sig(self.definition)
            .instantiate_identity()
            .skip_binder()
            .output();
        matches!(output.kind(), ty::Ref(region, inner, rustc_hir::Mutability::Not) if region.is_static() && inner.is_str())
    }
}

/// Exact enum `Display` implementation selecting one static string per variant.
pub struct DisplayCandidate {
    /// Stores the `span` value used by this analysis.
    pub(crate) span: Span,
    /// Stores the `owner` value used by this analysis.
    pub(crate) owner: rustc_hir::HirId,
    /// Stores the `enum_def` value used by this analysis.
    pub(crate) enum_def: LocalDefId,
    /// Stores the `values` value used by this analysis.
    pub(crate) values: HashMap<LocalDefId, String>,
}

impl DisplayCandidate {
    /// Receiver and formatter inputs required by `Display::fmt`.
    const DISPLAY_SIGNATURE_INPUT_COUNT: usize = 2;

    /// Performs the `from_impl_item` operation for this value.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() || item.ident.name.as_str() != "fmt" {
            return None;
        }
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };

        // Reject inputs that do not satisfy this stage.
        if signature.decl.inputs.len() != Self::DISPLAY_SIGNATURE_INPUT_COUNT {
            return None;
        }
        let enum_def = enclosing_trait_enum(
            cx,
            item.hir_id(),
            TraitIdentity {
                trait_name: "Display",
                crate_name: "core",
            },
        )?;
        let body = cx.tcx.hir_body(body_id);
        let receiver = binding_id(body.params.first()?.pat)?;
        let formatter = binding_id(body.params.get(1)?.pat)?;

        // Prepare the values used by this stage.
        let expression = peel_transparent(body.value);
        let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
            return None;
        };
        if !is_local_path(cx, scrutinee, receiver) {
            return None;
        }

        // Prepare the values used by this stage.
        let mut values = HashMap::new();

        // Process the candidates handled by this stage.
        for arm in arms {
            if arm.guard.is_some() {
                return None;
            }
            let variant = ignored_variant_pattern(cx, arm.pat)?;
            let value = formatter_write_str(cx, arm.body, formatter)?;
            if owning_enum(cx, variant) != Some(enum_def) || values.insert(variant, value).is_some()
            {
                return None;
            }
        }

        // Prepare the values used by this stage.
        let expected = cx
            .tcx
            .adt_def(enum_def.to_def_id())
            .variants()
            .iter()
            .filter_map(|variant| variant.def_id.as_local())
            .collect::<HashSet<_>>();

        // Perform the next step of the analysis.
        (values.len() == expected.len()
            && expected.iter().all(|variant| values.contains_key(variant)))
        .then(|| Self {
            span: expression.span.source_callsite(),
            owner: item.hir_id(),
            enum_def,
            values,
        })
    }
}

/// Static string table that may duplicate `VariantNames::VARIANTS`.
pub struct StringTableCandidate {
    /// Stores the `span` value used by this analysis.
    pub(crate) span: Span,
    /// Stores the `owner` value used by this analysis.
    pub(crate) owner: rustc_hir::HirId,
    /// Stores the `name` value used by this analysis.
    pub(super) name: Symbol,
    /// Stores the `enum_def` value used by this analysis.
    pub(super) enum_def: Option<LocalDefId>,
    /// Stores the `values` value used by this analysis.
    pub(super) values: Vec<String>,
    /// Stores the `is_public` value used by this analysis.
    pub(crate) is_public: bool,
}

impl StringTableCandidate {
    /// Performs the `from_item` operation for this value.
    pub(crate) fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() {
            return None;
        }
        let ItemKind::Const(_, _, _, ConstItemRhs::Body(body_id)) = item.kind else {
            return None;
        };
        let value = cx.tcx.hir_body(body_id).value;

        // Return the completed analysis result.
        Some(Self {
            span: value.span.source_callsite(),
            owner: item.hir_id(),
            name: item.kind.ident()?.name,
            enum_def: None,
            values: string_array(value)?,
            is_public: cx.tcx.visibility(item.owner_id.def_id).is_public(),
        })
    }

    /// Performs the `from_impl_item` operation for this value.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() {
            return None;
        }
        let ImplItemKind::Const(_, ConstItemRhs::Body(body_id)) = item.kind else {
            return None;
        };
        let value = cx.tcx.hir_body(body_id).value;

        // Return the completed analysis result.
        Some(Self {
            span: value.span.source_callsite(),
            owner: item.hir_id(),
            name: item.ident.name,
            enum_def: enclosing_inherent_enum(cx, item.hir_id()),
            values: string_array(value)?,
            is_public: cx.tcx.visibility(item.owner_id.def_id).is_public(),
        })
    }
}

/// Authored `FromStr` match over static string literals and unit variants.
pub struct StringParserCandidate {
    /// Stores the `span` value used by this analysis.
    pub(crate) span: Span,
    /// Stores the `owner` value used by this analysis.
    pub(crate) owner: rustc_hir::HirId,
    /// Stores the `enum_def` value used by this analysis.
    pub(crate) enum_def: LocalDefId,
    /// Stores the `names` value used by this analysis.
    pub(crate) names: HashMap<LocalDefId, Vec<String>>,
    /// Stores the `is_public` value used by this analysis.
    pub(crate) is_public: bool,
}

impl StringParserCandidate {
    /// Performs the `from_impl_item` operation for this value.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() || item.ident.name.as_str() != "from_str" {
            return None;
        }
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };

        // Reject inputs that do not satisfy this stage.
        if signature.decl.inputs.len() != 1 {
            return None;
        }
        let enum_def = enclosing_trait_enum(
            cx,
            item.hir_id(),
            TraitIdentity {
                trait_name: "FromStr",
                crate_name: "core",
            },
        )?;
        let body = cx.tcx.hir_body(body_id);
        let input = binding_id(body.params.first()?.pat)?;
        let expression = peel_transparent(body.value);

        // Prepare the values used by this stage.
        let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
            return None;
        };
        if !is_local_path(cx, scrutinee, input) {
            return None;
        }
        let mut names: HashMap<LocalDefId, Vec<String>> = HashMap::new();

        // Prepare the values used by this stage.
        let mut fallback = false;

        // Process the candidates handled by this stage.
        for arm in arms {
            if arm.guard.is_some() {
                return None;
            }
            if matches!(arm.pat.kind, PatKind::Wild) && result_err(cx, arm.body) {
                fallback = true;
                continue;
            }
            let name = string_pattern(arm.pat)?;
            let variant = result_ok_variant(cx, arm.body)?;
            if owning_enum(cx, variant) != Some(enum_def) {
                return None;
            }
            names.entry(variant).or_default().push(name);
        }

        // Prepare the values used by this stage.
        let definition = cx.tcx.adt_def(enum_def.to_def_id());
        let expected = definition
            .variants()
            .iter()
            .filter(|variant| variant.fields.is_empty())
            .filter_map(|variant| variant.def_id.as_local())
            .collect::<HashSet<_>>();

        // Perform the next step of the analysis.
        (fallback
            && names.len() == expected.len()
            && expected.iter().all(|variant| names.contains_key(variant)))
        .then(|| Self {
            span: expression.span.source_callsite(),
            owner: item.hir_id(),
            enum_def,
            names,
            is_public: cx.tcx.visibility(item.owner_id.def_id).is_public(),
        })
    }
}

/// One-to-one conversion from a payload enum into a private unit mirror enum.
pub struct DiscriminantMirrorCandidate {
    /// Stores the `span` value used by this analysis.
    pub(crate) span: Span,
    /// Stores the `owner` value used by this analysis.
    pub(crate) owner: rustc_hir::HirId,
    /// Stores the `source_enum` value used by this analysis.
    pub(crate) source_enum: LocalDefId,
    /// Stores the `mirror_enum` value used by this analysis.
    pub(crate) mirror_enum: LocalDefId,
}

impl DiscriminantMirrorCandidate {
    /// Performs the `from_impl_item` operation for this value.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() || item.ident.name.as_str() != "from" {
            return None;
        }
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };

        // Reject inputs that do not satisfy this stage.
        if signature.decl.inputs.len() != 1 {
            return None;
        }
        let mirror_enum = enclosing_trait_enum(
            cx,
            item.hir_id(),
            TraitIdentity {
                trait_name: "From",
                crate_name: "core",
            },
        )?;
        if cx.tcx.visibility(mirror_enum).is_public() {
            return None;
        }

        // Prepare the values used by this stage.
        let body = cx.tcx.hir_body(body_id);
        let input = binding_id(body.params.first()?.pat)?;
        let expression = peel_transparent(body.value);
        let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
            return None;
        };

        // Reject inputs that do not satisfy this stage.
        if !is_local_path(cx, scrutinee, input) {
            return None;
        }
        let mut source_enum = None;
        let mut observed = HashSet::new();

        // Process the candidates handled by this stage.
        for arm in arms {
            // Reject inputs that do not satisfy this stage.
            if arm.guard.is_some() {
                return None;
            }
            let source = ignored_variant_pattern(cx, arm.pat)?;
            let target = unit_variant_expression(cx, arm.body)?;
            let found_source_enum = owning_enum(cx, source)?;

            // Reject inputs that do not satisfy this stage.
            if source_enum
                .replace(found_source_enum)
                .is_some_and(|known| known != found_source_enum)
                || owning_enum(cx, target) != Some(mirror_enum)
                || cx.tcx.item_name(source.to_def_id()) != cx.tcx.item_name(target.to_def_id())
                || !observed.insert(source)
            {
                return None;
            }
        }

        // Prepare the values used by this stage.
        let source_enum = source_enum?;
        let source = cx.tcx.adt_def(source_enum.to_def_id());
        let mirror = cx.tcx.adt_def(mirror_enum.to_def_id());

        // Reject inputs that do not satisfy this stage.
        if source.variants().len() != observed.len()
            || mirror.variants().len() != observed.len()
            || !source
                .variants()
                .iter()
                .any(|variant| !variant.fields.is_empty())
            || mirror
                .variants()
                .iter()
                .any(|variant| !variant.fields.is_empty())
        // Perform the next step of the analysis.
        {
            return None;
        }

        // Return the completed analysis result.
        Some(Self {
            span: expression.span.source_callsite(),
            owner: item.hir_id(),
            source_enum,
            mirror_enum,
        })
    }
}

/// Performs the `formatter_write_str` step of the lint analysis.
fn formatter_write_str(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    formatter: rustc_hir::HirId,
) -> Option<String> {
    // Prepare the values used by this stage.
    let expression = peel_transparent(expression);
    let ExprKind::MethodCall(segment, receiver, [value], _) = expression.kind else {
        return None;
    };
    if segment.ident.name.as_str() != "write_str" || !is_local_path(cx, receiver, formatter) {
        return None;
    }

    // Classify the current analyze_candidate.
    match StaticValue::analyze_static_value(value)? {
        StaticValue::String(value) => Some(value),
        StaticValue::Bool(_) | StaticValue::Integer(_) => None,
    }
}

/// Performs the `string_array` step of the lint analysis.
fn string_array(expression: &Expr<'_>) -> Option<Vec<String>> {
    // Prepare the values used by this stage.
    let expression = peel_transparent(expression);
    let expression = if let ExprKind::AddrOf(_, _, inner) = expression.kind {
        peel_transparent(inner)
    } else {
        expression
    };

    // Prepare the values used by this stage.
    let ExprKind::Array(elements) = expression.kind else {
        return None;
    };

    // Perform the next step of the analysis.
    elements
        .iter()
        .map(
            |element| match StaticValue::analyze_static_value(element)? {
                StaticValue::String(value) => Some(value),
                StaticValue::Bool(_) | StaticValue::Integer(_) => None,
            },
        )
        .collect()
}

/// Performs the `string_pattern` step of the lint analysis.
fn string_pattern(pattern: &Pat<'_>) -> Option<String> {
    // Prepare the values used by this stage.
    let PatKind::Expr(expression) = pattern.kind else {
        return None;
    };
    let PatExprKind::Lit {
        lit,
        negated: false,
    } = expression.kind
    // Perform the next step of the analysis.
    else {
        return None;
    };
    let rustc_ast::LitKind::Str(value, _) = lit.node else {
        return None;
    };
    Some(value.as_str().to_owned())
}

/// Performs the `result_ok_variant` step of the lint analysis.
fn result_ok_variant(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<LocalDefId> {
    let ExprKind::Call(callee, [value]) = peel_transparent(expression).kind else {
        return None;
    };
    let constructor = constructor_resolution(cx, callee)?;
    if !is_result_variant(cx, constructor, "Ok") {
        return None;
    }
    unit_variant_expression(cx, value)
}

/// Performs the `result_err` step of the lint analysis.
fn result_err(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
    let ExprKind::Call(callee, [_]) = peel_transparent(expression).kind else {
        return false;
    };
    constructor_resolution(cx, callee)
        .is_some_and(|constructor| is_result_variant(cx, constructor, "Err"))
}

/// Performs the `constructor_resolution` step of the lint analysis.
fn constructor_resolution(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<DefId> {
    // Prepare the values used by this stage.
    let expression = peel_transparent(expression);
    let ExprKind::Path(path) = expression.kind else {
        return None;
    };
    let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
        cx.qpath_res(&path, expression.hir_id)
    // Perform the next step of the analysis.
    else {
        return None;
    };
    Some(constructor)
}

/// Performs the `is_result_variant` step of the lint analysis.
fn is_result_variant(cx: &LateContext<'_>, constructor: DefId, name: &str) -> bool {
    let variant = cx.tcx.parent(constructor);
    cx.tcx.item_name(variant).as_str() == name
        && cx
            .tcx
            .is_diagnostic_item(sym::Result, cx.tcx.parent(variant))
}

/// Performs the `unit_variant_expression` step of the lint analysis.
fn unit_variant_expression(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<LocalDefId> {
    let expression = peel_transparent(expression);
    let ExprKind::Path(path) = expression.kind else {
        return None;
    };
    variant_from_res(cx, cx.qpath_res(&path, expression.hir_id))
}

/// Performs the `ignored_variant_pattern` step of the lint analysis.
fn ignored_variant_pattern(cx: &LateContext<'_>, pattern: &Pat<'_>) -> Option<LocalDefId> {
    // Prepare the values used by this stage.
    let resolution = match pattern.kind {
        PatKind::Expr(expression) => {
            let PatExprKind::Path(path) = expression.kind else {
                return None;
            };
            cx.qpath_res(&path, expression.hir_id)
        }
        PatKind::TupleStruct(path, fields, _)
            if fields
                .iter()
                .all(|field| matches!(field.kind, PatKind::Wild)) =>
        {
            cx.qpath_res(&path, pattern.hir_id)
        }
        PatKind::Struct(path, fields, _)
            if fields
                .iter()
                .all(|field| matches!(field.pat.kind, PatKind::Wild)) =>
        {
            cx.qpath_res(&path, pattern.hir_id)
        }
        _ => return None,
    };

    // Perform the next step of the analysis.
    variant_from_res(cx, resolution)
}

/// Performs the `variant_from_res` step of the lint analysis.
fn variant_from_res(cx: &LateContext<'_>, resolution: Res) -> Option<LocalDefId> {
    match resolution {
        Res::Def(DefKind::Variant, variant) => variant.as_local(),
        Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) => {
            cx.tcx.opt_local_parent(constructor.as_local()?)
        }
        _ => None,
    }
}

/// Performs the `binding_id` step of the lint analysis.
const fn binding_id(pattern: &Pat<'_>) -> Option<rustc_hir::HirId> {
    let PatKind::Binding(_, binding, _, None) = pattern.kind else {
        return None;
    };
    Some(binding)
}

/// Performs the `is_local_path` step of the lint analysis.
fn is_local_path(cx: &LateContext<'_>, expression: &Expr<'_>, binding: rustc_hir::HirId) -> bool {
    let expression = peel_transparent(expression);
    let ExprKind::Path(path) = expression.kind else {
        return false;
    };
    matches!(cx.qpath_res(&path, expression.hir_id), Res::Local(found) if found == binding)
}

/// Performs the `enclosing_inherent_enum` step of the lint analysis.
fn enclosing_inherent_enum(cx: &LateContext<'_>, hir_id: rustc_hir::HirId) -> Option<LocalDefId> {
    cx.tcx.hir_parent_iter(hir_id).find_map(|(_, node)| {
        let Node::Item(item) = node else { return None };
        let ItemKind::Impl(implementation) = item.kind else {
            return None;
        };
        if implementation.of_trait.is_some() {
            return None;
        }
        cx.tcx
            .type_of(item.owner_id)
            .instantiate_identity()
            .ty_adt_def()?
            .did()
            .as_local()
    })
}

/// Identifies a trait by its defining crate and authored name.
#[derive(Clone, Copy)]
struct TraitIdentity<'name> {
    /// Authored trait name.
    trait_name: &'name str,
    /// Crate that defines the trait.
    crate_name: &'name str,
}

/// Performs the `enclosing_trait_enum` step of the lint analysis.
fn enclosing_trait_enum(
    cx: &LateContext<'_>,
    hir_id: rustc_hir::HirId,
    identity: TraitIdentity<'_>,
) -> Option<LocalDefId> {
    cx.tcx.hir_parent_iter(hir_id).find_map(|(_, node)| {
        let Node::Item(item) = node else { return None };
        let ItemKind::Impl(implementation) = item.kind else {
            return None;
        };
        let trait_def = implementation.of_trait?.trait_ref.trait_def_id()?;
        if cx.tcx.item_name(trait_def).as_str() != identity.trait_name
            || cx.tcx.crate_name(trait_def.krate).as_str() != identity.crate_name
        {
            return None;
        }
        cx.tcx
            .type_of(item.owner_id)
            .instantiate_identity()
            .ty_adt_def()?
            .did()
            .as_local()
    })
}

/// Performs the `owning_enum` step of the lint analysis.
fn owning_enum(cx: &LateContext<'_>, variant: LocalDefId) -> Option<LocalDefId> {
    cx.tcx.opt_local_parent(variant)
}

/// Performs the `peel_transparent` step of the lint analysis.
const fn peel_transparent<'hir>(mut expression: &'hir Expr<'hir>) -> &'hir Expr<'hir> {
    loop {
        expression = match expression.kind {
            ExprKind::Block(block, None) if block.stmts.is_empty() => {
                let Some(inner) = block.expr else {
                    return expression;
                };
                inner
            }
            ExprKind::DropTemps(inner) => inner,
            _ => return expression,
        };
    }
}
