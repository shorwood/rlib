extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use std::collections::HashMap;

use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{
    AmbigArg, Expr, ExprKind, GenericArg, GenericParam, GenericParamKind, Generics, HirId, Item,
    ItemKind, QPath, Ty, TyKind,
};
use rustc_lint::{LateContext, LintContext};
use rustc_middle::ty;
use rustc_session::config::CrateType;
use rustc_span::{Span, Symbol};

use super::visibility_package_policy::VisibilityPackagePolicy;

/// Returns whether the compiler is building an ordinary executable target.
fn is_binary_crate(cx: &LateContext<'_>) -> bool {
    !cx.sess().opts.test
        && cx
            .sess()
            .opts
            .crate_types
            .iter()
            .all(|kind| *kind == CrateType::Executable)
}

// -----------------------------------------------------------------------------
// GenericAbstraction: Declaration and use evidence
// -----------------------------------------------------------------------------

/// One authored type parameter belonging to an eligible nominal declaration.
struct GenericAbstractionParameter {
    /// Authored parameter name.
    name: Symbol,
    /// Authored parameter span receiving the diagnostic.
    span: Span,
    /// Position among non-lifetime generic arguments.
    argument_position: usize,
}

/// One authored generic nominal type retained until all active uses are known.
struct GenericAbstractionDeclaration {
    /// Definition identity used to join resolved paths.
    def_id: LocalDefId,
    /// Declaration node used to respect its lint level.
    hir_id: HirId,
    /// Authored declaration name.
    name: Symbol,
    /// Human-readable declaration category.
    kind: &'static str,
    /// Explicit authored type parameters evaluated independently.
    parameters: Vec<GenericAbstractionParameter>,
}
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
/// Stable parameter identity used as the evidence-map key.
struct GenericAbstractionParameterId {
    /// Owning generic declaration.
    declaration: LocalDefId,
    /// Position among non-lifetime arguments.
    argument_position: usize,
}

/// Supported declaration syntax extracted from one HIR item.
struct GenericAbstractionParts<'hir> {
    /// Authored declaration name.
    name: Symbol,
    /// Human-readable declaration category.
    kind: &'static str,
    /// Authored generic parameter list.
    generics: &'hir Generics<'hir>,
}

// -----------------------------------------------------------------------------
// GenericSubstitution: Evidence classification
// -----------------------------------------------------------------------------
#[derive(Default)]
/// Observed substitutions and open-boundary evidence for one type parameter.
struct GenericSubstitutionEvidence {
    /// Concrete substitution spelling and representative authored spans.
    concrete: HashMap<String, Vec<Span>>,
    /// Whether any active use inferred, projected, aliased, or forwarded this parameter.
    has_open_boundary: bool,
}
#[derive(Clone, Copy, Eq, PartialEq)]
/// Visibility context carried into one diagnostic.
enum GenericSubstitutionVisibilityContext {
    /// Private declaration or declaration in an executable target.
    Ordinary,
    /// Public declaration analyzed because its package explicitly disables publishing.
    ClosedPackagePublic,
}

impl GenericSubstitutionVisibilityContext {
    /// Classifies one declaration from its package policy and effective visibility.
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

    /// Reports whether the package policy is relevant diagnostic context.
    const fn is_closed_package_public(self) -> bool {
        matches!(self, Self::ClosedPackagePublic)
    }
}

/// Maximum representative use sites attached to one concrete substitution.
const GENERIC_SUBSTITUTION_MAX_REPRESENTATIVE_USE_SPANS: usize = 4;

// -----------------------------------------------------------------------------
// GenericAbstractionFinding: Diagnostic evidence
// -----------------------------------------------------------------------------

/// Declaration context retained for one parameter-level finding.
pub struct GenericAbstractionFindingDeclaration {
    /// Declaration node used to respect its lint level.
    pub(crate) hir_id: HirId,
    /// Declaration name rendered in guidance.
    pub(crate) name: Symbol,
    /// Human-readable declaration category.
    pub(crate) kind: &'static str,
    /// Whether the declaration is a callable rather than a nominal type.
    pub(crate) is_callable: bool,
}

/// Parameter context retained for one finding.
pub struct GenericAbstractionFindingParameter {
    /// Authored parameter name.
    pub(crate) name: Symbol,
    /// Authored parameter span receiving the primary diagnostic.
    pub(crate) span: Span,
}

/// Complete evidence for one parameter that has only one concrete substitution.
pub struct GenericAbstractionFinding {
    /// Generic declaration identity and diagnostic context.
    pub(crate) declaration: GenericAbstractionFindingDeclaration,
    /// Authored parameter identity and primary span.
    pub(crate) parameter: GenericAbstractionFindingParameter,
    /// Sole concrete substitution rendered in guidance.
    pub(crate) concrete_type: String,
    /// Representative active uses proving the sole substitution.
    pub(crate) use_spans: Vec<Span>,
    /// Whether a public declaration was analyzed under an explicitly closed package policy.
    pub(crate) is_closed_package_public: bool,
}

// -----------------------------------------------------------------------------
// GenericAbstractionOpenTypeVisitor: Conservative type classification
// -----------------------------------------------------------------------------

/// Returns whether one type form is inherently inferred or abstract.
const fn generic_abstraction_open_type_form_is_open(ty: &Ty<'_, AmbigArg>) -> bool {
    matches!(
        ty.kind,
        TyKind::OpaqueDef(_)
            | TyKind::TraitAscription(_)
            | TyKind::TraitObject(..)
            | TyKind::Infer(_)
            | TyKind::Err(_)
    )
}

/// Returns whether a resolved path denotes a forwarded or unresolved type.
const fn generic_abstraction_open_type_resolution_is_open(resolution: Res) -> bool {
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

/// Determines whether one authored substitution contains an open or unresolved type boundary.
struct GenericAbstractionOpenTypeVisitor<'a, 'tcx> {
    /// Compiler context used to resolve authored paths.
    cx: &'a LateContext<'tcx>,
    /// Accumulated conservative decision.
    has_open_boundary: bool,
}

impl<'tcx> Visitor<'tcx> for GenericAbstractionOpenTypeVisitor<'_, 'tcx> {
    fn visit_ty(&mut self, ty: &'tcx Ty<'tcx, AmbigArg>) {
        // Stop descending after any nested component has made the substitution open.
        if self.has_open_boundary {
            return;
        }

        // Reject inference and abstraction forms before descending into their children.
        if generic_abstraction_open_type_form_is_open(ty) {
            self.has_open_boundary = true;
            return;
        }

        // Parameters, projections, and aliases do not prove one stable concrete substitution.
        if let TyKind::Path(qpath) = ty.kind {
            let resolution = self.cx.qpath_res(&qpath, ty.hir_id);
            self.has_open_boundary = generic_abstraction_open_type_resolution_is_open(resolution);
            if self.has_open_boundary {
                return;
            }
        }

        intravisit::walk_ty(self, ty);
    }
}

// -----------------------------------------------------------------------------
// GenericAbstractionAnalyzer: Crate wide substitution model
// -----------------------------------------------------------------------------
#[derive(Default)]
/// Collects eligible nominal declarations and every authored active substitution.
pub struct GenericAbstractionAnalyzer {
    /// Eligible declarations indexed by local definition identity.
    declarations: HashMap<LocalDefId, GenericAbstractionDeclaration>,
    /// Parameter evidence indexed by declaration and non-lifetime argument position.
    evidence: HashMap<GenericAbstractionParameterId, GenericSubstitutionEvidence>,
}

impl GenericAbstractionAnalyzer {
    /// Packages one supported declaration shape without repeating field mapping.
    const fn parts<'hir>(
        name: Symbol,
        kind: &'static str,
        generics: &'hir Generics<'hir>,
    ) -> GenericAbstractionParts<'hir> {
        GenericAbstractionParts {
            name,
            kind,
            generics,
        }
    }

    /// Returns the generic declaration shape for one supported nominal item.
    const fn declaration_parts<'hir>(
        item: &'hir Item<'hir>,
    ) -> Option<GenericAbstractionParts<'hir>> {
        // Keep the supported nominal vocabulary explicit and closed.
        match item.kind {
            ItemKind::Struct(ident, generics, _) => {
                Some(Self::parts(ident.name, "struct", generics))
            }
            ItemKind::Enum(ident, generics, _) => Some(Self::parts(ident.name, "enum", generics)),
            ItemKind::Union(ident, generics, _) => Some(Self::parts(ident.name, "union", generics)),
            ItemKind::TyAlias(ident, generics, _) => {
                Some(Self::parts(ident.name, "type alias", generics))
            }
            _ => None,
        }
    }

    /// Maps constructors and variants back to their owning supported type declaration.
    fn declaration_def_id(cx: &LateContext<'_>, mut def_id: DefId) -> Option<LocalDefId> {
        loop {
            // Accept a local supported declaration after walking through value-level children.
            let kind = cx.tcx.def_kind(def_id);
            let is_supported = matches!(
                kind,
                DefKind::Struct | DefKind::Enum | DefKind::Union | DefKind::TyAlias
            );
            if is_supported {
                return def_id.as_local();
            }

            // Only constructors and variants can resolve below their owning nominal type.
            if !matches!(kind, DefKind::Ctor(..) | DefKind::Variant) {
                return None;
            }
            def_id = cx.tcx.parent(def_id);
        }
    }

    /// Classifies one explicit type argument and returns its stable authored spelling.
    fn concrete_argument<'tcx>(
        cx: &LateContext<'tcx>,
        ty: &'tcx Ty<'tcx, AmbigArg>,
    ) -> Option<String> {
        // Reject the complete nested type when any component is an open boundary.
        let mut visitor = GenericAbstractionOpenTypeVisitor {
            cx,
            has_open_boundary: false,
        };
        visitor.visit_ty(ty);
        if visitor.has_open_boundary {
            return None;
        }

        // Preserve authored vocabulary while treating unavailable source conservatively.
        let source_map = cx.sess().source_map();
        let Ok(source) = source_map.span_to_snippet(ty.span) else {
            return None;
        };
        Some(source.split_whitespace().collect::<Vec<_>>().join(" "))
    }

    /// Maps one non-lifetime generic parameter to this slice's diagnostic model.
    fn declaration_parameter(
        parameter: &GenericParam<'_>,
        argument_position: usize,
    ) -> Option<GenericAbstractionParameter> {
        // Reject const and compiler-synthesized type parameters from this first slice.
        let GenericParamKind::Type {
            synthetic: false, ..
        } = parameter.kind
        else {
            return None;
        };

        // Retain the authored identity together with its non-lifetime argument position.
        Some(GenericAbstractionParameter {
            name: parameter.name.ident().name,
            span: parameter.span,
            argument_position,
        })
    }

    /// Selects explicit authored type parameters and their argument positions.
    fn declaration_parameters(generics: &Generics<'_>) -> Vec<GenericAbstractionParameter> {
        // Lifetimes are omitted from type/const positions while consts still advance the index.
        let non_lifetime_parameters = generics
            .params
            .iter()
            .filter(|parameter| !matches!(parameter.kind, GenericParamKind::Lifetime { .. }))
            .enumerate();

        // Convert only authored type parameters while preserving the enumerated positions.
        non_lifetime_parameters
            .filter_map(|(position, parameter)| Self::declaration_parameter(parameter, position))
            .collect()
    }

    /// Builds a finding only from complete single-substitution evidence.
    fn parameter_finding(
        declaration: &GenericAbstractionDeclaration,
        parameter: &GenericAbstractionParameter,
        evidence: Option<&GenericSubstitutionEvidence>,
        visibility_context: GenericSubstitutionVisibilityContext,
    ) -> Option<GenericAbstractionFinding> {
        // Require at least one use, no open boundary, and exactly one concrete spelling.
        let evidence = evidence?;
        if evidence.has_open_boundary || evidence.concrete.len() != 1 {
            return None;
        }
        let (concrete_type, use_spans) = evidence.concrete.iter().next()?;

        // Package the owning declaration context.
        let finding_declaration = GenericAbstractionFindingDeclaration {
            hir_id: declaration.hir_id,
            name: declaration.name,
            kind: declaration.kind,
            is_callable: false,
        };

        // Package the independently diagnosed parameter context.
        let finding_parameter = GenericAbstractionFindingParameter {
            name: parameter.name,
            span: parameter.span,
        };

        // Return one immutable diagnostic record without proposing a mechanical rewrite.
        let is_closed_package_public = visibility_context.is_closed_package_public();

        // Assemble the diagnostic after every evidence and policy decision is complete.
        Some(GenericAbstractionFinding {
            declaration: finding_declaration,
            parameter: finding_parameter,
            concrete_type: concrete_type.clone(),
            use_spans: use_spans.clone(),
            is_closed_package_public,
        })
    }

    /// Records one parameter substitution or open-boundary use.
    fn record_parameter_use<'tcx>(
        evidence: &mut GenericSubstitutionEvidence,
        cx: &LateContext<'tcx>,
        argument: Option<&&GenericArg<'tcx>>,
        use_span: Span,
    ) {
        let Some(GenericArg::Type(argument_ty)) = argument.copied() else {
            evidence.has_open_boundary = true;
            return;
        };
        let Some(concrete) = Self::concrete_argument(cx, argument_ty) else {
            evidence.has_open_boundary = true;
            return;
        };

        // Bound labels while retaining every distinct substitution identity.
        let spans = evidence.concrete.entry(concrete).or_default();
        if spans.len() >= GENERIC_SUBSTITUTION_MAX_REPRESENTATIVE_USE_SPANS {
            return;
        }
        spans.push(use_span);
    }

    /// Returns the path segment carrying arguments for the resolved declaration.
    fn declaration_arguments<'hir>(
        cx: &LateContext<'_>,
        qpath: &QPath<'hir>,
        hir_id: HirId,
        declaration: LocalDefId,
    ) -> Option<&'hir rustc_hir::GenericArgs<'hir>> {
        // Qualified and type-relative paths do not expose a declaration segment reliably.
        let QPath::Resolved(_, path) = qpath else {
            return None;
        };

        // Prefer the segment which resolves directly to the owning declaration.
        let direct = path.segments.iter().find_map(|segment| {
            let segment_def_id = segment.res.opt_def_id()?;
            (Self::declaration_def_id(cx, segment_def_id) == Some(declaration))
                .then_some(segment.args)
                .flatten()
        });
        if direct.is_some() {
            return direct;
        }

        // A resolved expression may attach inferred arguments only to its terminal segment.
        let resolved = cx.qpath_res(qpath, hir_id).opt_def_id()?;
        (Self::declaration_def_id(cx, resolved) == Some(declaration))
            .then(|| path.segments.last()?.args)
            .flatten()
    }

    /// Records one eligible generic nominal declaration.
    pub(crate) fn record_item(&mut self, item: &Item<'_>) {
        // Reject generated and unsupported declarations before inspecting their generics.
        if item.span.from_expansion() {
            return;
        }
        let Some(parts) = Self::declaration_parts(item) else {
            return;
        };

        // Retain only declarations with at least one explicit authored type parameter.
        let parameters = Self::declaration_parameters(parts.generics);
        if parameters.is_empty() {
            return;
        }

        // Preserve declaration identity independently from use evidence and visitation order.
        let declaration = GenericAbstractionDeclaration {
            def_id: item.owner_id.def_id,
            hir_id: item.hir_id(),
            name: parts.name,
            kind: parts.kind,
            parameters,
        };

        // Index the complete candidate after every eligibility decision is complete.
        self.declarations.insert(declaration.def_id, declaration);
    }

    /// Derives source-ordered parameter findings from complete active-crate evidence.
    pub(crate) fn findings(self, cx: &LateContext<'_>) -> Vec<GenericAbstractionFinding> {
        let package = VisibilityPackagePolicy::for_current_package();
        let binary = is_binary_crate(cx);
        let evidence = self.evidence;
        let mut findings = Vec::new();

        // Evaluate every declaration against external visibility and its parameter evidence.
        for declaration in self.declarations.into_values() {
            // Preserve externally visible declarations in packages that may be published.
            let exported = cx
                .tcx
                .effective_visibilities(())
                .is_exported(declaration.def_id);
            if !binary && package.preserves_exported_public_items() && exported {
                continue;
            }

            // Map parameters independently so a genuinely varying sibling stays generic.
            let visibility_context = GenericSubstitutionVisibilityContext::for_declaration(
                cx,
                package,
                declaration.def_id,
            );

            // Join every retained parameter to its complete crate-wide evidence.
            let parameter_findings = declaration.parameters.iter().filter_map(|parameter| {
                let key = GenericAbstractionParameterId {
                    declaration: declaration.def_id,
                    argument_position: parameter.argument_position,
                };
                Self::parameter_finding(
                    &declaration,
                    parameter,
                    evidence.get(&key),
                    visibility_context,
                )
            });
            findings.extend(parameter_findings);
        }

        // Restore stable authored order independently from hash-map iteration.
        findings.sort_by_key(|finding| finding.parameter.span.lo());
        findings
    }

    /// Records every parameter in one resolved path use.
    fn record_qpath<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        qpath: &QPath<'tcx>,
        hir_id: HirId,
        use_span: Span,
    ) {
        // Resolve constructors and variants to the supported declaration owning their generics.
        let Some(resolved) = cx.qpath_res(qpath, hir_id).opt_def_id() else {
            return;
        };
        let Some(declaration) = Self::declaration_def_id(cx, resolved) else {
            return;
        };

        // Missing, partial, or inferred argument lists become absent argument evidence.
        let arguments = Self::declaration_arguments(cx, qpath, hir_id, declaration);
        let non_lifetime_arguments = arguments
            .map(|args| {
                args.args
                    .iter()
                    .filter(|argument| !matches!(argument, GenericArg::Lifetime(_)))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Preserve const positions while selecting the type parameters evaluated by this slice.
        let generics = cx.tcx.generics_of(declaration);
        let generic_parameters = generics
            .own_params
            .iter()
            .filter(|parameter| !matches!(parameter.kind, ty::GenericParamDefKind::Lifetime))
            .enumerate();

        // Record each type parameter independently from declaration visitation order.
        for (argument_position, parameter) in generic_parameters {
            // Const parameters retain their position but have no substitution finding yet.
            if !matches!(parameter.kind, ty::GenericParamDefKind::Type { .. }) {
                continue;
            }

            // Join this parameter position to its declaration-wide evidence bucket.
            let key = GenericAbstractionParameterId {
                declaration,
                argument_position,
            };
            let evidence = self.evidence.entry(key).or_default();
            let argument = non_lifetime_arguments.get(argument_position);
            Self::record_parameter_use(evidence, cx, argument, use_span);
        }
    }

    /// Records an explicit type-position substitution.
    pub(crate) fn record_ty<'tcx>(&mut self, cx: &LateContext<'tcx>, ty: &'tcx Ty<'tcx, AmbigArg>) {
        let TyKind::Path(qpath) = ty.kind else {
            return;
        };
        self.record_qpath(cx, &qpath, ty.hir_id, ty.span);
    }

    /// Records explicit constructor and variant substitutions in expression position.
    pub(crate) fn record_expr<'tcx>(
        &mut self,
        cx: &LateContext<'tcx>,
        expression: &'tcx Expr<'tcx>,
    ) {
        match expression.kind {
            ExprKind::Path(qpath) => {
                self.record_qpath(cx, &qpath, expression.hir_id, expression.span);
            }
            ExprKind::Struct(qpath, ..) => {
                self.record_qpath(cx, qpath, expression.hir_id, expression.span);
            }
            _ => {}
        }
    }
}
