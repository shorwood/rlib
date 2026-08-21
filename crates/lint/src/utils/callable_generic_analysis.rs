extern crate rustc_abi;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_abi::ExternAbi;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{
    AmbigArg, Expr, ExprKind, FnHeader, FnSig, GenericArg, GenericParamKind, Generics, HirId,
    ImplItem, ImplItemKind, Item, ItemKind, Node, QPath, Ty, TyKind, WherePredicateKind,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty::{self, GenericParamDefKind, TypeVisitableExt};
use rustc_session::config::CrateType;
use rustc_span::{Ident, Span, Symbol};

use super::generic_abstraction_analysis::{
    GenericAbstractionFinding, GenericAbstractionFindingDeclaration,
    GenericAbstractionFindingParameter,
};
use super::visibility_package_policy::VisibilityPackagePolicy;

// -----------------------------------------------------------------------------
// CallableGeneric: Declaration and evidence
// -----------------------------------------------------------------------------

/// One authored callable type parameter and its compiler argument index.
struct CallableGenericParameter {
    /// Authored parameter name.
    name: Symbol,
    /// Authored parameter span receiving the diagnostic.
    span: Span,
    /// Absolute compiler generic-argument index, including parent impl parameters.
    argument_index: usize,
}

/// One eligible free or inherent callable with its own authored type parameters.
struct CallableGenericDeclaration {
    /// Callable definition resolved at every call site.
    def_id: LocalDefId,
    /// Declaration node used to respect its lint level.
    hir_id: HirId,
    /// Authored callable name.
    name: Symbol,
    /// Human-readable callable category.
    kind: &'static str,
    /// Own authored type parameters evaluated independently.
    parameters: Vec<CallableGenericParameter>,
}

/// Stable evidence key for one callable parameter.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct CallableGenericParameterId {
    /// Owning callable.
    declaration: LocalDefId,
    /// Absolute compiler generic-argument index.
    argument_index: usize,
}

/// Complete resolved call evidence for one callable parameter.
#[derive(Default)]
struct CallableGenericEvidence {
    /// Concrete substitutions and representative call spans.
    concrete: HashMap<String, Vec<Span>>,
    /// Whether forwarding, escaping, or an unnameable substitution keeps the parameter open.
    has_open_boundary: bool,
}

/// Supported free-function declaration parts.
struct CallableGenericFreeFunction<'hir> {
    /// Authored function signature.
    signature: FnSig<'hir>,
    /// Authored function identity.
    ident: Ident,
    /// Own generic parameter declarations.
    generics: &'hir Generics<'hir>,
}

// -----------------------------------------------------------------------------
// ResolvedCallBoundary: Conservative call evidence
// -----------------------------------------------------------------------------

/// Whether authored call syntax preserves a conservative open boundary.
#[derive(Clone, Copy)]
enum ResolvedCallBoundary {
    /// Compiler-resolved arguments may supply concrete evidence.
    Concrete,
    /// Authored syntax contains an alias or abstract type form.
    Open,
}

impl ResolvedCallBoundary {
    /// Classifies authored type syntax on one direct path call.
    fn for_path<'tcx>(cx: &LateContext<'tcx>, path: &QPath<'tcx>) -> Self {
        if CallableGenericAnalyzer::has_path_explicit_open_argument(cx, path) {
            Self::Open
        } else {
            Self::Concrete
        }
    }

    /// Classifies authored type syntax on one method call.
    fn for_method<'tcx>(
        cx: &LateContext<'tcx>,
        arguments: Option<&'tcx rustc_hir::GenericArgs<'tcx>>,
    ) -> Self {
        let is_open = arguments.is_some_and(|arguments| {
            CallableGenericAnalyzer::has_explicit_open_argument(cx, arguments.args.iter())
        });
        if is_open { Self::Open } else { Self::Concrete }
    }
}

/// Maximum representative calls attached to one concrete substitution.
const RESOLVED_CALL_MAX_REPRESENTATIVE_SPANS: usize = 4;

// -----------------------------------------------------------------------------
// CallableVisibilityContext: Publication context
// -----------------------------------------------------------------------------

/// Publication context retained for callable diagnostics.
#[derive(Clone, Copy)]
enum CallableVisibilityContext {
    /// Private callable or callable in an executable target.
    Ordinary,
    /// Exported callable analyzed because its package is explicitly closed.
    ClosedPackagePublic,
}

impl CallableVisibilityContext {
    /// Classifies one declaration from effective visibility and package policy.
    fn for_declaration(
        cx: &LateContext<'_>,
        package: VisibilityPackagePolicy,
        declaration: LocalDefId,
    ) -> Self {
        let exported = cx.tcx.effective_visibilities(()).is_exported(declaration);
        if matches!(package, VisibilityPackagePolicy::Closed) && exported {
            Self::ClosedPackagePublic
        } else {
            Self::Ordinary
        }
    }
}

/// Returns whether the compiler is building an ordinary executable target.
fn is_callable_visibility_binary_crate(cx: &LateContext<'_>) -> bool {
    !cx.sess().opts.test
        && cx
            .sess()
            .opts
            .crate_types
            .iter()
            .all(|kind| *kind == CrateType::Executable)
}

// -----------------------------------------------------------------------------
// ExplicitOpenTypeVisitor: Authored abstract syntax
// -----------------------------------------------------------------------------

/// Returns whether explicit type syntax is inherently abstract.
const fn is_explicit_type_form_open(ty: &Ty<'_, AmbigArg>) -> bool {
    matches!(
        ty.kind,
        TyKind::OpaqueDef(_)
            | TyKind::TraitAscription(_)
            | TyKind::TraitObject(..)
            | TyKind::Infer(_)
            | TyKind::Err(_)
    )
}

/// Returns whether an explicit type path resolves through an open boundary.
const fn is_explicit_type_resolution_open(resolution: Res) -> bool {
    matches!(
        resolution,
        Res::Err
            | Res::Def(
                DefKind::TyParam
                    | DefKind::AssocTy
                    | DefKind::TyAlias
                    | DefKind::TraitAlias
                    | DefKind::OpaqueTy,
                _,
            )
    )
}

/// Finds explicit authored type syntax that must remain an open call boundary.
struct ExplicitOpenTypeVisitor<'a, 'tcx> {
    /// Compiler context used to resolve authored type paths.
    cx: &'a LateContext<'tcx>,
    /// Accumulated conservative classification.
    has_open_boundary: bool,
}

impl<'tcx> Visitor<'tcx> for ExplicitOpenTypeVisitor<'_, 'tcx> {
    fn visit_ty(&mut self, ty: &'tcx Ty<'tcx, AmbigArg>) {
        // Stop traversal once any authored component keeps the call open.
        if self.has_open_boundary {
            return;
        }

        // Reject inference and abstraction forms before descending into children.
        if is_explicit_type_form_open(ty) {
            self.has_open_boundary = true;
            return;
        }

        // Resolve paths so aliases and projections remain authored open boundaries.
        if let TyKind::Path(path) = ty.kind {
            let resolution = self.cx.qpath_res(&path, ty.hir_id);
            self.has_open_boundary = is_explicit_type_resolution_open(resolution);

            // Once a path proves an open boundary, nested syntax cannot make it concrete again.
            if self.has_open_boundary {
                return;
            }
        }

        // Inspect nested type arguments only while the outer form remains concrete.
        intravisit::walk_ty(self, ty);
    }
}

// -----------------------------------------------------------------------------
// CallableGenericAnalyzer: Crate wide resolved calls
// -----------------------------------------------------------------------------

/// Collects eligible callables, direct resolved calls, and callable escapes.
#[derive(Default)]
pub struct CallableGenericAnalyzer {
    /// Eligible declarations indexed independently from traversal order.
    declarations: HashMap<LocalDefId, CallableGenericDeclaration>,
    /// Parameter evidence accumulated from every active authored use.
    evidence: HashMap<CallableGenericParameterId, CallableGenericEvidence>,
}

impl CallableGenericAnalyzer {
    /// Returns whether explicit call arguments contain aliases or abstract type syntax.
    fn has_explicit_open_argument<'tcx>(
        cx: &LateContext<'tcx>,
        arguments: impl Iterator<Item = &'tcx GenericArg<'tcx>>,
    ) -> bool {
        let mut visitor = ExplicitOpenTypeVisitor {
            cx,
            has_open_boundary: false,
        };
        for argument in arguments {
            let GenericArg::Type(ty) = argument else {
                continue;
            };
            visitor.visit_ty(ty);
        }
        visitor.has_open_boundary
    }

    /// Finds explicit generic arguments attached to a direct path call.
    fn has_path_explicit_open_argument<'tcx>(cx: &LateContext<'tcx>, path: &QPath<'tcx>) -> bool {
        // Type-relative associated calls carry arguments on their terminal segment.
        if let QPath::TypeRelative(_, segment) = path {
            return segment.args.is_some_and(|arguments| {
                Self::has_explicit_open_argument(cx, arguments.args.iter())
            });
        }

        // Other qualified path forms do not expose resolved segment arguments here.
        let QPath::Resolved(_, path) = path else {
            return false;
        };

        // Resolved free-function paths may carry arguments on any qualified segment.
        let arguments = path
            .segments
            .iter()
            .filter_map(|segment| segment.args)
            .flat_map(|arguments| arguments.args);
        Self::has_explicit_open_argument(cx, arguments)
    }

    /// Returns whether bounds couple parameters or introduce a higher-ranked boundary.
    fn has_dependent_parameters(generics: &Generics<'_>) -> bool {
        // Higher-ranked bounds demonstrate reusable behavior beyond one resolved call type.
        let has_higher_ranked_bound = generics.predicates.iter().any(|predicate| {
            matches!(
                predicate.kind,
                WherePredicateKind::BoundPredicate(bound)
                    if !bound.bound_generic_params.is_empty()
            )
        });

        // Conservatively preserve bounded multi-parameter declarations as coupled contracts.
        let authored_type_count = generics
            .params
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter.kind,
                    GenericParamKind::Type {
                        synthetic: false,
                        ..
                    }
                )
            })
            .count();
        has_higher_ranked_bound || (authored_type_count > 1 && !generics.predicates.is_empty())
    }

    /// Selects own authored type parameters and their absolute compiler argument indexes.
    fn parameters(
        cx: &LateContext<'_>,
        declaration: LocalDefId,
        generics: &Generics<'_>,
    ) -> Vec<CallableGenericParameter> {
        // Preserve coupled or higher-ranked generic contracts as open boundaries.
        if Self::has_dependent_parameters(generics) {
            return Vec::new();
        }

        // Keep compiler type parameters aligned with the authored type-only sequence.
        let compiler_generics = cx.tcx.generics_of(declaration);
        let compiler_parameters = compiler_generics
            .own_params
            .iter()
            .filter(|parameter| matches!(parameter.kind, GenericParamDefKind::Type { .. }));

        // Select authored type parameters independently from lifetimes and const parameters.
        let authored_parameters = generics.params.iter().filter(|parameter| {
            matches!(
                parameter.kind,
                GenericParamKind::Type {
                    synthetic: false,
                    ..
                }
            )
        });

        // Pair only explicit type parameters; lifetimes, consts, and synthetic parameters are out.
        authored_parameters
            .zip(compiler_parameters)
            .map(|(authored, compiler)| CallableGenericParameter {
                name: authored.name.ident().name,
                span: authored.span,
                argument_index: compiler.index as usize,
            })
            .collect()
    }

    /// Returns whether an authored callable has ordinary safe Rust call semantics.
    fn is_header_eligible(header: FnHeader) -> bool {
        header.abi == ExternAbi::Rust && !header.is_unsafe()
    }

    /// Returns one stable spelling only for fully concrete, nameable semantic types.
    fn concrete_type(argument: ty::Ty<'_>) -> Option<String> {
        // Reject generic and inference state before inspecting nested semantic type forms.
        if argument.has_param()
            || argument.has_infer()
            || argument.has_placeholders()
            || argument.has_escaping_bound_vars()
        {
            return None;
        }

        // Closures, callables, coroutine internals, trait objects, and aliases stay open.
        let has_open_component = argument.walk().any(|component| {
            // Non-type generic components do not affect type nameability.
            let Some(component) = component.as_type() else {
                return false;
            };
            matches!(
                component.kind(),
                ty::Alias(..)
                    | ty::Bound(..)
                    | ty::Closure(..)
                    | ty::Coroutine(..)
                    | ty::CoroutineClosure(..)
                    | ty::CoroutineWitness(..)
                    | ty::Dynamic(..)
                    | ty::Error(_)
                    | ty::FnDef(..)
                    | ty::FnPtr(..)
                    | ty::Infer(..)
                    | ty::Param(..)
                    | ty::Placeholder(..)
            )
        });
        (!has_open_component).then(|| argument.to_string())
    }

    /// Builds one finding from complete single-substitution call evidence.
    fn parameter_finding(
        declaration: &CallableGenericDeclaration,
        parameter: &CallableGenericParameter,
        evidence: Option<&CallableGenericEvidence>,
        visibility: CallableVisibilityContext,
    ) -> Option<GenericAbstractionFinding> {
        // Require at least one resolved call, no open boundary, and one concrete identity.
        let evidence = evidence?;

        // Open or varying substitutions preserve the authored generic abstraction.
        if evidence.has_open_boundary || evidence.concrete.len() != 1 {
            return None;
        }
        let (concrete_type, use_spans) = evidence.concrete.iter().next()?;

        // Package declaration and parameter identity independently from use evidence.
        let finding_declaration = GenericAbstractionFindingDeclaration {
            hir_id: declaration.hir_id,
            name: declaration.name,
            kind: declaration.kind,
            is_callable: true,
        };

        // Retain the independently diagnosed authored parameter identity.
        let finding_parameter = GenericAbstractionFindingParameter {
            name: parameter.name,
            span: parameter.span,
        };

        // Preserve the closed-package rationale without proposing a mechanical edit.
        let is_closed_package_public =
            matches!(visibility, CallableVisibilityContext::ClosedPackagePublic);

        // Assemble the immutable diagnostic record after all policy decisions.
        Some(GenericAbstractionFinding {
            declaration: finding_declaration,
            parameter: finding_parameter,
            concrete_type: concrete_type.clone(),
            use_spans: use_spans.clone(),
            is_closed_package_public,
        })
    }

    /// Extracts one authored free function with a real body.
    fn free_function<'hir>(item: &'hir Item<'hir>) -> Option<CallableGenericFreeFunction<'hir>> {
        // Extract the supported free-function signature and generic declarations.
        let ItemKind::Fn { sig, generics, .. } = item.kind else {
            return None;
        };

        // Package authored identity separately from the HIR owner identity.
        Some(CallableGenericFreeFunction {
            signature: sig,
            ident: item.kind.ident()?,
            generics,
        })
    }

    /// Records one eligible free function declaration.
    pub(crate) fn record_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        // Limit this slice to authored free functions with bodies and ordinary call semantics.
        let Some(function) = Self::free_function(item) else {
            return;
        };

        // Preserve generated, unsafe, and foreign-call-contract declarations.
        if item.span.from_expansion() || !Self::is_header_eligible(function.signature.header) {
            return;
        }

        // Retain only callables declaring at least one own authored type parameter.
        let parameters = Self::parameters(cx, item.owner_id.def_id, function.generics);

        // Functions without eligible authored type parameters need no abstraction finding.
        if parameters.is_empty() {
            return;
        }

        // Package the complete free-function declaration context.
        let declaration = CallableGenericDeclaration {
            def_id: item.owner_id.def_id,
            hir_id: item.hir_id(),
            name: function.ident.name,
            kind: "function",
            parameters,
        };

        // Index the complete declaration only after every eligibility decision.
        self.declarations.insert(declaration.def_id, declaration);
    }

    /// Records one eligible inherent function or method declaration.
    pub(crate) fn record_impl_item(&mut self, cx: &LateContext<'_>, item: &ImplItem<'_>) {
        // Exclude trait declarations and trait implementations from callable ownership analysis.
        let parent = cx.tcx.local_parent(item.owner_id.def_id);

        // Associated items whose parent is unavailable as an item cannot prove inherent ownership.
        let Node::Item(parent_item) = cx.tcx.hir_node_by_def_id(parent) else {
            return;
        };

        // Only implementation parents can establish inherent callable ownership.
        let ItemKind::Impl(implementation) = parent_item.kind else {
            return;
        };

        // Trait implementation methods inherit a trait contract rather than owning this abstraction.
        if implementation.of_trait.is_some() {
            return;
        }

        // Require an authored ordinary inherent callable with own generic parameters.
        let ImplItemKind::Fn(signature, _) = item.kind else {
            return;
        };

        // Generated, unsafe, or foreign callables are outside ordinary generic API guidance.
        if item.span.from_expansion() || !Self::is_header_eligible(signature.header) {
            return;
        }
        let parameters = Self::parameters(cx, item.owner_id.def_id, item.generics);

        // Callables without eligible authored type parameters need no abstraction finding.
        if parameters.is_empty() {
            return;
        }

        // Distinguish receiver methods from associated functions in diagnostic vocabulary.
        let kind = if signature.decl.implicit_self.has_implicit_self() {
            "method"
        } else {
            "associated function"
        };

        // Package the complete inherent-callable declaration context.
        let declaration = CallableGenericDeclaration {
            def_id: item.owner_id.def_id,
            hir_id: item.hir_id(),
            name: item.ident.name,
            kind,
            parameters,
        };

        // Index the complete declaration only after trait and signature screening.
        self.declarations.insert(declaration.def_id, declaration);
    }

    /// Derives source-ordered callable findings from complete active-crate evidence.
    pub(crate) fn findings(self, cx: &LateContext<'_>) -> Vec<GenericAbstractionFinding> {
        let package = VisibilityPackagePolicy::for_current_package();
        let binary = is_callable_visibility_binary_crate(cx);
        let evidence = self.evidence;
        let mut findings = Vec::new();

        // Apply publication policy before evaluating each callable parameter independently.
        for declaration in self.declarations.into_values() {
            // Preserve publishable public contracts before classifying package context.
            let exported = cx
                .tcx
                .effective_visibilities(())
                .is_exported(declaration.def_id);
            if !binary && package.is_preserving_exported_public_items() && exported {
                continue;
            }
            let visibility =
                CallableVisibilityContext::for_declaration(cx, package, declaration.def_id);

            // Join independently evaluated own parameters to their complete use evidence.
            findings.extend(declaration.parameters.iter().filter_map(|parameter| {
                let key = CallableGenericParameterId {
                    declaration: declaration.def_id,
                    argument_index: parameter.argument_index,
                };
                Self::parameter_finding(&declaration, parameter, evidence.get(&key), visibility)
            }));
        }

        // Restore authored order independently from hash-map traversal.
        findings.sort_by_key(|finding| finding.parameter.span.lo());
        findings
    }

    /// Records one resolved parameter substitution or open call boundary.
    fn record_parameter_call(
        &mut self,
        key: CallableGenericParameterId,
        argument: Option<ty::Ty<'_>>,
        span: Span,
        boundary: ResolvedCallBoundary,
    ) {
        let evidence = self.evidence.entry(key).or_default();

        // Explicitly open call syntax preserves the parameter without concrete classification.
        if matches!(boundary, ResolvedCallBoundary::Open) {
            evidence.has_open_boundary = true;
            return;
        }

        // Unresolved and unnameable semantic arguments preserve polymorphic possibility.
        let Some(concrete) = argument.and_then(Self::concrete_type) else {
            evidence.has_open_boundary = true;
            return;
        };

        // Bound labels while retaining every distinct concrete substitution identity.
        let spans = evidence.concrete.entry(concrete).or_default();

        // Additional calls beyond the representative limit add no diagnostic information.
        if spans.len() >= RESOLVED_CALL_MAX_REPRESENTATIVE_SPANS {
            return;
        }
        spans.push(span);
    }

    /// Records resolved arguments for one direct call target.
    fn record_call(
        &mut self,
        cx: &LateContext<'_>,
        target: DefId,
        arguments: ty::GenericArgsRef<'_>,
        span: Span,
        boundary: ResolvedCallBoundary,
    ) {
        // Calls into foreign declarations cannot provide local abstraction evidence.
        let Some(declaration) = target.as_local() else {
            return;
        };
        let generics = cx.tcx.generics_of(target);

        // Classify each own type parameter independently from declaration visitation order.
        for parameter in &generics.own_params {
            // Ignore lifetime and const parameters outside this first callable slice.
            if !matches!(parameter.kind, GenericParamDefKind::Type { .. }) {
                continue;
            }

            // Join the compiler argument index to its stable evidence key.
            let argument_index = parameter.index as usize;
            let key = CallableGenericParameterId {
                declaration,
                argument_index,
            };

            // Resolve the matching compiler argument before recording its evidence.
            let argument = arguments
                .get(argument_index)
                .and_then(|item| item.as_type());
            self.record_parameter_call(key, argument, span, boundary);
        }
    }

    /// Marks every own parameter open when its callable value escapes direct invocation.
    fn record_escape(&mut self, cx: &LateContext<'_>, declaration: LocalDefId) {
        for parameter in &cx.tcx.generics_of(declaration).own_params {
            if !matches!(parameter.kind, GenericParamDefKind::Type { .. }) {
                continue;
            }

            // Preserve escape evidence independently for every own type parameter.
            let key = CallableGenericParameterId {
                declaration,
                argument_index: parameter.index as usize,
            };
            self.evidence.entry(key).or_default().has_open_boundary = true;
        }
    }

    /// Records one ordinary direct path call.
    fn record_direct_call<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
        callee: &'tcx Expr<'tcx>,
    ) {
        // Resolve one direct path target before inspecting its generic arguments.
        let ExprKind::Path(path) = callee.kind else {
            return;
        };

        // Unresolved callee paths cannot identify a generic declaration.
        let Some(target) = cx.qpath_res(&path, callee.hir_id).opt_def_id() else {
            return;
        };

        // Combine compiler-resolved arguments with conservative authored syntax evidence.
        let boundary = ResolvedCallBoundary::for_path(cx, &path);
        let arguments = cx.typeck_results().node_args(callee.hir_id);
        self.record_call(cx, target, arguments, expression.span, boundary);
    }

    /// Records one compiler-selected method call.
    fn record_method_call<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
        segment: &'tcx rustc_hir::PathSegment<'tcx>,
    ) {
        // Resolve compiler-selected method identity and arguments together.
        let typeck = cx.typeck_results();

        // Calls without a compiler-selected method cannot provide substitution evidence.
        let Some(target) = typeck.type_dependent_def_id(expression.hir_id) else {
            return;
        };

        // Preserve explicit aliases while accepting fully inferred concrete arguments.
        let boundary = ResolvedCallBoundary::for_method(cx, segment.args);

        // Record the selected target after both syntax and semantic classification.
        self.record_call(
            cx,
            target,
            typeck.node_args(expression.hir_id),
            expression.span,
            boundary,
        );
    }

    /// Records a callable path used outside its direct call position.
    fn record_path_escape(
        &mut self,
        cx: &LateContext<'_>,
        expression: &Expr<'_>,
        path: &QPath<'_>,
    ) {
        // The direct callee path is already represented by its parent call expression.
        let is_direct_callee = matches!(
            cx.tcx.parent_hir_node(expression.hir_id),
            Node::Expr(parent)
                if matches!(parent.kind, ExprKind::Call(callee, _) if callee.hir_id == expression.hir_id)
        );

        // Direct callees are recorded by the parent call with their resolved arguments.
        if is_direct_callee {
            return;
        }

        // Any other local function or associated-function value keeps its parameters open.
        let Some(target) = cx.qpath_res(path, expression.hir_id).opt_def_id() else {
            return;
        };

        // Non-callable paths do not create an escaping callable value.
        if !matches!(cx.tcx.def_kind(target), DefKind::Fn | DefKind::AssocFn) {
            return;
        }

        // Foreign callable values do not affect a local declaration's abstraction boundary.
        let Some(target) = target.as_local() else {
            return;
        };
        self.record_escape(cx, target);
    }

    /// Records direct calls and non-call callable-value uses.
    pub(crate) fn record_expression<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
    ) {
        // Macro-generated expressions do not provide authored call-boundary evidence.
        if expression.span.from_expansion() {
            return;
        }

        // Route each call or reference form to its focused semantic recorder.
        match expression.kind {
            ExprKind::Call(callee, _) => self.record_direct_call(cx, expression, callee),
            ExprKind::MethodCall(segment, ..) => self.record_method_call(cx, expression, segment),
            ExprKind::Path(path) => self.record_path_escape(cx, expression, &path),
            _ => {}
        }
    }
}
