extern crate rustc_abi;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::collections::{HashMap, HashSet};

use rustc_abi::ExternAbi;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::{Expr, ExprKind, HirId, ImplItem, ImplItemKind, Item, ItemKind, Node};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, TypeVisitableExt};
use rustc_session::config::CrateType;
use rustc_span::{Span, Symbol};

use super::direct_forwarding::DirectForwarding;
use super::impl_target::ImplTargetExt;
use super::visibility_package_policy::VisibilityPackagePolicy;

/// Returns whether the compiler is building an ordinary executable target.
fn is_delegating_type_binary_crate(cx: &LateContext<'_>) -> bool {
    !cx.sess().opts.test
        && cx
            .sess()
            .opts
            .crate_types
            .iter()
            .all(|kind| *kind == CrateType::Executable)
}

// -----------------------------------------------------------------------------
// DelegatingType: Candidate and evidence
// -----------------------------------------------------------------------------

/// One authored concrete single-field struct eligible for complete API analysis.
struct DelegatingTypeCandidate {
    /// Definition identity shared by construction and implementation evidence.
    def_id: LocalDefId,
    /// Authored wrapper name and diagnostic anchor.
    name: Symbol,
    /// Authored wrapper name span.
    name_span: Span,
    /// Sole field source span.
    field_span: Span,
    /// Resolved stored type rendered in guidance.
    inner_type: String,
}

/// Complete crate evidence retained for one wrapper `candidate`.
#[derive(Default)]
struct DelegatingTypeEvidence {
    /// Authored direct construction sites.
    constructions: Vec<Span>,
    /// Methods proven to forward exactly to the stored value.
    forwarding_methods: Vec<Span>,
    /// Wrapper, field, and associated declarations governed by removal guidance.
    owned_declarations: HashSet<LocalDefId>,
}

/// Maximum number of forwarding methods labeled by one diagnostic.
const DELEGATING_TYPE_MAX_METHOD_LABELS: usize = 4;

/// Minimum exact forwarding surface needed to diagnose a wrapper.
const DELEGATING_TYPE_MIN_FORWARDING_METHODS: usize = 2;

// -----------------------------------------------------------------------------
// DelegatingTypeFinding: Diagnostic context
// -----------------------------------------------------------------------------

/// Wrapper declaration context retained for one finding.
pub struct DelegatingTypeFindingDeclaration {
    /// Wrapper definition used for precedence and policy coordination.
    pub(crate) def_id: LocalDefId,
    /// Declaration node used to respect its lint level.
    pub(crate) hir_id: HirId,
    /// Authored wrapper name and primary span.
    pub(crate) name: Symbol,
    /// Authored wrapper name span.
    pub(crate) name_span: Span,
}

/// Stored value and representative forwarding behavior.
pub struct DelegatingTypeFindingDelegation {
    /// Sole field source span.
    pub(crate) field_span: Span,
    /// Resolved stored type rendered in guidance.
    pub(crate) inner_type: String,
    /// Representative exact-forwarding method names.
    pub(crate) forwarding_methods: Vec<Span>,
}

/// Complete evidence for one behavior-free forwarding wrapper.
pub struct DelegatingTypeFinding {
    /// Wrapper declaration identity and diagnostic anchor.
    pub(crate) declaration: DelegatingTypeFindingDeclaration,
    /// Stored value and exact-forwarding evidence.
    pub(crate) delegation: DelegatingTypeFindingDelegation,
    /// Wrapper-owned declarations suppressed from visibility narrowing.
    pub(crate) owned_declarations: HashSet<LocalDefId>,
    /// Whether public visibility is analyzed under a closed package contract.
    pub(crate) is_closed_package_public: bool,
}

// -----------------------------------------------------------------------------
// DelegatingTypeAnalyzer: Crate wide wrapper policy
// -----------------------------------------------------------------------------
/// Collects wrapper declarations, complete inherent behavior, traits, and constructions.
#[derive(Default)]
pub struct DelegatingTypeAnalyzer {
    /// Eligible declarations indexed independently from traversal order.
    candidates: HashMap<LocalDefId, DelegatingTypeCandidate>,
    /// Construction and forwarding evidence indexed by wrapper definition.
    evidence: HashMap<LocalDefId, DelegatingTypeEvidence>,
    /// Wrappers with any structural evidence of meaningful ownership.
    disqualified: HashSet<LocalDefId>,
}

impl DelegatingTypeAnalyzer {
    /// Returns whether wrapper and target receivers preserve ownership and borrow mutability.
    fn is_matching_receiver_modes(wrapper: ty::Ty<'_>, target: ty::Ty<'_>) -> bool {
        match (wrapper.kind(), target.kind()) {
            (ty::Ref(_, _, wrapper_mutability), ty::Ref(_, _, target_mutability)) => {
                wrapper_mutability == target_mutability
            }
            (ty::Ref(..), _) | (_, ty::Ref(..)) => false,
            _ => true,
        }
    }

    /// Resolves a tuple constructor or struct path to its local struct definition.
    fn struct_resolution(cx: &LateContext<'_>, resolution: Res) -> Option<LocalDefId> {
        let mut definition = resolution.opt_def_id()?;
        while matches!(
            cx.tcx.def_kind(definition),
            DefKind::Ctor(..) | DefKind::Variant
        ) {
            definition = cx.tcx.parent(definition);
        }
        matches!(cx.tcx.def_kind(definition), DefKind::Struct)
            .then(|| definition.as_local())
            .flatten()
    }

    /// Returns whether one expression directly selects the `candidate`'s sole field from `self`.
    fn is_inner_field(cx: &LateContext<'_>, expression: &Expr<'_>, self_binding: HirId) -> bool {
        // Permit an explicit borrow around the field while preserving its identity.
        let expression = match expression.kind {
            ExprKind::AddrOf(_, _, inner) => inner,
            _ => expression,
        };

        // Only field expressions can select the wrapper's stored value.
        let ExprKind::Field(base, _) = expression.kind else {
            return false;
        };
        DirectForwarding::is_binding(cx, base, self_binding)
    }

    /// Returns whether a local target is authored as an asynchronous function.
    fn is_async_target(cx: &LateContext<'_>, target: DefId) -> bool {
        // External targets cannot be inspected as authored async functions.
        let Some(target) = target.as_local() else {
            return false;
        };
        match cx.tcx.hir_node_by_def_id(target) {
            Node::Item(item) => {
                matches!(item.kind, ItemKind::Fn { sig, .. } if sig.header.is_async())
            }
            Node::ImplItem(item) => {
                matches!(item.kind, ImplItemKind::Fn(signature, _) if signature.header.is_async())
            }
            _ => false,
        }
    }

    /// Proves that one method forwards receiver, parameters, and output without adaptation.
    fn forwarding_method<'tcx>(cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) -> Option<Span> {
        // Require one ordinary nongeneric inherent method with an authored receiver.
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return None;
        };

        // ABI, safety, and generic parameters can make forwarding semantically observable.
        if !signature.decl.implicit_self.has_implicit_self()
            || signature.header.abi != ExternAbi::Rust
            || signature.header.is_unsafe()
            || item.generics.params.iter().any(|parameter| {
                !matches!(parameter.kind, rustc_hir::GenericParamKind::Lifetime { .. })
            })
        {
            return None;
        }

        // Resolve the one direct call and require a receiver plus every authored parameter.
        let body = cx.tcx.hir_body(body_id);
        let forwarding =
            DirectForwarding::expression(cx, item.owner_id.def_id, signature.header, body)?;
        let call = DirectForwarding::call(cx, forwarding.typeck_owner, forwarding.forwarded)?;

        // Forwarding requires one method call with every authored argument preserved.
        if !call.is_method || call.arguments.len() != forwarding.bindings.len() {
            return None;
        }

        // Require the call receiver to be the wrapper's stored field.
        let self_binding = *forwarding.bindings.first()?;

        // The call receiver must be the wrapper's sole stored value.
        if !Self::is_inner_field(cx, call.arguments[0], self_binding) {
            return None;
        }

        // Resolve the wrapper signature and body types for adjustment and output checks.
        let typeck = cx.tcx.typeck(forwarding.typeck_owner);
        let wrapper_signature = cx
            .tcx
            .fn_sig(item.owner_id.def_id)
            .instantiate_identity()
            .skip_binder();
        let target_signature = cx
            .tcx
            .fn_sig(call.target)
            .instantiate_identity()
            .skip_binder();

        // Wrapper and target receivers must preserve ownership and borrow mutability.
        if !Self::is_matching_receiver_modes(
            wrapper_signature.inputs()[0],
            target_signature.inputs()[0],
        ) {
            return None;
        }

        // Require all explicit parameters to retain binding identity, order, and adjusted type.
        let forwarded_arguments = call.arguments.iter().skip(1);
        let forwarded_bindings = forwarding.bindings.iter().skip(1);
        for (argument, binding) in forwarded_arguments.zip(forwarded_bindings) {
            let matches = typeck.expr_ty(argument) == typeck.expr_ty_adjusted(argument)
                && DirectForwarding::is_binding(cx, argument, *binding);

            // Any adapted or reordered parameter makes the method more than direct forwarding.
            if !matches {
                return None;
            }
        }

        // Preserve direct return and async behavior without normalizing adapters.
        // Both branches require the target call to preserve the wrapper method's execution contract.
        if signature.header.is_async() {
            // Async forwarding requires an async target method as well.
            if !Self::is_async_target(cx, call.target) {
                return None;
            }
        } else {
            // Synchronous forwarding must preserve the direct result type.
            if wrapper_signature.output() != typeck.expr_ty(forwarding.forwarded) {
                return None;
            }
        }
        Some(item.ident.span)
    }

    /// Recognizes an identity constructor or direct accessor that adds no wrapper behavior.
    fn is_neutral_method<'tcx>(
        cx: &LateContext<'tcx>,
        item: &'tcx ImplItem<'tcx>,
        wrapper: LocalDefId,
    ) -> bool {
        // Require one ordinary nongeneric Rust function before inspecting its body.
        let ImplItemKind::Fn(signature, body_id) = item.kind else {
            return false;
        };

        // ABI, safety, async behavior, and non-lifetime generics add wrapper semantics.
        if signature.header.abi != ExternAbi::Rust
            || signature.header.is_unsafe()
            || signature.header.is_async()
            || item.generics.params.iter().any(|parameter| {
                !matches!(parameter.kind, rustc_hir::GenericParamKind::Lifetime { .. })
            })
        {
            return false;
        }

        // Reduce the body to one transparent expression.
        let body = cx.tcx.hir_body(body_id);

        // Neutral methods reduce to one transparent expression.
        let Some(expression) = DirectForwarding::single_body_expression(body.value) else {
            return false;
        };

        // Direct accessors expose the same field through the authored receiver.
        if signature.decl.implicit_self.has_implicit_self() {
            // Accessors require a receiver binding that can identify the stored field.
            let Some(parameter) = body.params.first() else {
                return false;
            };

            // The receiver pattern must remain a direct binding.
            let rustc_hir::PatKind::Binding(_, self_binding, _, None) = parameter.pat.kind else {
                return false;
            };
            return Self::is_inner_field(cx, expression, self_binding);
        }

        // Resolve the identity constructor's one plain input binding.
        let [parameter] = body.params else {
            return false;
        };

        // The identity input must retain a direct binding identity.
        let rustc_hir::PatKind::Binding(_, binding, _, None) = parameter.pat.kind else {
            return false;
        };

        // Require the constructor expression to produce this wrapper directly.
        let expression_type = cx.tcx.typeck(item.owner_id.def_id).expr_ty(expression);

        // Only nominal expression results can be the candidate wrapper.
        let ty::Adt(definition, _) = expression_type.kind() else {
            return false;
        };

        // The constructed nominal type must be the wrapper under analysis.
        if definition.did().as_local() != Some(wrapper) {
            return false;
        }

        // Accept tuple and record construction only when they wrap that binding unchanged.
        match expression.kind {
            ExprKind::Call(_, [argument]) => DirectForwarding::is_binding(cx, argument, binding),
            ExprKind::Struct(_, [field], _) => {
                DirectForwarding::is_binding(cx, field.expr, binding)
            }
            _ => false,
        }
    }

    /// Classifies every inherent associated item owned by a `candidate` wrapper.
    pub(crate) fn record_impl_item<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        item: &'tcx ImplItem<'tcx>,
    ) {
        // Resolve the owning inherent implementation and its direct struct target.
        let implementation = cx.tcx.local_parent(item.owner_id.def_id);

        // Only implementation parents can own an associated wrapper item.
        let Node::Item(parent) = cx.tcx.hir_node_by_def_id(implementation) else {
            return;
        };

        // The implementation must target one direct struct wrapper.
        let Some(wrapper) = parent.direct_struct(cx) else {
            return;
        };

        // Retain associated ownership before classifying authored behavior.
        self.evidence
            .entry(wrapper)
            .or_default()
            .owned_declarations
            .insert(item.owner_id.def_id);

        // Generated or semantically annotated methods preserve meaningful wrapper ownership.
        if item.span.from_expansion()
            || !DirectForwarding::has_only_nonsemantic_attributes(cx, item.hir_id())
        {
            self.disqualified.insert(wrapper);
            return;
        }

        // Associated constants and types demonstrate ownership beyond method forwarding.
        let ImplItemKind::Fn(..) = item.kind else {
            self.disqualified.insert(wrapper);
            return;
        };

        // Only prequalified wrapper declarations accumulate forwarding evidence.
        if !self.candidates.contains_key(&wrapper) {
            return;
        }
        if let Some(span) = Self::forwarding_method(cx, item) {
            self.evidence
                .entry(wrapper)
                .or_default()
                .forwarding_methods
                .push(span);
        } else if !Self::is_neutral_method(cx, item, wrapper) {
            self.disqualified.insert(wrapper);
        }
    }

    /// Records one authored direct construction of a `candidate` struct.
    pub(crate) fn record_expression(&mut self, cx: &LateContext<'_>, expression: &Expr<'_>) {
        // Ignore generated use sites before resolving tuple or record construction syntax.
        if expression.span.from_expansion() {
            return;
        }
        let wrapper = match expression.kind {
            ExprKind::Call(callee, [_]) => {
                // Tuple construction must name its constructor directly.
                let ExprKind::Path(path) = callee.kind else {
                    return;
                };
                Self::struct_resolution(cx, cx.qpath_res(&path, callee.hir_id))
            }
            ExprKind::Struct(path, [_], _) => {
                Self::struct_resolution(cx, cx.qpath_res(path, expression.hir_id)).or_else(|| {
                    let expression_type = cx.typeck_results().expr_ty(expression);

                    // The record expression type must be nominal before it identifies a wrapper.
                    let ty::Adt(definition, _) = expression_type.kind() else {
                        return None;
                    };
                    definition.did().as_local()
                })
            }

            // Other expression forms are not direct wrapper construction sites.
            _ => return,
        };

        // Count any authored direct construction because its operand is outside the boundary.
        let Some(wrapper) = wrapper else {
            return;
        };
        self.evidence
            .entry(wrapper)
            .or_default()
            .constructions
            .push(expression.span);
    }

    /// Derives source-ordered findings after complete active-crate analysis.
    pub(crate) fn findings(self, cx: &LateContext<'_>) -> Vec<DelegatingTypeFinding> {
        let package = VisibilityPackagePolicy::for_current_package();
        let binary = is_delegating_type_binary_crate(cx);
        let mut evidence = self.evidence;
        let mut findings = Vec::new();
        for candidate in self.candidates.into_values() {
            // Require an eligible wrapper with construction and the minimum forwarding surface.
            if self.disqualified.contains(&candidate.def_id) {
                continue;
            }
            let Some(mut candidate_evidence) = evidence.remove(&candidate.def_id) else {
                continue;
            };
            if candidate_evidence.constructions.is_empty()
                || candidate_evidence.forwarding_methods.len()
                    < DELEGATING_TYPE_MIN_FORWARDING_METHODS
            {
                continue;
            }

            // Preserve unobservable downstream behavior in publishable libraries.
            let exported = cx
                .tcx
                .effective_visibilities(())
                .is_exported(candidate.def_id);
            if !binary && package.is_preserving_exported_public_items() && exported {
                continue;
            }

            // Bound labels before packaging complete diagnostic and precedence evidence.
            candidate_evidence
                .forwarding_methods
                .truncate(DELEGATING_TYPE_MAX_METHOD_LABELS);

            // Separate declaration identity from stored-value behavior evidence.
            let declaration = DelegatingTypeFindingDeclaration {
                def_id: candidate.def_id,
                hir_id: cx.tcx.local_def_id_to_hir_id(candidate.def_id),
                name: candidate.name,
                name_span: candidate.name_span,
            };

            // Retain the sole field and representative exact-forwarding methods.
            let delegation = DelegatingTypeFindingDelegation {
                field_span: candidate.field_span,
                inner_type: candidate.inner_type,
                forwarding_methods: candidate_evidence.forwarding_methods,
            };

            // Package precedence ownership and closed-package diagnostic context.
            let is_closed_package_public = package == VisibilityPackagePolicy::Closed && exported;
            findings.push(DelegatingTypeFinding {
                declaration,
                delegation,
                owned_declarations: candidate_evidence.owned_declarations,
                is_closed_package_public,
            });
        }
        findings.sort_by_key(|finding| finding.declaration.name_span.lo());
        findings
    }

    /// Records one eligible authored concrete single-field struct.
    fn record_struct(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Require an authored nongeneric struct before inspecting its stored field.
        let ItemKind::Struct(ident, generics, data) = item.kind else {
            return;
        };

        // Select exactly one authored field with no semantic attributes.
        let [field] = data.fields() else {
            return;
        };

        // Generated, generic, or semantically annotated fields do not have a fixed wrapper contract.
        if item.span.from_expansion()
            || !generics.params.is_empty()
            || !DirectForwarding::has_only_nonsemantic_attributes(cx, item.hir_id())
            || !DirectForwarding::has_only_nonsemantic_attributes(cx, field.hir_id)
        {
            return;
        }

        // Resolve and reject a stored type whose target remains generic.
        let inner = cx.tcx.type_of(field.def_id).instantiate_identity();

        // Generic storage types prevent a fixed delegation contract.
        if inner.has_param() {
            return;
        }

        // Retain complete declaration context independently from traversal order.
        let candidate = DelegatingTypeCandidate {
            def_id: item.owner_id.def_id,
            name: ident.name,
            name_span: ident.span,
            field_span: field.span,
            inner_type: inner.to_string(),
        };

        // Seed ownership evidence for visibility precedence.
        let evidence = self.evidence.entry(candidate.def_id).or_default();
        evidence.owned_declarations.insert(candidate.def_id);
        evidence.owned_declarations.insert(field.def_id);
        self.candidates.insert(candidate.def_id, candidate);
    }

    /// Records trait ownership or semantic attributes on an implementation block.
    fn record_implementation(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Resolve one direct local wrapper implementation.
        let ItemKind::Impl(implementation) = item.kind else {
            return;
        };

        // Only direct struct implementations contribute wrapper ownership evidence.
        let Some(wrapper) = item.direct_struct(cx) else {
            return;
        };

        // Trait contracts, generation, and semantic attributes preserve ownership.
        let preserves_wrapper = implementation.of_trait.is_some()
            || item.span.from_expansion()
            || !DirectForwarding::has_only_nonsemantic_attributes(cx, item.hir_id());
        self.disqualified
            .extend(preserves_wrapper.then_some(wrapper));
    }

    /// Records one declaration or implementation-level structural signal.
    pub(crate) fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        match item.kind {
            ItemKind::Struct(..) => self.record_struct(cx, item),
            ItemKind::Impl(..) => self.record_implementation(cx, item),
            _ => {}
        }
    }
}
