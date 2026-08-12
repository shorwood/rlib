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

/// Providers capable of generating flat unit-enum string parsers.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum StringParserProvider {
    StrumEnumString,
    DeriveMoreFromStr,
}

/// Providers capable of generating enum `Display` implementations.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DisplayProvider {
    StrumDisplay,
    DeriveMoreDisplay,
}

impl DisplayProvider {
    pub(crate) fn providers(cx: &LateContext<'_>) -> Vec<Self> {
        let mut providers = vec![Self::StrumDisplay];
        if cx.tcx.sess.opts.externs.get("derive_more").is_some() {
            providers.push(Self::DeriveMoreDisplay);
        }
        providers
    }

    pub(crate) fn selected(cx: &LateContext<'_>, configured: Option<Self>) -> Option<Self> {
        let providers = Self::providers(cx);
        if providers.len() == 1 {
            return providers.first().copied();
        }
        configured.filter(|provider| providers.contains(provider))
    }
}

impl StringParserProvider {
    pub(crate) fn providers(cx: &LateContext<'_>) -> Vec<Self> {
        let mut providers = vec![Self::StrumEnumString];
        if cx.tcx.sess.opts.externs.get("derive_more").is_some() {
            providers.push(Self::DeriveMoreFromStr);
        }
        providers
    }

    pub(crate) fn selected(cx: &LateContext<'_>, configured: Option<Self>) -> Option<Self> {
        let providers = Self::providers(cx);
        if providers.len() == 1 {
            return providers.first().copied();
        }
        configured.filter(|provider| providers.contains(provider))
    }
}

// -----------------------------------------------------------------------------
// VariantValueFamily: Exhaustive variant-to-literal matches
// -----------------------------------------------------------------------------

/// Static literal supported by Strum message or property metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StaticValue {
    Bool(bool),
    Integer(u128),
    String(String),
}

/// Complete authored variant-to-static-value method.
pub(crate) struct VariantValueFamily {
    pub(crate) span: Span,
    pub(crate) owner: rustc_hir::HirId,
    pub(crate) definition: LocalDefId,
    pub(crate) enum_def: LocalDefId,
    pub(crate) method_name: Symbol,
    pub(crate) values: HashMap<LocalDefId, StaticValue>,
    pub(crate) is_public: bool,
}

/// Exact enum `Display` implementation selecting one static string per variant.
pub(crate) struct DisplayCandidate {
    pub(crate) span: Span,
    pub(crate) owner: rustc_hir::HirId,
    pub(crate) enum_def: LocalDefId,
    pub(crate) values: HashMap<LocalDefId, String>,
}

impl DisplayCandidate {
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        if item.span.from_expansion() || item.ident.name.as_str() != "fmt" {
            return None;
        }
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };
        if signature.decl.inputs.len() != 2 {
            return None;
        }
        let enum_def = enclosing_trait_enum(cx, item.hir_id(), "Display", "core")?;
        let body = cx.tcx.hir_body(body_id);
        let receiver = binding_id(body.params.first()?.pat)?;
        let formatter = binding_id(body.params.get(1)?.pat)?;
        let expression = peel_transparent(body.value);
        let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
            return None;
        };
        if !is_local_path(cx, scrutinee, receiver) {
            return None;
        }
        let mut values = HashMap::new();
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
        let expected = cx
            .tcx
            .adt_def(enum_def.to_def_id())
            .variants()
            .iter()
            .filter_map(|variant| variant.def_id.as_local())
            .collect::<HashSet<_>>();
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

impl VariantValueFamily {
    /// Resolves one exhaustive receiver match returning only static literals.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        if item.span.from_expansion() {
            return None;
        }
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };
        if signature.decl.inputs.len() != 1 {
            return None;
        }
        let enum_def = enclosing_inherent_enum(cx, item.hir_id())?;
        let body = cx.tcx.hir_body(body_id);
        let receiver = binding_id(body.params.first()?.pat)?;
        let expression = peel_transparent(body.value);
        let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
            return None;
        };
        if !is_local_path(cx, scrutinee, receiver) {
            return None;
        }
        let mut values = HashMap::new();
        for arm in arms {
            if arm.guard.is_some() {
                return None;
            }
            let variant = ignored_variant_pattern(cx, arm.pat)?;
            if owning_enum(cx, variant) != Some(enum_def)
                || values.insert(variant, static_value(arm.body)?).is_some()
            {
                return None;
            }
        }
        let definition = cx.tcx.adt_def(enum_def.to_def_id());
        let expected = definition
            .variants()
            .iter()
            .filter_map(|variant| variant.def_id.as_local())
            .collect::<HashSet<_>>();
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

// -----------------------------------------------------------------------------
// StringTableCandidate: Authored canonical-name arrays
// -----------------------------------------------------------------------------

/// Static string table that may duplicate `VariantNames::VARIANTS`.
pub(crate) struct StringTableCandidate {
    pub(crate) span: Span,
    pub(crate) owner: rustc_hir::HirId,
    pub(crate) name: Symbol,
    pub(crate) enum_def: Option<LocalDefId>,
    pub(crate) values: Vec<String>,
    pub(crate) is_public: bool,
}

impl StringTableCandidate {
    pub(crate) fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        if item.span.from_expansion() {
            return None;
        }
        let ItemKind::Const(_, _, _, ConstItemRhs::Body(body_id)) = item.kind else {
            return None;
        };
        let value = cx.tcx.hir_body(body_id).value;
        Some(Self {
            span: value.span.source_callsite(),
            owner: item.hir_id(),
            name: item.kind.ident()?.name,
            enum_def: None,
            values: string_array(value)?,
            is_public: cx.tcx.visibility(item.owner_id.def_id).is_public(),
        })
    }

    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        if item.span.from_expansion() {
            return None;
        }
        let ImplItemKind::Const(_, ConstItemRhs::Body(body_id)) = item.kind else {
            return None;
        };
        let value = cx.tcx.hir_body(body_id).value;
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

// -----------------------------------------------------------------------------
// StringParserCandidate: Exact string-to-unit-variant match
// -----------------------------------------------------------------------------

/// Authored `FromStr` match over static string literals and unit variants.
pub(crate) struct StringParserCandidate {
    pub(crate) span: Span,
    pub(crate) owner: rustc_hir::HirId,
    pub(crate) enum_def: LocalDefId,
    pub(crate) names: HashMap<LocalDefId, Vec<String>>,
    pub(crate) is_public: bool,
}

/// One-to-one conversion from a payload enum into a private unit mirror enum.
pub(crate) struct DiscriminantMirrorCandidate {
    pub(crate) span: Span,
    pub(crate) owner: rustc_hir::HirId,
    pub(crate) source_enum: LocalDefId,
    pub(crate) mirror_enum: LocalDefId,
}

impl DiscriminantMirrorCandidate {
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        if item.span.from_expansion() || item.ident.name.as_str() != "from" {
            return None;
        }
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };
        if signature.decl.inputs.len() != 1 {
            return None;
        }
        let mirror_enum = enclosing_trait_enum(cx, item.hir_id(), "From", "core")?;
        if cx.tcx.visibility(mirror_enum).is_public() {
            return None;
        }
        let body = cx.tcx.hir_body(body_id);
        let input = binding_id(body.params.first()?.pat)?;
        let expression = peel_transparent(body.value);
        let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
            return None;
        };
        if !is_local_path(cx, scrutinee, input) {
            return None;
        }
        let mut source_enum = None;
        let mut observed = HashSet::new();
        for arm in arms {
            if arm.guard.is_some() {
                return None;
            }
            let source = ignored_variant_pattern(cx, arm.pat)?;
            let target = unit_variant_expression(cx, arm.body)?;
            let found_source_enum = owning_enum(cx, source)?;
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
        let source_enum = source_enum?;
        let source = cx.tcx.adt_def(source_enum.to_def_id());
        let mirror = cx.tcx.adt_def(mirror_enum.to_def_id());
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
        {
            return None;
        }
        Some(Self {
            span: expression.span.source_callsite(),
            owner: item.hir_id(),
            source_enum,
            mirror_enum,
        })
    }
}

impl StringParserCandidate {
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        if item.span.from_expansion() || item.ident.name.as_str() != "from_str" {
            return None;
        }
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };
        if signature.decl.inputs.len() != 1 {
            return None;
        }
        let enum_def = enclosing_trait_enum(cx, item.hir_id(), "FromStr", "core")?;
        let body = cx.tcx.hir_body(body_id);
        let input = binding_id(body.params.first()?.pat)?;
        let expression = peel_transparent(body.value);
        let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
            return None;
        };
        if !is_local_path(cx, scrutinee, input) {
            return None;
        }
        let mut names: HashMap<LocalDefId, Vec<String>> = HashMap::new();
        let mut fallback = false;
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
        let definition = cx.tcx.adt_def(enum_def.to_def_id());
        let expected = definition
            .variants()
            .iter()
            .filter(|variant| variant.fields.is_empty())
            .filter_map(|variant| variant.def_id.as_local())
            .collect::<HashSet<_>>();
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

// -----------------------------------------------------------------------------
// Shared HIR resolution
// -----------------------------------------------------------------------------

fn static_value(expression: &Expr<'_>) -> Option<StaticValue> {
    let ExprKind::Lit(literal) = peel_transparent(expression).kind else {
        return None;
    };
    match literal.node {
        rustc_ast::LitKind::Bool(value) => Some(StaticValue::Bool(value)),
        rustc_ast::LitKind::Int(value, _) => Some(StaticValue::Integer(value.get())),
        rustc_ast::LitKind::Str(value, _) => Some(StaticValue::String(value.as_str().to_owned())),
        _ => None,
    }
}

fn formatter_write_str(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    formatter: rustc_hir::HirId,
) -> Option<String> {
    let expression = peel_transparent(expression);
    let ExprKind::MethodCall(segment, receiver, [value], _) = expression.kind else {
        return None;
    };
    if segment.ident.name.as_str() != "write_str" || !is_local_path(cx, receiver, formatter) {
        return None;
    }
    match static_value(value)? {
        StaticValue::String(value) => Some(value),
        StaticValue::Bool(_) | StaticValue::Integer(_) => None,
    }
}

fn string_array(expression: &Expr<'_>) -> Option<Vec<String>> {
    let expression = peel_transparent(expression);
    let expression = if let ExprKind::AddrOf(_, _, inner) = expression.kind {
        peel_transparent(inner)
    } else {
        expression
    };
    let ExprKind::Array(elements) = expression.kind else {
        return None;
    };
    elements
        .iter()
        .map(|element| match static_value(element)? {
            StaticValue::String(value) => Some(value),
            StaticValue::Bool(_) | StaticValue::Integer(_) => None,
        })
        .collect()
}

fn string_pattern(pattern: &Pat<'_>) -> Option<String> {
    let PatKind::Expr(expression) = pattern.kind else {
        return None;
    };
    let PatExprKind::Lit {
        lit,
        negated: false,
    } = expression.kind
    else {
        return None;
    };
    let rustc_ast::LitKind::Str(value, _) = lit.node else {
        return None;
    };
    Some(value.as_str().to_owned())
}

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

fn result_err(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
    let ExprKind::Call(callee, [_]) = peel_transparent(expression).kind else {
        return false;
    };
    constructor_resolution(cx, callee)
        .is_some_and(|constructor| is_result_variant(cx, constructor, "Err"))
}

fn constructor_resolution(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<DefId> {
    let expression = peel_transparent(expression);
    let ExprKind::Path(path) = expression.kind else {
        return None;
    };
    let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
        cx.qpath_res(&path, expression.hir_id)
    else {
        return None;
    };
    Some(constructor)
}

fn is_result_variant(cx: &LateContext<'_>, constructor: DefId, name: &str) -> bool {
    let variant = cx.tcx.parent(constructor);
    cx.tcx.item_name(variant).as_str() == name
        && cx
            .tcx
            .is_diagnostic_item(sym::Result, cx.tcx.parent(variant))
}

fn unit_variant_expression(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<LocalDefId> {
    let expression = peel_transparent(expression);
    let ExprKind::Path(path) = expression.kind else {
        return None;
    };
    variant_from_res(cx, cx.qpath_res(&path, expression.hir_id))
}

fn ignored_variant_pattern(cx: &LateContext<'_>, pattern: &Pat<'_>) -> Option<LocalDefId> {
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
    variant_from_res(cx, resolution)
}

fn variant_from_res(cx: &LateContext<'_>, resolution: Res) -> Option<LocalDefId> {
    match resolution {
        Res::Def(DefKind::Variant, variant) => variant.as_local(),
        Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) => {
            cx.tcx.opt_local_parent(constructor.as_local()?)
        }
        _ => None,
    }
}

const fn binding_id(pattern: &Pat<'_>) -> Option<rustc_hir::HirId> {
    let PatKind::Binding(_, binding, _, None) = pattern.kind else {
        return None;
    };
    Some(binding)
}

fn is_local_path(cx: &LateContext<'_>, expression: &Expr<'_>, binding: rustc_hir::HirId) -> bool {
    let expression = peel_transparent(expression);
    let ExprKind::Path(path) = expression.kind else {
        return false;
    };
    matches!(cx.qpath_res(&path, expression.hir_id), Res::Local(found) if found == binding)
}

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

fn enclosing_trait_enum(
    cx: &LateContext<'_>,
    hir_id: rustc_hir::HirId,
    trait_name: &str,
    crate_name: &str,
) -> Option<LocalDefId> {
    cx.tcx.hir_parent_iter(hir_id).find_map(|(_, node)| {
        let Node::Item(item) = node else { return None };
        let ItemKind::Impl(implementation) = item.kind else {
            return None;
        };
        let trait_def = implementation.of_trait?.trait_ref.trait_def_id()?;
        if cx.tcx.item_name(trait_def).as_str() != trait_name
            || cx.tcx.crate_name(trait_def.krate).as_str() != crate_name
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

fn owning_enum(cx: &LateContext<'_>, variant: LocalDefId) -> Option<LocalDefId> {
    cx.tcx.opt_local_parent(variant)
}

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
