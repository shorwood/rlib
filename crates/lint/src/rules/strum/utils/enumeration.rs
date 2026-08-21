extern crate rustc_ast;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use std::num::TryFromIntError;

use rustc_hir::def::{CtorOf, DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{
    BodyId, ConstItemRhs, Expr, ExprKind, ImplItem, ImplItemKind, Item, ItemKind, Node,
};
use rustc_lint::LateContext;
use rustc_middle::ty;
use rustc_span::{Span, Symbol};

use crate::utils::variant_methods::VariantMetadata;

/// Maximum helper-call depth followed while resolving a variant sequence.
const MAXIMUM_FORWARDING_DEPTH: usize = 4;

// -----------------------------------------------------------------------------
// Collection: Configured generated API surface
// -----------------------------------------------------------------------------

use crate::config::providers::CollectionProvider;

/// Authored API shape reproduced by a Strum enumeration derive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionSurface {
    /// A fixed array or slice containing every variant.
    Static,
    /// An `IntoIterator` implementation yielding every variant.
    Iterator,
}

impl CollectionSurface {
    /// Classifies a resolved output type as static or iterator-like.
    fn from_output(output: ty::Ty<'_>) -> Self {
        if matches!(output.kind(), ty::Array(..) | ty::Slice(_))
            || matches!(output.kind(), ty::Ref(_, inner, _) if matches!(inner.kind(), ty::Array(..) | ty::Slice(_)))
        {
            Self::Static
        } else {
            Self::Iterator
        }
    }

    /// Returns whether the collection is exposed as a fixed array or slice.
    const fn is_static(self) -> bool {
        matches!(self, Self::Static)
    }
}

/// Resolves authored enum sequences and their surrounding API contracts.
struct CollectionExpressionAnalysis;

impl CollectionExpressionAnalysis {
    /// Returns whether the crate can use Strum derives as a remediation.
    fn has_available_strum_derives(cx: &LateContext<'_>) -> bool {
        cx.tcx.sess.opts.externs.get("strum").is_some()
    }

    /// Classifies fixed array/slice outputs separately from iterator-like outputs.
    fn output_surface(cx: &LateContext<'_>, definition: LocalDefId) -> CollectionSurface {
        let output = cx
            .tcx
            .fn_sig(definition)
            .instantiate_identity()
            .skip_binder()
            .output();
        CollectionSurface::from_output(output)
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
                ExprKind::AddrOf(_, _, inner) | ExprKind::DropTemps(inner) => inner,

                // Any other wrapper can change the collection expression's meaning.
                _ => return expression,
            };
        }
    }

    /// Resolves a directly referenced constant, static, or zero-argument function body.
    fn forwarded_body(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<BodyId> {
        let definition = match expression.kind {
            ExprKind::Path(path) => match cx.qpath_res(&path, expression.hir_id) {
                Res::Def(
                    DefKind::Const { .. } | DefKind::AssocConst { .. } | DefKind::Static { .. },
                    definition,
                ) => definition.as_local()?,

                // Only directly referenced local storage can forward a collection body.
                _ => return None,
            },
            ExprKind::Call(callee, []) => {
                // Forwarded helper calls must name a direct function path.
                let ExprKind::Path(path) = Self::peel_transparent(callee).kind else {
                    return None;
                };
                match cx.qpath_res(&path, callee.hir_id) {
                    Res::Def(DefKind::Fn | DefKind::AssocFn, definition) => {
                        definition.as_local()?
                    }

                    // Other resolutions cannot provide a local forwarding body.
                    _ => return None,
                }
            }

            // Only direct references and zero-argument calls can forward a body.
            _ => return None,
        };

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

    /// Resolves an expression that names one local unit variant.
    fn unit_variant(cx: &LateContext<'_>, expression: &Expr<'_>) -> Option<LocalDefId> {
        // Unit variants must be written as direct constructor paths.
        let ExprKind::Path(path) = Self::peel_transparent(expression).kind else {
            return None;
        };

        // Only a resolved variant constructor can identify a unit variant.
        let Res::Def(DefKind::Ctor(CtorOf::Variant, _), constructor) =
            cx.qpath_res(&path, expression.hir_id)
        else {
            return None;
        };
        cx.tcx.opt_local_parent(constructor.as_local()?)
    }
}

// -----------------------------------------------------------------------------
// VariantCollector: Exact unit-variant sequence recovery
// -----------------------------------------------------------------------------

/// Collects direct unit-variant expressions in authored order.
struct VariantCollector<'cx, 'tcx> {
    /// Compiler context used to resolve each expression to its variant definition.
    cx: &'cx LateContext<'tcx>,
    /// Variants participating in the analyzed contract.
    variants: Vec<LocalDefId>,
    /// Whether authored runtime logic prevents this sequence from being an exact table.
    is_invalid: bool,
}

impl<'cx, 'tcx> VariantCollector<'cx, 'tcx> {
    /// Starts ordered variant recovery for the active compiler context.
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
        // A resolved unit variant is a complete leaf in the collected sequence.
        if let Some(variant) = CollectionExpressionAnalysis::unit_variant(self.cx, expression) {
            self.variants.push(variant);
            return;
        }

        // Authored non-variant expressions invalidate an otherwise exact sequence.
        if !expression.span.from_expansion() {
            self.is_invalid = true;
            return;
        }
        intravisit::walk_expr(self, expression);
    }
}

// -----------------------------------------------------------------------------
// GeneratedVariantSets: Provider-specific membership
// -----------------------------------------------------------------------------

/// Variant sequences generated by Strum's two collection providers.
struct GeneratedVariantSets {
    /// Sequence exposed by `VariantArray` when the enum shape permits it.
    variant_array: Option<Vec<LocalDefId>>,
    /// Sequence exposed by `EnumIter` after disabled variants are removed.
    enum_iter: Option<Vec<LocalDefId>>,
}

// -----------------------------------------------------------------------------
// EnumCollectionAnalysis: Enum membership and API ownership
// -----------------------------------------------------------------------------

/// Resolves enum membership and the API surface that owns it.
struct EnumCollectionAnalysis;

impl EnumCollectionAnalysis {
    /// Resolves the local enum that owns a variant.
    fn owning_enum(cx: &LateContext<'_>, variant: DefId) -> Option<LocalDefId> {
        cx.tcx.opt_local_parent(variant.as_local()?)
    }

    /// Returns the exact variant sequences generated by each collection provider.
    fn generated_variant_sets(
        cx: &LateContext<'_>,
        enum_def: LocalDefId,
    ) -> Option<GeneratedVariantSets> {
        // Generated membership can be recovered only from a local enum item.
        let Node::Item(item) = cx.tcx.hir_node_by_def_id(enum_def) else {
            return None;
        };

        // Only enum items expose the variants used by collection derives.
        let ItemKind::Enum(_, _, hir_definition) = item.kind else {
            return None;
        };
        let definition = cx.tcx.adt_def(enum_def.to_def_id());

        let variant_array = definition
            .variants()
            .iter()
            .all(|variant| variant.fields.is_empty())
            .then(|| {
                hir_definition
                    .variants
                    .iter()
                    .map(|variant| variant.def_id)
                    .collect()
            });
        let enum_iter = hir_definition
            .variants
            .iter()
            .filter(|variant| !VariantMetadata::is_strum_disabled(cx, variant.hir_id))
            .map(|variant| {
                let definition = definition
                    .variants()
                    .iter()
                    .find(|definition| definition.def_id.as_local() == Some(variant.def_id))?;
                definition.fields.is_empty().then_some(variant.def_id)
            })
            .collect::<Option<Vec<_>>>();

        Some(GeneratedVariantSets {
            variant_array,
            enum_iter,
        })
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

            // Trait implementations do not establish inherent collection methods.
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

    /// Resolves the local enum targeted by an enclosing `IntoIterator` implementation.
    fn enclosing_into_iterator_enum(
        cx: &LateContext<'_>,
        hir_id: rustc_hir::HirId,
    ) -> Option<LocalDefId> {
        cx.tcx.hir_parent_iter(hir_id).find_map(|(_, node)| {
            // Only item ancestors can contain the requested trait implementation.
            let Node::Item(item) = node else {
                return None;
            };

            // The candidate must be enclosed by an implementation item.
            let ItemKind::Impl(implementation) = item.kind else {
                return None;
            };
            let implementation_header = implementation.of_trait?;

            // The trait reference must resolve before its collection surface can be checked.
            let Res::Def(DefKind::Trait, trait_definition) =
                implementation_header.trait_ref.path.res
            else {
                return None;
            };

            // Only core's `IntoIterator` implementation supplies this collection surface.
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

    /// Reads a non-negative integer literal without accepting computed values.
    fn integer_literal(expression: &Expr<'_>) -> Result<Option<usize>, TryFromIntError> {
        // Cardinality recognition requires a literal expression.
        let ExprKind::Lit(literal) =
            CollectionExpressionAnalysis::peel_transparent(expression).kind
        else {
            return Ok(None);
        };

        // Only integer literals can encode an enum cardinality.
        let rustc_ast::LitKind::Int(value, _) = literal.node else {
            return Ok(None);
        };

        usize::try_from(value.get()).map(Some)
    }
}

// -----------------------------------------------------------------------------
// CountCandidate: Exact authored enum cardinality
// -----------------------------------------------------------------------------

/// Exact authored total count associated with one enum.
pub struct CountCandidate {
    /// Authored declaration or expression range used as the diagnostic anchor.
    pub(crate) span: Span,
    /// Declaration whose lint level governs this finding.
    pub(crate) owner: rustc_hir::HirId,
    /// Local enum definition that owns the analyzed contract.
    enum_def: LocalDefId,
    /// Whether replacement would change a public API.
    is_public_api: bool,
}

impl CountCandidate {
    /// Recovers a constant or method that returns the enum's exact cardinality.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Replacement requires Strum derives to be available in the current crate.
        if item.span.from_expansion()
            || !CollectionExpressionAnalysis::has_available_strum_derives(cx)
        {
            return None;
        }

        let components = item
            .ident
            .name
            .as_str()
            .trim_start_matches("r#")
            .split('_')
            .map(str::to_ascii_lowercase)
            .collect::<Vec<_>>();

        // The declaration name must indicate a cardinality API.
        if !components.iter().any(|component| {
            matches!(
                component.as_str(),
                "count" | "cardinality" | "len" | "length"
            )
        }) {
            return None;
        }

        // Accept a usize constant or zero-argument function as the manual cardinality surface.
        let body = Self::cardinality_body(cx, item)?;

        // Resolve the enclosing enum and the literal cardinality returned by this surface.
        let enum_def = EnumCollectionAnalysis::enclosing_inherent_enum(cx, item.hir_id())?;
        let count = match EnumCollectionAnalysis::integer_literal(cx.tcx.hir_body(body).value) {
            Ok(Some(count)) => count,

            // A non-literal value cannot prove exact cardinality.
            Ok(None) => return None,

            // An unrepresentable literal cannot prove exact cardinality.
            Err(_error) => return None,
        };

        // The candidate must resolve to the local enum item being counted.
        let Node::Item(enum_item) = cx.tcx.hir_node_by_def_id(enum_def) else {
            return None;
        };

        // Only enum definitions expose the variants used for the count.
        let ItemKind::Enum(_, _, definition) = enum_item.kind else {
            return None;
        };
        let expected = definition
            .variants
            .iter()
            .filter(|variant| !VariantMetadata::is_strum_disabled(cx, variant.hir_id))
            .count();

        (count == expected).then(|| Self {
            span: cx.tcx.hir_body(body).value.span,
            owner: item.hir_id(),
            enum_def,
            is_public_api: cx.tcx.visibility(item.owner_id.def_id).is_public(),
        })
    }

    /// Resolves the body of a usize constant or zero-argument usize function.
    fn cardinality_body(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<BodyId> {
        match item.kind {
            ImplItemKind::Const(_, ConstItemRhs::Body(body))
                if cx
                    .tcx
                    .type_of(item.owner_id)
                    .instantiate_identity()
                    .is_usize() =>
            {
                Some(body)
            }
            ImplItemKind::Fn(signature, body)
                if signature.decl.inputs.is_empty()
                    && !signature.header.is_unsafe()
                    && !signature.header.is_async()
                    && cx
                        .tcx
                        .generics_of(item.owner_id.def_id)
                        .own_params
                        .is_empty()
                    && cx
                        .tcx
                        .fn_sig(item.owner_id.def_id)
                        .instantiate_identity()
                        .skip_binder()
                        .output()
                        .is_usize() =>
            {
                Some(body)
            }
            _ => None,
        }
    }

    /// Returns whether replacement would alter a public API.
    pub(crate) const fn is_public_api(&self) -> bool {
        self.is_public_api
    }

    /// Resolves the owning enum name used in diagnostics.
    pub(crate) fn enum_name(&self, cx: &LateContext<'_>) -> Symbol {
        cx.tcx.item_name(self.enum_def.to_def_id())
    }
}

// -----------------------------------------------------------------------------
// VariantSequence: Resolved authored variant sequence
// -----------------------------------------------------------------------------

/// Resolved enum variants paired with the collection interface that exposes them.
struct VariantSequence {
    /// Variants in the authored collection order.
    variants: Vec<LocalDefId>,
    /// Collection interface presented by the authored code.
    surface: CollectionSurface,
    /// Whether iteration yields enum values rather than references.
    has_owned_yield: bool,
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
        // Bounded resolution prevents cyclic or excessively indirect helper analysis.
        if forwarding_depth > MAXIMUM_FORWARDING_DEPTH {
            return None;
        }
        let expression = CollectionExpressionAnalysis::peel_transparent(expression);

        // Arrays are complete owned sequences without additional iterator adaptation.
        if let ExprKind::Array(elements) = expression.kind {
            let variants = elements
                .iter()
                .map(|element| CollectionExpressionAnalysis::unit_variant(cx, element))
                .collect::<Option<Vec<_>>>()?;

            return Some(Self {
                variants,
                surface: declared_surface,
                has_owned_yield: true,
                span: expression.span.source_callsite(),
            });
        }

        // Recognized adapter methods preserve a forwarded iterator sequence.
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
                has_owned_yield: match segment.ident.name.as_str() {
                    "iter" => false,
                    "copied" | "cloned" => true,
                    "into_iter" | "collect" => sequence.has_owned_yield,
                    _ => unreachable!(),
                },
                span: sequence.span,
            });
        }

        // A resolved helper body can contribute the same collection contract.
        if let Some(body) = CollectionExpressionAnalysis::forwarded_body(cx, expression) {
            let sequence = Self::resolve(
                cx,
                cx.tcx.hir_body(body).value,
                declared_surface,
                forwarding_depth + 1,
            )?;
            return Some(Self {
                variants: sequence.variants,
                surface: sequence.surface,
                has_owned_yield: sequence.has_owned_yield,
                span: expression.span.source_callsite(),
            });
        }

        if Self::is_vec_expansion(cx, expression.span) {
            let mut collector = VariantCollector::new(cx);
            collector.visit_expr(expression);

            // Macro expansion is usable only when it yields a nonempty pure variant sequence.
            if !collector.is_invalid && !collector.variants.is_empty() {
                return Some(Self {
                    variants: collector.variants,
                    surface: declared_surface,
                    has_owned_yield: true,
                    span: expression.span.source_callsite(),
                });
            }
        }

        None
    }

    /// Returns whether a collection expression was emitted by the standard `vec!` macro.
    fn is_vec_expansion(cx: &LateContext<'_>, span: Span) -> bool {
        span.macro_backtrace().any(|expansion| {
            expansion.macro_def_id.is_some_and(|definition| {
                cx.tcx.item_name(definition).as_str() == "vec"
                    && cx.tcx.crate_name(definition.krate).as_str() == "alloc"
            })
        })
    }
}

// -----------------------------------------------------------------------------
// CollectionCandidate: Complete authored enum collections
// -----------------------------------------------------------------------------

/// Exact exhaustive unit-enum collection found in authored code.
pub struct CollectionCandidate {
    /// Authored declaration or expression range used as the diagnostic anchor.
    pub(crate) span: Span,
    /// Declaration whose lint level governs this finding.
    pub(crate) owner: rustc_hir::HirId,
    /// Local enum definition that owns the analyzed contract.
    enum_def: LocalDefId,
    /// Providers whose generated membership and surface match this collection.
    providers: Vec<CollectionProvider>,
    /// Whether replacement would change a public API.
    is_public_api: bool,
}

impl CollectionCandidate {
    /// Recovers this contract from one authored declaration.
    pub(crate) fn from_item(cx: &LateContext<'_>, item: &Item<'_>) -> Option<Self> {
        // Macro-expanded declarations are not reliable authored collection evidence.
        if item.span.from_expansion() {
            return None;
        }

        let (body, surface) = match item.kind {
            ItemKind::Const(_, _, _, ConstItemRhs::Body(body))
            | ItemKind::Static(_, _, _, body) => (body, CollectionSurface::Static),
            ItemKind::Fn { sig, body, .. } if sig.decl.inputs.is_empty() => (
                body,
                CollectionExpressionAnalysis::output_surface(cx, item.owner_id.def_id),
            ),

            // Only static storage or zero-input functions can expose a collection contract.
            _ => return None,
        };

        Self::from_body(cx, body, item.hir_id(), surface, item.owner_id.def_id)
    }

    /// Recovers a static collection or iterator that yields every variant in order.
    pub(crate) fn from_impl_item(cx: &LateContext<'_>, item: &ImplItem<'_>) -> Option<Self> {
        // Macro-expanded implementation items are not authored collection evidence.
        if item.span.from_expansion() {
            return None;
        }
        let into_iterator_enum =
            EnumCollectionAnalysis::enclosing_into_iterator_enum(cx, item.hir_id());

        let (body, surface) = match item.kind {
            ImplItemKind::Const(_, ConstItemRhs::Body(body)) => (body, CollectionSurface::Static),
            ImplItemKind::Fn(signature, body) if signature.decl.inputs.is_empty() => (
                body,
                CollectionExpressionAnalysis::output_surface(cx, item.owner_id.def_id),
            ),
            ImplItemKind::Fn(signature, body)
                if signature.decl.inputs.len() == 1 && into_iterator_enum.is_some() =>
            {
                (body, CollectionSurface::Iterator)
            }

            // Other implementation items do not expose a supported collection surface.
            _ => return None,
        };

        let candidate = Self::from_body(cx, body, item.hir_id(), surface, item.owner_id.def_id)?;

        // An iterator implementation must enumerate the enum targeted by that trait impl.
        if into_iterator_enum.is_some_and(|enum_def| enum_def != candidate.enum_def) {
            return None;
        }
        Some(candidate)
    }

    /// Recovers a complete ordered variant sequence from one function body.
    fn from_body(
        cx: &LateContext<'_>,
        body_id: BodyId,
        owner: rustc_hir::HirId,
        declared_surface: CollectionSurface,
        definition: LocalDefId,
    ) -> Option<Self> {
        // Collection replacement requires Strum derives to be available.
        if !CollectionExpressionAnalysis::has_available_strum_derives(cx) {
            return None;
        }
        let expression = cx.tcx.hir_body(body_id).value;
        let VariantSequence {
            variants,
            surface,
            has_owned_yield: yields_owned,
            span,
        } = VariantSequence::resolve(cx, expression, declared_surface, 0)?;
        let enum_def = EnumCollectionAnalysis::owning_enum(cx, variants.first()?.to_def_id())?;

        // Require one enum owner before comparing the sequence with generated providers.
        if variants.iter().any(|variant| {
            EnumCollectionAnalysis::owning_enum(cx, variant.to_def_id()) != Some(enum_def)
        }) {
            return None;
        }

        // Compare the authored sequence against each enabled generated collection surface.
        let GeneratedVariantSets {
            variant_array,
            enum_iter,
        } = EnumCollectionAnalysis::generated_variant_sets(cx, enum_def)?;
        let mut providers = Vec::new();
        if surface.is_static()
            && variant_array
                .as_ref()
                .is_some_and(|expected| variants == *expected)
        {
            providers.push(CollectionProvider::StrumVariantArray);
        }
        if yields_owned
            && enum_iter
                .as_ref()
                .is_some_and(|expected| variants == *expected)
        {
            providers.push(CollectionProvider::StrumEnumIter);
        }

        // No enabled derive matches a collection with a different surface or membership.
        if providers.is_empty() {
            return None;
        }

        Some(Self {
            span,
            owner,
            enum_def,
            providers,
            is_public_api: cx.tcx.visibility(definition).is_public(),
        })
    }

    /// Lists framework providers available in the current compilation.
    pub(crate) fn providers(&self) -> &[CollectionProvider] {
        &self.providers
    }

    /// Resolves the configured provider when several implementations are available.
    pub(crate) fn selected(
        &self,
        configured: Option<CollectionProvider>,
    ) -> Option<CollectionProvider> {
        let providers = self.providers();

        // A single compatible provider needs no configuration to resolve ownership.
        if providers.len() == 1 {
            return providers.first().copied();
        }
        configured.filter(|provider| providers.contains(provider))
    }

    /// Returns whether replacement would alter a public API.
    pub(crate) const fn is_public_api(&self) -> bool {
        self.is_public_api
    }

    /// Resolves the owning enum name used in diagnostics.
    pub(crate) fn enum_name(&self, cx: &LateContext<'_>) -> Symbol {
        cx.tcx.item_name(self.enum_def.to_def_id())
    }
}
