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

// -----------------------------------------------------------------------------
// StringParserProvider: Configured parser ownership
// -----------------------------------------------------------------------------
use crate::config::providers::{DisplayProvider, StringParserProvider};

impl StringParserProvider {
    /// Lists framework providers available in the current compilation.
    pub(crate) fn providers(cx: &LateContext<'_>) -> Vec<Self> {
        let mut providers = vec![Self::StrumEnumString];
        if cx.tcx.sess.opts.externs.get("derive_more").is_some() {
            providers.push(Self::DeriveMoreFromStr);
        }
        providers
    }

    /// Resolves the configured provider when several implementations are available.
    pub(crate) fn selected(cx: &LateContext<'_>, configured: Option<Self>) -> Option<Self> {
        let providers = Self::providers(cx);

        // A sole available provider needs no configuration to resolve ownership.
        if providers.len() == 1 {
            return providers.first().copied();
        }
        configured.filter(|provider| providers.contains(provider))
    }
}

// -----------------------------------------------------------------------------
// DisplayProvider: Configured display ownership
// -----------------------------------------------------------------------------

impl DisplayProvider {
    /// Lists framework providers available in the current compilation.
    pub(crate) fn providers(cx: &LateContext<'_>) -> Vec<Self> {
        let mut providers = vec![Self::StrumDisplay];
        if cx.tcx.sess.opts.externs.get("derive_more").is_some() {
            providers.push(Self::DeriveMoreDisplay);
        }
        providers
    }

    /// Resolves the configured provider when several implementations are available.
    pub(crate) fn selected(cx: &LateContext<'_>, configured: Option<Self>) -> Option<Self> {
        let providers = Self::providers(cx);

        // A sole available provider needs no configuration to resolve ownership.
        if providers.len() == 1 {
            return providers.first().copied();
        }
        configured.filter(|provider| providers.contains(provider))
    }
}

// -----------------------------------------------------------------------------
// StaticValue: Static metadata values
// -----------------------------------------------------------------------------

/// Static literal supported by Strum message or property metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StaticValue {
    /// Boolean literal returned for a matched variant.
    Bool(
        /// Boolean value returned for the variant.
        bool,
    ),
    /// Unsigned integer literal returned for a matched variant.
    Integer(
        /// Integer value returned for the variant.
        u128,
    ),
    /// String literal returned for a matched variant.
    String(
        /// String value returned for the variant.
        String,
    ),
}

impl StaticValue {
    /// Recovers a supported metadata literal after removing transparent wrappers.
    fn from_expression(expression: &Expr<'_>) -> Option<Self> {
        // Static metadata must be represented by a literal expression.
        let ExprKind::Lit(literal) = AuthoredContractAnalysis::peel_transparent(expression).kind
        else {
            return None;
        };

        match literal.node {
            rustc_ast::LitKind::Bool(value) => Some(Self::Bool(value)),
            rustc_ast::LitKind::Int(value, _) => Some(Self::Integer(value.get())),
            rustc_ast::LitKind::Str(value, _) => Some(Self::String(value.as_str().to_owned())),
            _ => None,
        }
    }
}

// -----------------------------------------------------------------------------
// VariantValueFamily: Complete static metadata mappings
// -----------------------------------------------------------------------------

/// Complete authored variant-to-static-value method.
pub struct VariantValueFamily {
    /// Authored declaration or expression range used as the diagnostic anchor.
    pub(crate) span: Span,
    /// Declaration whose lint level governs this finding.
    pub(crate) owner: rustc_hir::HirId,
    /// Local declaration identity used to associate evidence collected in separate passes.
    definition: LocalDefId,
    /// Local enum definition that owns the analyzed contract.
    pub(crate) enum_def: LocalDefId,
    /// Authored method that exposes the variant-to-value mapping.
    pub(crate) method_name: Symbol,
    /// Distinct semantic values collected for this contract.
    pub(crate) values: HashMap<LocalDefId, StaticValue>,
    /// Whether the declaration is visible outside its defining module.
    pub(crate) is_public: bool,
}

impl VariantValueFamily {
    /// Resolves one exhaustive receiver match returning only static literals.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Macro-expanded methods are not reliable authored contract evidence.
        if item.span.from_expansion() {
            return None;
        }

        // Only function items can provide the required receiver match.
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };

        // The contract requires exactly one implicit receiver parameter.
        if signature.decl.inputs.len() != 1 || !signature.decl.implicit_self.has_implicit_self() {
            return None;
        }
        let enum_def = AuthoredContractAnalysis::enclosing_inherent_enum(cx, item.hir_id())?;
        let receiver = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder()
            .inputs()[0];

        // The method must borrow the enum whose variants it maps.
        if !matches!(receiver.kind(), ty::Ref(_, inner, rustc_hir::Mutability::Not) if inner.ty_adt_def()?.did().as_local() == Some(enum_def))
        {
            return None;
        }
        let body = cx.tcx.hir_body(body_id);
        let receiver = AuthoredContractAnalysis::binding_id(body.params.first()?.pat)?;
        let expression = AuthoredContractAnalysis::peel_transparent(body.value);

        // Static mappings are recognized only from an explicit variant match.
        let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
            return None;
        };

        // The match must dispatch on the method receiver itself.
        if !AuthoredContractAnalysis::is_local_path(cx, scrutinee, receiver) {
            return None;
        }
        let mut values = HashMap::new();

        for arm in arms {
            // Guarded arms cannot establish a complete variant mapping.
            if arm.guard.is_some() {
                return None;
            }
            let variant = AuthoredContractAnalysis::ignored_variant_pattern(cx, arm.pat)?;

            // Every arm must cover a distinct variant of the enclosing enum.
            if AuthoredContractAnalysis::owning_enum(cx, variant) != Some(enum_def)
                || values
                    .insert(variant, StaticValue::from_expression(arm.body)?)
                    .is_some()
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
// DisplayCandidate: Exact enum-to-string implementations
// -----------------------------------------------------------------------------

/// Exact enum `Display` implementation selecting one static string per variant.
pub struct DisplayCandidate {
    /// Authored declaration or expression range used as the diagnostic anchor.
    pub(crate) span: Span,
    /// Declaration whose lint level governs this finding.
    pub(crate) owner: rustc_hir::HirId,
    /// Local enum definition that owns the analyzed contract.
    pub(crate) enum_def: LocalDefId,
    /// Distinct semantic values collected for this contract.
    pub(crate) values: HashMap<LocalDefId, String>,
}

impl DisplayCandidate {
    /// Receiver and formatter inputs required by `Display::fmt`.
    const DISPLAY_SIGNATURE_INPUT_COUNT: usize = 2;

    /// Recovers an exhaustive static `Display` mapping for one enum.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Only authored `fmt` methods can implement the candidate `Display` contract.
        if item.span.from_expansion() || item.ident.name.as_str() != "fmt" {
            return None;
        }

        // Only function items can provide the formatter match body.
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };

        // `Display::fmt` requires its receiver and formatter inputs.
        if signature.decl.inputs.len() != Self::DISPLAY_SIGNATURE_INPUT_COUNT {
            return None;
        }
        let enum_def = AuthoredContractAnalysis::enclosing_trait_enum(
            cx,
            item.hir_id(),
            TraitIdentity {
                trait_name: "Display",
                crate_name: "core",
            },
        )?;
        let body = cx.tcx.hir_body(body_id);
        let receiver = AuthoredContractAnalysis::binding_id(body.params.first()?.pat)?;
        let formatter = AuthoredContractAnalysis::binding_id(body.params.get(1)?.pat)?;

        let expression = AuthoredContractAnalysis::peel_transparent(body.value);

        // Static display mappings are recognized only from an explicit variant match.
        let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
            return None;
        };

        // The match must dispatch on the formatter method's receiver.
        if !AuthoredContractAnalysis::is_local_path(cx, scrutinee, receiver) {
            return None;
        }

        let mut values = HashMap::new();

        for arm in arms {
            // Guarded arms cannot establish a complete variant mapping.
            if arm.guard.is_some() {
                return None;
            }
            let variant = AuthoredContractAnalysis::ignored_variant_pattern(cx, arm.pat)?;
            let value = AuthoredContractAnalysis::formatter_write_str(cx, arm.body, formatter)?;

            // Every arm must cover a distinct variant of the displayed enum.
            if AuthoredContractAnalysis::owning_enum(cx, variant) != Some(enum_def)
                || values.insert(variant, value).is_some()
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

// -----------------------------------------------------------------------------
// StringTableCandidate: Exact static variant-name tables
// -----------------------------------------------------------------------------

/// Static string table that may duplicate `VariantNames::VARIANTS`.
pub struct StringTableCandidate {
    /// Authored declaration or expression range used as the diagnostic anchor.
    pub(crate) span: Span,
    /// Declaration whose lint level governs this finding.
    pub(crate) owner: rustc_hir::HirId,
    /// Authored enum or variant name involved in the Strum contract.
    pub(super) name: Symbol,
    /// Local enum definition that owns the analyzed contract.
    pub(super) enum_def: Option<LocalDefId>,
    /// Distinct semantic values collected for this contract.
    pub(super) values: Vec<String>,
    /// Whether the declaration is visible outside its defining module.
    pub(crate) is_public: bool,
}

impl StringTableCandidate {
    /// Recovers this contract from one authored declaration.
    pub(crate) fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Macro-expanded constants are not reliable authored table evidence.
        if item.span.from_expansion() {
            return None;
        }

        // Only constants with an explicit body can be static name tables.
        let ItemKind::Const(_, _, _, ConstItemRhs::Body(body_id)) = item.kind else {
            return None;
        };
        let value = cx.tcx.hir_body(body_id).value;

        Some(Self {
            span: value.span.source_callsite(),
            owner: item.hir_id(),
            name: item.kind.ident()?.name,
            enum_def: None,
            values: AuthoredContractAnalysis::string_array(value)?,
            is_public: cx.tcx.visibility(item.owner_id.def_id).is_public(),
        })
    }

    /// Recovers an exhaustive static variant-name table.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Macro-expanded constants are not reliable authored table evidence.
        if item.span.from_expansion() {
            return None;
        }

        // Only implementation constants with a body can be static name tables.
        let ImplItemKind::Const(_, ConstItemRhs::Body(body_id)) = item.kind else {
            return None;
        };
        let value = cx.tcx.hir_body(body_id).value;

        Some(Self {
            span: value.span.source_callsite(),
            owner: item.hir_id(),
            name: item.ident.name,
            enum_def: AuthoredContractAnalysis::enclosing_inherent_enum(cx, item.hir_id()),
            values: AuthoredContractAnalysis::string_array(value)?,
            is_public: cx.tcx.visibility(item.owner_id.def_id).is_public(),
        })
    }
}

// -----------------------------------------------------------------------------
// StringParserCandidate: Exact string-to-variant parsers
// -----------------------------------------------------------------------------

/// Authored `FromStr` match over static string literals and unit variants.
pub struct StringParserCandidate {
    /// Authored declaration or expression range used as the diagnostic anchor.
    pub(crate) span: Span,
    /// Declaration whose lint level governs this finding.
    pub(crate) owner: rustc_hir::HirId,
    /// Local enum definition that owns the analyzed contract.
    pub(crate) enum_def: LocalDefId,
    /// Effective external names keyed by their declarations.
    pub(crate) names: HashMap<LocalDefId, Vec<String>>,
    /// Whether the declaration is visible outside its defining module.
    pub(crate) is_public: bool,
}

impl StringParserCandidate {
    /// Recovers an exhaustive unit-enum `FromStr` implementation.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Only authored `from_str` methods can implement the parser contract.
        if item.span.from_expansion() || item.ident.name.as_str() != "from_str" {
            return None;
        }

        // Only function items can provide the parser match body.
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };

        // `FromStr` accepts exactly one input string.
        if signature.decl.inputs.len() != 1 {
            return None;
        }
        let enum_def = AuthoredContractAnalysis::enclosing_trait_enum(
            cx,
            item.hir_id(),
            TraitIdentity {
                trait_name: "FromStr",
                crate_name: "core",
            },
        )?;
        let body = cx.tcx.hir_body(body_id);
        let input = AuthoredContractAnalysis::binding_id(body.params.first()?.pat)?;
        let expression = AuthoredContractAnalysis::peel_transparent(body.value);

        // Parser recognition requires a direct match over the input.
        let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
            return None;
        };

        // The parser must match directly on its input binding.
        if !AuthoredContractAnalysis::is_local_path(cx, scrutinee, input) {
            return None;
        }
        let mut names: HashMap<LocalDefId, Vec<String>> = HashMap::new();

        let mut fallback = false;

        for arm in arms {
            // Guarded arms cannot establish a complete parser table.
            if arm.guard.is_some() {
                return None;
            }
            if matches!(arm.pat.kind, PatKind::Wild)
                && AuthoredContractAnalysis::result_err(cx, arm.body)
            {
                fallback = true;
                continue;
            }
            let arm_names = AuthoredContractAnalysis::string_patterns(arm.pat)?;
            let variant = AuthoredContractAnalysis::result_ok_variant(cx, arm.body)?;

            // Successful arms must construct variants of the parsed enum.
            if AuthoredContractAnalysis::owning_enum(cx, variant) != Some(enum_def) {
                return None;
            }
            names.entry(variant).or_default().extend(arm_names);
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
// DiscriminantMirrorCandidate: Exact payload-to-unit enum mirrors
// -----------------------------------------------------------------------------

/// One-to-one conversion from a payload enum into a private unit mirror enum.
pub struct DiscriminantMirrorCandidate {
    /// Authored declaration or expression range used as the diagnostic anchor.
    pub(crate) span: Span,
    /// Declaration whose lint level governs this finding.
    pub(crate) owner: rustc_hir::HirId,
    /// Payload enum consumed by the conversion.
    pub(crate) source_enum: LocalDefId,
    /// Unit-only enum reproducing the source enum's variant set.
    pub(crate) mirror_enum: LocalDefId,
}

impl DiscriminantMirrorCandidate {
    /// Recovers a one-to-one conversion into a unit discriminant mirror.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Only authored `From::from` methods can establish a mirror conversion.
        if item.span.from_expansion() || item.ident.name.as_str() != "from" {
            return None;
        }

        // Only function items can provide the conversion match body.
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };

        // `From::from` accepts exactly one source value.
        if signature.decl.inputs.len() != 1 {
            return None;
        }
        let mirror_enum = AuthoredContractAnalysis::enclosing_trait_enum(
            cx,
            item.hir_id(),
            TraitIdentity {
                trait_name: "From",
                crate_name: "core",
            },
        )?;

        // Public mirrors may be part of the API and cannot be silently replaced.
        if cx.tcx.effective_visibilities(()).is_exported(mirror_enum) {
            return None;
        }

        let body = cx.tcx.hir_body(body_id);
        let input = AuthoredContractAnalysis::binding_id(body.params.first()?.pat)?;
        let expression = AuthoredContractAnalysis::peel_transparent(body.value);

        // Mirror recognition requires a direct match over the source value.
        let ExprKind::Match(scrutinee, arms, _) = expression.kind else {
            return None;
        };

        // The conversion must dispatch on its single source input.
        if !AuthoredContractAnalysis::is_local_path(cx, scrutinee, input) {
            return None;
        }
        let mut source_enum = None;
        let mut observed = HashSet::new();

        for arm in arms {
            // Guarded arms cannot establish a one-to-one mirror mapping.
            if arm.guard.is_some() {
                return None;
            }
            let source = AuthoredContractAnalysis::ignored_variant_pattern(cx, arm.pat)?;
            let target = AuthoredContractAnalysis::unit_variant_expression(cx, arm.body)?;
            let found_source_enum = AuthoredContractAnalysis::owning_enum(cx, source)?;

            // Each source must map once to an identically named mirror variant.
            if source_enum
                .replace(found_source_enum)
                .is_some_and(|known| known != found_source_enum)
                || AuthoredContractAnalysis::owning_enum(cx, target) != Some(mirror_enum)
                || cx.tcx.item_name(source.to_def_id()) != cx.tcx.item_name(target.to_def_id())
                || !observed.insert(source)
            {
                return None;
            }
        }

        let source_enum = source_enum?;
        let source = cx.tcx.adt_def(source_enum.to_def_id());
        let mirror = cx.tcx.adt_def(mirror_enum.to_def_id());

        // The conversion must cover every source and unit-only mirror variant exactly once.
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

// -----------------------------------------------------------------------------
// AuthoredContracts: Shared syntax and trait-resolution evidence
// -----------------------------------------------------------------------------

/// Identifies a trait by its defining crate and authored name.
#[derive(Clone, Copy)]
struct TraitIdentity<'name> {
    /// Authored trait name.
    trait_name: &'name str,
    /// Crate that defines the trait.
    crate_name: &'name str,
}

/// Resolves syntax and trait identities shared by authored text contracts.
struct AuthoredContractAnalysis;

impl AuthoredContractAnalysis {
    /// Recovers the string literal matched by a parser arm.
    fn string_patterns(pattern: &Pat<'_>) -> Option<Vec<String>> {
        // Alternative patterns must each resolve to supported string literals.
        if let PatKind::Or(patterns) = pattern.kind {
            return patterns
                .iter()
                .map(Self::string_patterns)
                .collect::<Option<Vec<_>>>()
                .map(|groups| groups.into_iter().flatten().collect());
        }

        // A parser key must be written as an expression pattern.
        let PatKind::Expr(expression) = pattern.kind else {
            return None;
        };

        // Only non-negated string literals are supported parser keys.
        let PatExprKind::Lit {
            lit,
            negated: false,
        } = expression.kind
        else {
            return None;
        };

        // Non-string literals cannot provide an authored list of serialized names.
        let rustc_ast::LitKind::Str(value, _) = lit.node else {
            return None;
        };
        Some(vec![value.as_str().to_owned()])
    }

    /// Returns whether a constructor resolves to the named standard `Result` variant.
    fn is_result_variant(cx: &LateContext<'_>, constructor: DefId, name: &str) -> bool {
        let variant = cx.tcx.parent(constructor);
        cx.tcx.item_name(variant).as_str() == name
            && cx
                .tcx
                .is_diagnostic_item(sym::Result, cx.tcx.parent(variant))
    }

    /// Resolves either a variant or its constructor to the local variant definition.
    fn variant_from_res(cx: &LateContext<'_>, resolution: Res) -> Option<LocalDefId> {
        match resolution {
            Res::Def(DefKind::Variant, variant) => variant.as_local(),
            Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) => {
                cx.tcx.opt_local_parent(constructor.as_local()?)
            }
            _ => None,
        }
    }

    /// Resolves a variant pattern whose payload is entirely ignored.
    fn ignored_variant_pattern(cx: &LateContext<'_>, pattern: &Pat<'_>) -> Option<LocalDefId> {
        let resolution = match pattern.kind {
            PatKind::Expr(expression) => {
                // A payload-free variant must be written as a direct path.
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

            // Other patterns can observe or bind payload data.
            _ => return None,
        };
        Self::variant_from_res(cx, resolution)
    }

    /// Returns the binding introduced by a plain, unqualified identifier pattern.
    const fn binding_id(pattern: &Pat<'_>) -> Option<rustc_hir::HirId> {
        // Generated-style contracts require an unmodified input binding.
        let PatKind::Binding(_, binding, _, None) = pattern.kind else {
            return None;
        };
        Some(binding)
    }

    /// Resolves the local enum targeted by the enclosing inherent implementation.
    fn enclosing_inherent_enum(
        cx: &LateContext<'_>,
        hir_id: rustc_hir::HirId,
    ) -> Option<LocalDefId> {
        cx.tcx.hir_parent_iter(hir_id).find_map(|(_, node)| {
            // Only item ancestors can contain an inherent implementation.
            let Node::Item(item) = node else { return None };

            // The candidate must be enclosed by an implementation item.
            let ItemKind::Impl(implementation) = item.kind else {
                return None;
            };

            // Trait implementations do not establish inherent generated methods.
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

    /// Resolves the local enum targeted by an enclosing implementation of a known trait.
    fn enclosing_trait_enum(
        cx: &LateContext<'_>,
        hir_id: rustc_hir::HirId,
        identity: TraitIdentity<'_>,
    ) -> Option<LocalDefId> {
        cx.tcx.hir_parent_iter(hir_id).find_map(|(_, node)| {
            // Only item ancestors can contain the requested trait implementation.
            let Node::Item(item) = node else { return None };

            // The candidate must be enclosed by an implementation item.
            let ItemKind::Impl(implementation) = item.kind else {
                return None;
            };
            let trait_def = implementation.of_trait?.trait_ref.trait_def_id()?;

            // Only the named standard trait establishes this generated contract.
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

    /// Resolves the local enum that owns a variant.
    fn owning_enum(cx: &LateContext<'_>, variant: LocalDefId) -> Option<LocalDefId> {
        cx.tcx.opt_local_parent(variant)
    }

    /// Removes expression wrappers that do not change value semantics.
    const fn peel_transparent<'hir>(mut expression: &'hir Expr<'hir>) -> &'hir Expr<'hir> {
        loop {
            expression = match expression.kind {
                ExprKind::Block(block, None) if block.stmts.is_empty() => {
                    // An empty block without a tail expression cannot be peeled further.
                    let Some(inner) = block.expr else {
                        return expression;
                    };
                    inner
                }
                ExprKind::DropTemps(inner) => inner,

                // Any other wrapper can change the expression's contract semantics.
                _ => return expression,
            };
        }
    }

    /// Resolves a direct variant constructor path.
    fn constructor_resolution(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<DefId> {
        let expression = Self::peel_transparent(expression);

        // Constructor resolution requires a direct path expression.
        let ExprKind::Path(path) = expression.kind else {
            return None;
        };

        // Only a resolved variant constructor can be a standard result constructor.
        let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
            cx.qpath_res(&path, expression.hir_id)
        else {
            return None;
        };
        Some(constructor)
    }

    /// Returns whether an expression constructs standard `Result::Err`.
    fn result_err(cx: &LateContext<'_>, expression: &Expr<'_>) -> bool {
        // `Err` recognition requires a one-argument constructor call.
        let ExprKind::Call(callee, [_]) = Self::peel_transparent(expression).kind else {
            return false;
        };
        Self::constructor_resolution(cx, callee)
            .is_some_and(|constructor| Self::is_result_variant(cx, constructor, "Err"))
    }

    /// Returns whether an expression is one direct local binding.
    fn is_local_path(
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
        binding: rustc_hir::HirId,
    ) -> bool {
        let expression = Self::peel_transparent(expression);

        // Only direct paths can be compared with the captured input binding.
        let ExprKind::Path(path) = expression.kind else {
            return false;
        };
        matches!(cx.qpath_res(&path, expression.hir_id), Res::Local(found) if found == binding)
    }

    /// Resolves a direct expression to a local unit variant.
    fn unit_variant_expression(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<LocalDefId> {
        let expression = Self::peel_transparent(expression);

        // A unit variant result must be a direct constructor path.
        let ExprKind::Path(path) = expression.kind else {
            return None;
        };
        Self::variant_from_res(cx, cx.qpath_res(&path, expression.hir_id))
    }

    /// Extracts a unit variant wrapped by standard `Result::Ok`.
    fn result_ok_variant(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<LocalDefId> {
        // `Ok` recognition requires a one-argument constructor call.
        let ExprKind::Call(callee, [value]) = Self::peel_transparent(expression).kind else {
            return None;
        };
        let constructor = Self::constructor_resolution(cx, callee)?;

        // The constructor must be standard `Result::Ok`.
        if !Self::is_result_variant(cx, constructor, "Ok") {
            return None;
        }
        Self::unit_variant_expression(cx, value)
    }

    /// Extracts a direct array of static string literals.
    fn string_array(expression: &Expr<'_>) -> Option<Vec<String>> {
        let expression = Self::peel_transparent(expression);
        let expression = if let ExprKind::AddrOf(_, _, inner) = expression.kind {
            Self::peel_transparent(inner)
        } else {
            expression
        };

        // Static name tables must be direct array expressions.
        let ExprKind::Array(elements) = expression.kind else {
            return None;
        };

        elements
            .iter()
            .map(|element| match StaticValue::from_expression(element)? {
                StaticValue::String(value) => Some(value),
                StaticValue::Bool(_) | StaticValue::Integer(_) => None,
            })
            .collect()
    }

    /// Resolves one direct `Formatter::write_str` call with static text.
    fn formatter_write_str(
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
        formatter: rustc_hir::HirId,
    ) -> Option<String> {
        let expression = Self::peel_transparent(expression);

        // Display mappings require a one-argument formatter method call.
        let ExprKind::MethodCall(segment, receiver, [value], _) = expression.kind else {
            return None;
        };

        // The call must invoke `write_str` on the captured formatter binding.
        if segment.ident.name.as_str() != "write_str"
            || !Self::is_local_path(cx, receiver, formatter)
        {
            return None;
        }

        match StaticValue::from_expression(value)? {
            StaticValue::String(value) => Some(value),
            StaticValue::Bool(_) | StaticValue::Integer(_) => None,
        }
    }
}
