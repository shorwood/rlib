extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use std::num::TryFromIntError;

use rustc_hir::def::{CtorOf, DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{
    BodyId, ConstItemRhs, Expr, ExprKind, ImplItem, ImplItemKind, Item, ItemKind, Node,
};
use rustc_lint::LateContext;
use rustc_span::{Span, Symbol};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
/// Provider values accepted by the enum collection conflict knob.
#[serde(rename_all = "snake_case")]
pub enum CollectionProvider {
    /// Represents the `StrumEnumIter` case.
    StrumEnumIter,
    /// Represents the `StrumVariantArray` case.
    StrumVariantArray,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Authored API shape reproduced by a Strum enumeration derive.
pub enum CollectionSurface {
    /// Represents the `Static` case.
    Static,
    /// Represents the `Iterator` case.
    Iterator,
}

/// Performs the `strum_derives_available` step of the lint analysis.
fn strum_derives_available(cx: &LateContext<'_>) -> bool {
    cx.tcx.sess.opts.externs.get("strum").is_some()
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
            ExprKind::AddrOf(_, _, inner) | ExprKind::DropTemps(inner) => inner,
            _ => return expression,
        };
    }
}

/// Performs the `forwarded_body` step of the lint analysis.
fn forwarded_body(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<BodyId> {
    // Prepare the values used by this stage.
    let definition = match expression.kind {
        ExprKind::Path(path) => match cx.qpath_res(&path, expression.hir_id) {
            Res::Def(
                DefKind::Const { .. } | DefKind::AssocConst { .. } | DefKind::Static { .. },
                definition,
            ) => definition.as_local()?,
            _ => return None,
        },
        ExprKind::Call(callee, []) => {
            let ExprKind::Path(path) = peel_transparent(callee).kind else {
                return None;
            };
            match cx.qpath_res(&path, callee.hir_id) {
                Res::Def(DefKind::Fn | DefKind::AssocFn, definition) => definition.as_local()?,
                _ => return None,
            }
        }
        _ => return None,
    };

    // Classify the current analyze_candidate.
    match cx.tcx.hir_node_by_def_id(definition) {
        Node::Item(item) => match item.kind {
            ItemKind::Const(_, _, _, ConstItemRhs::Body(body))
            | ItemKind::Static(_, _, _, body)
            | ItemKind::Fn { body, .. } => Some(body),
            _ => None,
        },
        Node::ImplItem(item) => match item.kind {
            ImplItemKind::Const(_, ConstItemRhs::Body(body)) | ImplItemKind::Fn(_, body) => {
                Some(body)
            }
            _ => None,
        },
        _ => None,
    }
}

/// Performs the `unit_variant` step of the lint analysis.
fn unit_variant(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<LocalDefId> {
    let ExprKind::Path(path) = peel_transparent(expression).kind else {
        return None;
    };
    let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
        cx.qpath_res(&path, expression.hir_id)
    else {
        return None;
    };
    cx.tcx.opt_local_parent(constructor.as_local()?)
}

/// Carries the `VariantCollector` state used by this analysis.
struct VariantCollector<'cx, 'tcx> {
    /// Stores the `cx` value used by this analysis.
    cx: &'cx LateContext<'tcx>,
    /// Stores the `variants` value used by this analysis.
    variants: Vec<LocalDefId>,
    /// Stores the `is_invalid` value used by this analysis.
    is_invalid: bool,
}

impl<'cx, 'tcx> VariantCollector<'cx, 'tcx> {
    /// Performs the `new` operation for this value.
    const fn new(cx: &'cx LateContext<'tcx>) -> Self {
        Self {
            cx,
            variants: Vec::new(),
            is_invalid: false,
        }
    }
}

impl<'tcx> Visitor<'tcx> for VariantCollector<'_, 'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let Some(variant) = unit_variant(self.cx, expression) {
            self.variants.push(variant);
            return;
        }
        if !expression.span.from_expansion() {
            self.is_invalid = true;
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

/// Performs the `owning_enum` step of the lint analysis.
fn owning_enum(cx: &LateContext<'_>, variant: DefId) -> Option<LocalDefId> {
    cx.tcx.opt_local_parent(variant.as_local()?)
}

/// Performs the `enum_has_strum_variant_attributes` step of the lint analysis.
fn enum_has_strum_variant_attributes(cx: &LateContext<'_>, enum_def: LocalDefId) -> bool {
    let Node::Item(item) = cx.tcx.hir_node_by_def_id(enum_def) else {
        return true;
    };
    let ItemKind::Enum(_, _, definition) = item.kind else {
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

/// Performs the `eligible_unit_variants` step of the lint analysis.
fn eligible_unit_variants(cx: &LateContext<'_>, enum_def: LocalDefId) -> Option<Vec<LocalDefId>> {
    // Prepare the values used by this stage.
    let definition = cx.tcx.adt_def(enum_def.to_def_id());
    if !definition.is_enum()
        || definition
            .variants()
            .iter()
            .any(|variant| !variant.fields.is_empty())
        || enum_has_strum_variant_attributes(cx, enum_def)
    {
        return None;
    }

    // Return the completed analysis result.
    Some(
        definition
            .variants()
            .iter()
            .filter_map(|variant| variant.def_id.as_local())
            .collect(),
    )
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

/// Performs the `enclosing_into_iterator_enum` step of the lint analysis.
fn enclosing_into_iterator_enum(
    cx: &LateContext<'_>,
    hir_id: rustc_hir::HirId,
) -> Option<LocalDefId> {
    cx.tcx.hir_parent_iter(hir_id).find_map(|(_, node)| {
        let Node::Item(item) = node else {
            return None;
        };
        let ItemKind::Impl(implementation) = item.kind else {
            return None;
        };
        let implementation_header = implementation.of_trait?;
        let Res::Def(DefKind::Trait, trait_definition) = implementation_header.trait_ref.path.res
        else {
            return None;
        };
        if cx.tcx.item_name(trait_definition).as_str() != "IntoIterator"
            || cx.tcx.crate_name(trait_definition.krate).as_str() != "core"
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

/// Performs the `integer_literal` step of the lint analysis.
fn integer_literal(expression: &Expr<'_>) -> Result<Option<usize>, TryFromIntError> {
    // Prepare the values used by this stage.
    let ExprKind::Lit(literal) = peel_transparent(expression).kind else {
        return Ok(None);
    };
    let rustc_ast::LitKind::Int(value, _) = literal.node else {
        return Ok(None);
    };

    // Classify the current analyze_candidate.
    usize::try_from(value.get()).map(Some)
}

/// Exact authored total count associated with one enum.
pub struct CountCandidate {
    /// Stores the `span` value used by this analysis.
    pub(crate) span: Span,
    /// Stores the `owner` value used by this analysis.
    pub(crate) owner: rustc_hir::HirId,
    /// Stores the `enum_def` value used by this analysis.
    enum_def: LocalDefId,
    /// Stores the `is_public_api` value used by this analysis.
    is_public_api: bool,
}

impl CountCandidate {
    /// Performs the `from_impl_item` operation for this value.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() || !strum_derives_available(cx) {
            return None;
        }

        // Prepare the values used by this stage.
        let body = match item.kind {
            ImplItemKind::Const(_, ConstItemRhs::Body(body)) => body,
            ImplItemKind::Fn(signature, body) if signature.decl.inputs.is_empty() => body,
            _ => return None,
        };
        let enum_def = enclosing_inherent_enum(cx, item.hir_id())?;
        let count = match integer_literal(cx.tcx.hir_body(body).value) {
            Ok(Some(count)) => count,
            Ok(None) => return None,
            Err(_error) => return None,
        };

        // Reject inputs that do not satisfy this stage.
        if enum_has_strum_variant_attributes(cx, enum_def) {
            return None;
        }
        let expected = cx.tcx.adt_def(enum_def.to_def_id()).variants().len();

        // Perform the next step of the analysis.
        (count == expected).then(|| Self {
            span: cx.tcx.hir_body(body).value.span,
            owner: item.hir_id(),
            enum_def,
            is_public_api: cx.tcx.visibility(item.owner_id.def_id).is_public(),
        })
    }

    /// Performs the `is_public_api` operation for this value.
    pub(crate) const fn is_public_api(&self) -> bool {
        self.is_public_api
    }

    /// Performs the `enum_name` operation for this value.
    pub(crate) fn enum_name(&self, cx: &LateContext<'_>) -> Symbol {
        cx.tcx.item_name(self.enum_def.to_def_id())
    }
}

/// Performs the `is_vec_expansion` step of the lint analysis.
fn is_vec_expansion(cx: &LateContext<'_>, span: Span) -> bool {
    span.macro_backtrace().any(|expansion| {
        expansion.macro_def_id.is_some_and(|definition| {
            cx.tcx.item_name(definition).as_str() == "vec"
                && cx.tcx.crate_name(definition.krate).as_str() == "alloc"
        })
    })
}

/// Carries one resolved variant sequence and its collection surface.
struct VariantSequence {
    /// Variants in the authored collection order.
    variants: Vec<LocalDefId>,
    /// Collection interface presented by the authored code.
    surface: CollectionSurface,
    /// Authored source span covering the collection.
    span: Span,
}

impl VariantSequence {
    /// Resolves a variant sequence through transparent collection helpers.
    fn resolve<'tcx>(
        cx: &LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
        declared_surface: CollectionSurface,
        forwarding_depth: usize,
    ) -> Option<Self> {
        /// Maximum helper-call depth followed while resolving a variant sequence.
        const MAXIMUM_FORWARDING_DEPTH: usize = 4;

        // Reject inputs that do not satisfy this stage.
        if forwarding_depth > MAXIMUM_FORWARDING_DEPTH {
            return None;
        }
        let expression = peel_transparent(expression);

        // Reject inputs that do not satisfy this stage.
        if let ExprKind::Array(elements) = expression.kind {
            // Prepare the values used by this stage.
            let variants = elements
                .iter()
                .map(|element| unit_variant(cx, element))
                .collect::<Option<Vec<_>>>()?;

            // Return the completed analysis result.
            return Some(Self {
                variants,
                surface: declared_surface,
                span: expression.span.source_callsite(),
            });
        }

        // Reject inputs that do not satisfy this stage.
        if let ExprKind::MethodCall(segment, receiver, arguments, _) = expression.kind
            && arguments.is_empty()
            && matches!(
                segment.ident.name.as_str(),
                "iter" | "into_iter" | "copied" | "cloned" | "collect"
            )
        {
            let sequence =
                Self::resolve(cx, receiver, CollectionSurface::Iterator, forwarding_depth)?;
            return Some(Self {
                variants: sequence.variants,
                surface: CollectionSurface::Iterator,
                span: sequence.span,
            });
        }

        // Reject inputs that do not satisfy this stage.
        if let Some(body) = forwarded_body(cx, expression) {
            let sequence = Self::resolve(
                cx,
                cx.tcx.hir_body(body).value,
                declared_surface,
                forwarding_depth + 1,
            )?;
            return Some(Self {
                variants: sequence.variants,
                surface: sequence.surface,
                span: expression.span.source_callsite(),
            });
        }

        // Reject inputs that do not satisfy this stage.
        if is_vec_expansion(cx, expression.span) {
            let mut collector = VariantCollector::new(cx);
            collector.visit_expr(expression);
            if !collector.is_invalid && !collector.variants.is_empty() {
                return Some(Self {
                    variants: collector.variants,
                    surface: declared_surface,
                    span: expression.span.source_callsite(),
                });
            }
        }

        // Perform the next step of the analysis.
        None
    }
}

/// Exact exhaustive unit-enum collection found in authored code.
pub struct CollectionCandidate {
    /// Stores the `span` value used by this analysis.
    pub(crate) span: Span,
    /// Stores the `owner` value used by this analysis.
    pub(crate) owner: rustc_hir::HirId,
    /// Stores the `enum_def` value used by this analysis.
    enum_def: LocalDefId,
    /// Stores the `surface` value used by this analysis.
    surface: CollectionSurface,
    /// Stores the `is_public_api` value used by this analysis.
    is_public_api: bool,
}

impl CollectionCandidate {
    /// Performs the `from_item` operation for this value.
    pub(crate) fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() {
            return None;
        }

        // Prepare the values used by this stage.
        let (body, surface) = match item.kind {
            ItemKind::Const(_, _, _, ConstItemRhs::Body(body))
            | ItemKind::Static(_, _, _, body) => (body, CollectionSurface::Static),
            ItemKind::Fn { sig, body, .. } if sig.decl.inputs.is_empty() => {
                (body, CollectionSurface::Iterator)
            }
            _ => return None,
        };

        // Perform the next step of the analysis.
        Self::from_body(cx, body, item.hir_id(), surface, item.owner_id.def_id)
    }

    /// Performs the `from_impl_item` operation for this value.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Reject inputs that do not satisfy this stage.
        if item.span.from_expansion() {
            return None;
        }
        let into_iterator_enum = enclosing_into_iterator_enum(cx, item.hir_id());

        // Prepare the values used by this stage.
        let (body, surface) = match item.kind {
            ImplItemKind::Const(_, ConstItemRhs::Body(body)) => (body, CollectionSurface::Static),
            ImplItemKind::Fn(signature, body) if signature.decl.inputs.is_empty() => {
                (body, CollectionSurface::Iterator)
            }
            ImplItemKind::Fn(signature, body)
                if signature.decl.inputs.len() == 1 && into_iterator_enum.is_some() =>
            {
                (body, CollectionSurface::Iterator)
            }
            _ => return None,
        };

        // Prepare the values used by this stage.
        let analyze_candidate =
            Self::from_body(cx, body, item.hir_id(), surface, item.owner_id.def_id)?;
        if into_iterator_enum.is_some_and(|enum_def| enum_def != analyze_candidate.enum_def) {
            return None;
        }
        Some(analyze_candidate)
    }

    /// Performs the `from_body` operation for this value.
    fn from_body(
        cx: &LateContext<'_>,
        body_id: BodyId,
        owner: rustc_hir::HirId,
        declared_surface: CollectionSurface,
        definition: LocalDefId,
    ) -> Option<Self> {
        // Reject inputs that do not satisfy this stage.
        if !strum_derives_available(cx) {
            return None;
        }
        let expression = cx.tcx.hir_body(body_id).value;
        let VariantSequence {
            variants,
            surface,
            span,
        } = VariantSequence::resolve(cx, expression, declared_surface, 0)?;
        let enum_def = owning_enum(cx, variants.first()?.to_def_id())?;

        // Reject inputs that do not satisfy this stage.
        if variants
            .iter()
            .any(|variant| owning_enum(cx, variant.to_def_id()) != Some(enum_def))
        {
            return None;
        }
        let expected = eligible_unit_variants(cx, enum_def)?;

        // Reject inputs that do not satisfy this stage.
        if variants != expected {
            return None;
        }

        // Return the completed analysis result.
        Some(Self {
            span,
            owner,
            enum_def,
            surface,
            is_public_api: cx.tcx.visibility(definition).is_public(),
        })
    }

    /// Performs the `providers` operation for this value.
    pub(crate) const fn providers(&self) -> &'static [CollectionProvider] {
        match self.surface {
            CollectionSurface::Static => &[
                CollectionProvider::StrumEnumIter,
                CollectionProvider::StrumVariantArray,
            ],
            CollectionSurface::Iterator => &[CollectionProvider::StrumEnumIter],
        }
    }

    /// Performs the `selected` operation for this value.
    pub(crate) fn selected(
        &self,
        configured: Option<CollectionProvider>,
    ) -> Option<CollectionProvider> {
        let providers = self.providers();
        if providers.len() == 1 {
            return providers.first().copied();
        }
        configured.filter(|provider| providers.contains(provider))
    }

    /// Performs the `is_public_api` operation for this value.
    pub(crate) const fn is_public_api(&self) -> bool {
        self.is_public_api
    }

    /// Performs the `enum_name` operation for this value.
    pub(crate) fn enum_name(&self, cx: &LateContext<'_>) -> Symbol {
        cx.tcx.item_name(self.enum_def.to_def_id())
    }
}
