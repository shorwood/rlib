extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use convert_case::{Case, Casing};
use rustc_hir::def::{CtorKind, CtorOf, DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::{Arm, Expr, ExprKind, ImplItem, ImplItemKind, Node, Pat, PatExprKind, PatKind};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::symbol::sym;
use rustc_span::{Span, Symbol};
use serde::Deserialize;

/// Provider values accepted by the enum-predicate conflict knob.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PredicateProvider {
    StrumEnumIs,
    DeriveMoreIsVariant,
}

/// Complete authored predicate family reproducible by `EnumIs`.
pub(crate) struct PredicateFamily {
    pub(crate) span: Span,
    pub(crate) owner: rustc_hir::HirId,
    enum_def: LocalDefId,
    is_public_api: bool,
}

impl PredicateFamily {
    /// Returns every provider directly available to the consumer.
    pub(crate) fn providers(cx: &LateContext<'_>) -> Vec<PredicateProvider> {
        let mut providers = vec![PredicateProvider::StrumEnumIs];
        if cx.tcx.sess.opts.externs.get("derive_more").is_some() {
            providers.push(PredicateProvider::DeriveMoreIsVariant);
        }
        providers
    }

    /// Selects the sole provider or the explicitly configured provider.
    pub(crate) fn selected(
        cx: &LateContext<'_>,
        configured: Option<PredicateProvider>,
    ) -> Option<PredicateProvider> {
        let providers = Self::providers(cx);
        if providers.len() == 1 {
            return providers.first().copied();
        }
        configured.filter(|provider| providers.contains(provider))
    }

    /// Returns the enum's authored name.
    pub(crate) fn enum_name(&self, cx: &LateContext<'_>) -> Symbol {
        cx.tcx.item_name(self.enum_def.to_def_id())
    }

    /// Returns whether replacing the family changes a public API.
    pub(crate) const fn is_public_api(&self) -> bool {
        self.is_public_api
    }
}

/// Accumulates predicate methods until a complete enum family can be proven.
#[derive(Default)]
pub(crate) struct PredicateFamilyAnalyzer {
    methods: HashMap<LocalDefId, Vec<PredicateMethod>>,
}

impl PredicateFamilyAnalyzer {
    /// Records one exact authored variant predicate when eligible.
    pub(crate) fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        if let Some(method) = PredicateMethod::from_impl_item(cx, item) {
            self.methods
                .entry(method.enum_def)
                .or_default()
                .push(method);
        }
    }

    /// Returns only families covering every enabled enum variant exactly once.
    pub(crate) fn complete_families(&self, cx: &LateContext<'_>) -> Vec<PredicateFamily> {
        self.methods
            .iter()
            .filter_map(|(&enum_def, methods)| {
                let expected = eligible_variants(cx, enum_def)?;
                let found = methods
                    .iter()
                    .map(|method| method.variant)
                    .collect::<HashSet<_>>();
                if methods.len() != expected.len()
                    || found.len() != expected.len()
                    || !expected.iter().all(|variant| found.contains(variant))
                {
                    return None;
                }
                let first = methods.first()?;
                Some(PredicateFamily {
                    span: first.span,
                    owner: first.owner,
                    enum_def,
                    is_public_api: methods.iter().any(|method| method.is_public_api),
                })
            })
            .collect()
    }
}

/// One exact `is_variant`-style inherent method.
struct PredicateMethod {
    span: Span,
    owner: rustc_hir::HirId,
    enum_def: LocalDefId,
    variant: LocalDefId,
    is_public_api: bool,
}

impl PredicateMethod {
    /// Resolves one shared-receiver, exact single-variant boolean match.
    fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
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
        if !shared_receiver(cx, item.owner_id.def_id, enum_def) {
            return None;
        }
        let body = cx.tcx.hir_body(body_id);
        let receiver = binding_id(body.params.first()?.pat)?;
        let (variant, span) = predicate_match(cx, body.value, receiver)?;
        if owning_enum(cx, variant) != Some(enum_def) {
            return None;
        }
        let expected_name = format!(
            "is_{}",
            cx.tcx
                .item_name(variant.to_def_id())
                .as_str()
                .to_case(Case::Snake)
        );
        if item.ident.name.as_str() != expected_name {
            return None;
        }
        Some(Self {
            span,
            owner: item.hir_id(),
            enum_def,
            variant,
            is_public_api: cx.tcx.visibility(item.owner_id.def_id).is_public(),
        })
    }
}

/// Receiver mode generated by `EnumTryAs`.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum AccessorMode {
    Owned,
    Shared,
    Mutable,
}

/// Complete authored accessor family reproducible by `EnumTryAs`.
pub(crate) struct AccessorFamily {
    pub(crate) span: Span,
    pub(crate) owner: rustc_hir::HirId,
    enum_def: LocalDefId,
    is_public_api: bool,
}

impl AccessorFamily {
    /// Returns the enum's authored name.
    pub(crate) fn enum_name(&self, cx: &LateContext<'_>) -> Symbol {
        cx.tcx.item_name(self.enum_def.to_def_id())
    }

    /// Returns whether replacing the family changes a public API.
    pub(crate) const fn is_public_api(&self) -> bool {
        self.is_public_api
    }
}

/// Accumulates exact payload accessors until every generated method is represented.
#[derive(Default)]
pub(crate) struct AccessorFamilyAnalyzer {
    methods: HashMap<LocalDefId, Vec<AccessorMethod>>,
}

impl AccessorFamilyAnalyzer {
    /// Records one exact authored tuple-variant accessor when eligible.
    pub(crate) fn check_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        if let Some(method) = AccessorMethod::from_impl_item(cx, item) {
            self.methods
                .entry(method.enum_def)
                .or_default()
                .push(method);
        }
    }

    /// Returns only families matching all three generated modes for every tuple variant.
    pub(crate) fn complete_families(&self, cx: &LateContext<'_>) -> Vec<AccessorFamily> {
        self.methods
            .iter()
            .filter_map(|(&enum_def, methods)| {
                let variants = eligible_tuple_variants(cx, enum_def)?;
                let expected = variants.len().checked_mul(3)?;
                let found = methods
                    .iter()
                    .map(|method| (method.variant, method.mode))
                    .collect::<HashSet<_>>();
                if methods.len() != expected
                    || found.len() != expected
                    || !variants.iter().all(|variant| {
                        [
                            AccessorMode::Owned,
                            AccessorMode::Shared,
                            AccessorMode::Mutable,
                        ]
                        .into_iter()
                        .all(|mode| found.contains(&(*variant, mode)))
                    })
                {
                    return None;
                }
                let first = methods.first()?;
                Some(AccessorFamily {
                    span: first.span,
                    owner: first.owner,
                    enum_def,
                    is_public_api: methods.iter().any(|method| method.is_public_api),
                })
            })
            .collect()
    }
}

/// One exact owned, shared, or mutable tuple-variant accessor.
struct AccessorMethod {
    span: Span,
    owner: rustc_hir::HirId,
    enum_def: LocalDefId,
    variant: LocalDefId,
    mode: AccessorMode,
    is_public_api: bool,
}

impl AccessorMethod {
    /// Resolves one `Option` accessor that returns bound fields without transformation.
    fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
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
        let mode = receiver_mode(cx, item.owner_id.def_id, enum_def)?;
        option_output(cx, item.owner_id.def_id)?;
        let body = cx.tcx.hir_body(body_id);
        let receiver = binding_id(body.params.first()?.pat)?;
        let (variant, span) = accessor_match(cx, body.value, receiver)?;
        if owning_enum(cx, variant) != Some(enum_def) {
            return None;
        }
        let suffix = match mode {
            AccessorMode::Owned => "",
            AccessorMode::Shared => "_ref",
            AccessorMode::Mutable => "_mut",
        };
        let expected_name = format!(
            "try_as_{}{}",
            cx.tcx
                .item_name(variant.to_def_id())
                .as_str()
                .to_case(Case::Snake),
            suffix
        );
        if item.ident.name.as_str() != expected_name {
            return None;
        }
        Some(Self {
            span,
            owner: item.hir_id(),
            enum_def,
            variant,
            mode,
            is_public_api: cx.tcx.visibility(item.owner_id.def_id).is_public(),
        })
    }
}

/// One exact inherent integer-to-unit-variant conversion reproducible by `FromRepr`.
pub(crate) struct ReprConversionCandidate {
    pub(crate) span: Span,
    pub(crate) owner: rustc_hir::HirId,
    enum_def: LocalDefId,
    is_public_api: bool,
}

impl ReprConversionCandidate {
    /// Resolves an exhaustive literal-to-variant `from_repr` implementation.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        if item.span.from_expansion() || item.ident.name.as_str() != "from_repr" {
            return None;
        }
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };
        if signature.decl.inputs.len() != 1 {
            return None;
        }
        let enum_def = enclosing_inherent_enum(cx, item.hir_id())?;
        cx.tcx.adt_def(enum_def.to_def_id()).repr().int?;
        let output = option_output(cx, item.owner_id.def_id)?;
        if output.ty_adt_def()?.did().as_local() != Some(enum_def) {
            return None;
        }
        let body = cx.tcx.hir_body(body_id);
        let input = binding_id(body.params.first()?.pat)?;
        let span = repr_match(cx, body.value, input, enum_def)?;
        Some(Self {
            span,
            owner: item.hir_id(),
            enum_def,
            is_public_api: cx.tcx.visibility(item.owner_id.def_id).is_public(),
        })
    }

    /// Returns the enum's authored name.
    pub(crate) fn enum_name(&self, cx: &LateContext<'_>) -> Symbol {
        cx.tcx.item_name(self.enum_def.to_def_id())
    }

    /// Returns whether replacing the method changes a public API.
    pub(crate) const fn is_public_api(&self) -> bool {
        self.is_public_api
    }
}

/// Resolves an exact two-arm variant predicate.
fn predicate_match(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    receiver: rustc_hir::HirId,
) -> Option<(LocalDefId, Span)> {
    let expression = peel_transparent(expression);
    let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
        return None;
    };
    if !is_local_path(cx, scrutinee, receiver) || arms.len() != 2 {
        return None;
    }
    let (positive, negative) = bool_arms(arms)?;
    if positive.guard.is_some()
        || negative.guard.is_some()
        || !matches!(negative.pat.kind, PatKind::Wild)
    {
        return None;
    }
    let variant = ignored_variant_pattern(cx, positive.pat)?;
    Some((variant, expression.span.source_callsite()))
}

/// Resolves an exact two-arm payload extraction match.
fn accessor_match(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    receiver: rustc_hir::HirId,
) -> Option<(LocalDefId, Span)> {
    let expression = peel_transparent(expression);
    let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
        return None;
    };
    if !is_local_path(cx, scrutinee, receiver) || arms.len() != 2 {
        return None;
    }
    let success = arms
        .iter()
        .find(|arm| option_some(cx, arm.body).is_some())?;
    let failure = arms.iter().find(|arm| is_option_none(cx, arm.body))?;
    if success.guard.is_some()
        || failure.guard.is_some()
        || !matches!(failure.pat.kind, PatKind::Wild)
    {
        return None;
    }
    let (variant, bindings) = bound_tuple_variant(cx, success.pat)?;
    let returned = option_some(cx, success.body)?;
    if !returns_bindings(cx, returned, &bindings) {
        return None;
    }
    Some((variant, expression.span.source_callsite()))
}

/// Resolves an exhaustive positive integer discriminant match.
fn repr_match(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    input: rustc_hir::HirId,
    enum_def: LocalDefId,
) -> Option<Span> {
    let expression = peel_transparent(expression);
    let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
        return None;
    };
    if !is_local_path(cx, scrutinee, input) {
        return None;
    }
    let mut observed = HashMap::new();
    let mut has_fallback = false;
    for arm in arms {
        if arm.guard.is_some() {
            return None;
        }
        if matches!(arm.pat.kind, PatKind::Wild) && is_option_none(cx, arm.body) {
            has_fallback = true;
            continue;
        }
        let value = integer_pattern(arm.pat)?;
        let variant_expression = option_some(cx, arm.body)?;
        let variant = unit_variant_expression(cx, variant_expression)?;
        if owning_enum(cx, variant) != Some(enum_def) || observed.insert(variant, value).is_some() {
            return None;
        }
    }
    if !has_fallback || enum_has_strum_variant_attributes(cx, enum_def) {
        return None;
    }
    let definition = cx.tcx.adt_def(enum_def.to_def_id());
    if definition
        .variants()
        .iter()
        .any(|variant| !variant.fields.is_empty())
        || observed.len() != definition.variants().len()
    {
        return None;
    }
    for (index, discriminant) in definition.discriminants(cx.tcx) {
        let variant = definition.variant(index).def_id.as_local()?;
        if observed.get(&variant).copied() != Some(discriminant.val) {
            return None;
        }
    }
    Some(expression.span.source_callsite())
}

/// Finds the true and false arms of a boolean match.
fn bool_arms<'hir>(arms: &'hir [Arm<'hir>]) -> Option<(&'hir Arm<'hir>, &'hir Arm<'hir>)> {
    let positive = arms
        .iter()
        .find(|arm| bool_literal(arm.body) == Some(true))?;
    let negative = arms
        .iter()
        .find(|arm| bool_literal(arm.body) == Some(false))?;
    Some((positive, negative))
}

/// Resolves a literal boolean after transparent blocks.
const fn bool_literal(expression: &Expr<'_>) -> Option<bool> {
    let ExprKind::Lit(literal) = peel_transparent(expression).kind else {
        return None;
    };
    let rustc_ast::LitKind::Bool(value) = literal.node else {
        return None;
    };
    Some(value)
}

/// Resolves a positive integer literal pattern.
fn integer_pattern(pattern: &Pat<'_>) -> Option<u128> {
    let PatKind::Expr(expression) = pattern.kind else {
        return None;
    };
    let PatExprKind::Lit {
        lit: literal,
        negated: false,
    } = expression.kind
    else {
        return None;
    };
    let rustc_ast::LitKind::Int(value, _) = literal.node else {
        return None;
    };
    Some(value.get())
}

/// Resolves a direct unit-variant expression.
fn unit_variant_expression(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<LocalDefId> {
    let ExprKind::Path(path) = peel_transparent(expression).kind else {
        return None;
    };
    variant_from_res(cx, cx.qpath_res(&path, expression.hir_id))
}

/// Resolves a variant pattern whose payload is entirely ignored.
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

/// Resolves a tuple-variant pattern and its ordered field bindings.
fn bound_tuple_variant(
    cx: &LateContext<'_>,
    pattern: &Pat<'_>,
) -> Option<(LocalDefId, Vec<rustc_hir::HirId>)> {
    let PatKind::TupleStruct(path, fields, rest) = pattern.kind else {
        return None;
    };
    if rest.as_opt_usize().is_some() {
        return None;
    }
    let bindings = fields
        .iter()
        .map(|field| binding_id(field))
        .collect::<Option<Vec<_>>>()?;
    Some((
        variant_from_res(cx, cx.qpath_res(&path, pattern.hir_id))?,
        bindings,
    ))
}

/// Resolves a path or constructor resolution to its local variant definition.
fn variant_from_res(cx: &LateContext<'_>, resolution: Res) -> Option<LocalDefId> {
    match resolution {
        Res::Def(DefKind::Variant, variant) => variant.as_local(),
        Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) => {
            cx.tcx.opt_local_parent(constructor.as_local()?)
        }
        _ => None,
    }
}

/// Resolves one plain binding pattern.
const fn binding_id(pattern: &Pat<'_>) -> Option<rustc_hir::HirId> {
    let PatKind::Binding(_, binding, _, None) = pattern.kind else {
        return None;
    };
    Some(binding)
}

/// Returns whether an expression is a direct path to a local binding.
fn is_local_path(cx: &LateContext<'_>, expression: &Expr<'_>, binding: rustc_hir::HirId) -> bool {
    let expression = peel_transparent(expression);
    let ExprKind::Path(path) = expression.kind else {
        return false;
    };
    matches!(cx.qpath_res(&path, expression.hir_id), Res::Local(found) if found == binding)
}

/// Returns the payload expression inside standard `Option::Some`.
fn option_some<'hir>(
    cx: &LateContext<'_>,
    expression: &'hir Expr<'hir>,
) -> Option<&'hir Expr<'hir>> {
    let expression = peel_transparent(expression);
    let ExprKind::Call(callee, [value]) = expression.kind else {
        return None;
    };
    let ExprKind::Path(path) = peel_transparent(callee).kind else {
        return None;
    };
    let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
        cx.qpath_res(&path, callee.hir_id)
    else {
        return None;
    };
    is_option_variant(cx, constructor, "Some").then_some(value)
}

/// Returns whether an expression is standard `Option::None`.
fn is_option_none(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
    let expression = peel_transparent(expression);
    let ExprKind::Path(path) = expression.kind else {
        return false;
    };
    matches!(cx.qpath_res(&path, expression.hir_id), Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) if is_option_variant(cx, constructor, "None"))
}

/// Returns whether a constructor is the named variant of standard `Option`.
fn is_option_variant(cx: &LateContext<'_>, constructor: DefId, name: &str) -> bool {
    let variant = cx.tcx.parent(constructor);
    cx.tcx.item_name(variant).as_str() == name
        && cx
            .tcx
            .is_diagnostic_item(sym::Option, cx.tcx.parent(variant))
}

/// Returns the payload of an exact standard `Option` return type.
fn option_output<'tcx>(cx: &LateContext<'tcx>, definition: LocalDefId) -> Option<ty::Ty<'tcx>> {
    let output = cx
        .tcx
        .fn_sig(definition)
        .instantiate_identity()
        .skip_binder()
        .output();
    let ty::Adt(option, arguments) = output.kind() else {
        return None;
    };
    cx.tcx
        .is_diagnostic_item(sym::Option, option.did())
        .then(|| arguments.type_at(0))
}

/// Checks that a `Some` payload returns every pattern binding in order.
fn returns_bindings(
    cx: &LateContext<'_>,
    expression: &Expr<'_>,
    bindings: &[rustc_hir::HirId],
) -> bool {
    if bindings.len() == 1 {
        return is_local_path(cx, expression, bindings[0]);
    }
    let ExprKind::Tup(elements) = peel_transparent(expression).kind else {
        return false;
    };
    elements.len() == bindings.len()
        && elements
            .iter()
            .zip(bindings)
            .all(|(element, binding)| is_local_path(cx, element, *binding))
}

/// Resolves the receiver mode and verifies that it targets the enclosing enum.
fn receiver_mode(
    cx: &LateContext<'_>,
    definition: LocalDefId,
    enum_def: LocalDefId,
) -> Option<AccessorMode> {
    let signature = cx
        .tcx
        .fn_sig(definition)
        .instantiate_identity()
        .skip_binder();
    let receiver = signature.inputs().first()?;
    match receiver.kind() {
        ty::Adt(definition, _) if definition.did().as_local() == Some(enum_def) => {
            Some(AccessorMode::Owned)
        }
        ty::Ref(_, inner, rustc_hir::Mutability::Not)
            if inner.ty_adt_def()?.did().as_local() == Some(enum_def) =>
        {
            Some(AccessorMode::Shared)
        }
        ty::Ref(_, inner, rustc_hir::Mutability::Mut)
            if inner.ty_adt_def()?.did().as_local() == Some(enum_def) =>
        {
            Some(AccessorMode::Mutable)
        }
        _ => None,
    }
}

/// Verifies the shared receiver contract generated by `EnumIs`.
fn shared_receiver(cx: &LateContext<'_>, definition: LocalDefId, enum_def: LocalDefId) -> bool {
    receiver_mode(cx, definition, enum_def) == Some(AccessorMode::Shared)
        && cx
            .tcx
            .fn_sig(definition)
            .instantiate_identity()
            .skip_binder()
            .output()
            .is_bool()
}

/// Finds the local enum targeted by an inherent implementation item.
fn enclosing_inherent_enum(cx: &LateContext<'_>, hir_id: rustc_hir::HirId) -> Option<LocalDefId> {
    cx.tcx.hir_parent_iter(hir_id).find_map(|(_, node)| {
        let Node::Item(item) = node else {
            return None;
        };
        let rustc_hir::ItemKind::Impl(implementation) = item.kind else {
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

/// Returns the enum owning one local variant.
fn owning_enum(cx: &LateContext<'_>, variant: LocalDefId) -> Option<LocalDefId> {
    cx.tcx.opt_local_parent(variant)
}

/// Returns every variant when Strum attributes cannot change generated coverage.
fn eligible_variants(cx: &LateContext<'_>, enum_def: LocalDefId) -> Option<Vec<LocalDefId>> {
    if enum_has_strum_variant_attributes(cx, enum_def) {
        return None;
    }
    let definition = cx.tcx.adt_def(enum_def.to_def_id());
    definition.is_enum().then(|| {
        definition
            .variants()
            .iter()
            .filter_map(|variant| variant.def_id.as_local())
            .collect()
    })
}

/// Returns every tuple variant when Strum attributes cannot change generated coverage.
fn eligible_tuple_variants(cx: &LateContext<'_>, enum_def: LocalDefId) -> Option<Vec<LocalDefId>> {
    let variants = eligible_variants(cx, enum_def)?;
    let definition = cx.tcx.adt_def(enum_def.to_def_id());
    Some(
        variants
            .into_iter()
            .filter(|variant| {
                definition
                    .variants()
                    .iter()
                    .find(|candidate| candidate.def_id.as_local() == Some(*variant))
                    .is_some_and(|candidate| {
                        !candidate.fields.is_empty() && candidate.ctor_kind() == Some(CtorKind::Fn)
                    })
            })
            .collect(),
    )
}

/// Rejects enums whose variant attributes may disable or rename generated methods.
fn enum_has_strum_variant_attributes(cx: &LateContext<'_>, enum_def: LocalDefId) -> bool {
    let Node::Item(item) = cx.tcx.hir_node_by_def_id(enum_def) else {
        return true;
    };
    let rustc_hir::ItemKind::Enum(_, _, definition) = item.kind else {
        return true;
    };
    let strum = Symbol::intern("strum");
    definition.variants.iter().any(|variant| {
        cx.tcx
            .hir_attrs(variant.hir_id)
            .iter()
            .any(|attribute| attribute.has_name(strum))
    })
}

/// Removes blocks and compiler drop-temporary wrappers without changing semantics.
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
