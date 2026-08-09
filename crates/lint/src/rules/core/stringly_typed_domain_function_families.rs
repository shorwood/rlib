extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_span;

use rustc_errors::DiagDecorator;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{Body, FieldDef, FnDecl, Item};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_span::Span;
use rustc_span::def_id::LocalDefId;

use crate::utils::parameter_analysis::ParameterSignature;
use crate::utils::string_domain_analysis::DomainAnalyzer;

// -----------------------------------------------------------------------------
// StringlyTypedDomainFunctionFamilies
// -----------------------------------------------------------------------------

/// Collects free functions and raw textual fields into module-local domain families.
#[derive(Default)]
struct StringlyTypedDomainFunctionFamilies {
    /// Shared module-local domain analysis.
    analyzer: DomainAnalyzer,
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds groups of free functions collectively acting like an impl block for a missing
    /// string-backed domain type. A family requires either two related behaviors or one behavior
    /// reinforced by a raw textual field with the same normalized concept name.
    ///
    /// ### Why is this bad?
    ///
    /// A domain represented only by repeated names has no owner for validity, normalization,
    /// comparison, formatting, or derived behavior. Callers can pass arbitrary text, related
    /// functions remain scattered in the module namespace, and generated code tends to add more
    /// helpers instead of recognizing the missing concept.
    ///
    /// ```rust
    /// fn validate_slug(slug: &str) -> Result<(), SlugError> {}
    /// fn normalize_slug(slug: &str) -> String {}
    /// fn slug_path(slug: &str) -> PathBuf {}
    /// ```
    ///
    /// Introduce one domain value that owns the invariant and behavior:
    ///
    /// ```rust
    /// struct Slug(String);
    ///
    /// impl Slug {
    ///     fn path(&self) -> PathBuf {}
    /// }
    ///
    /// impl FromStr for Slug {
    ///     type Err = SlugError;
    ///     fn from_str(source: &str) -> Result<Self, Self::Err> {}
    /// }
    /// ```
    pub STRINGLY_TYPED_DOMAIN_FUNCTION_FAMILIES,
    Warn,
    "detects free-function families that imply a missing string-backed domain type",
    StringlyTypedDomainFunctionFamilies::default()
}

impl<'tcx> LateLintPass<'tcx> for StringlyTypedDomainFunctionFamilies {
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        kind: FnKind<'tcx>,
        _: &'tcx FnDecl<'tcx>,
        body: &'tcx Body<'tcx>,
        _: Span,
        def_id: LocalDefId,
    ) {
        let Some(signature) = ParameterSignature::from_body(cx, kind, body, def_id) else {
            return;
        };
        self.analyzer.record_function(cx, &signature, body);
    }

    fn check_field_def(&mut self, cx: &LateContext<'tcx>, field: &'tcx FieldDef<'tcx>) {
        self.analyzer.record_field(cx, field);
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.analyzer.record_item(cx, item);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        for finding in self.analyzer.family_findings() {
            cx.emit_span_lint(
                STRINGLY_TYPED_DOMAIN_FUNCTION_FAMILIES,
                finding.span,
                DiagDecorator(|diag| {
                    diag.primary_message(format!(
                        "these declarations collectively define a stringly typed `{}` domain",
                        finding.domain
                    ));
                    for label in finding.labels {
                        diag.span_label(label.span, label.message);
                    }
                    if let Some(existing_type) = finding.existing_type {
                        diag.help(format!(
                            "accept the existing `{existing_type}` type and move the remaining domain behavior onto it"
                        ));
                    } else {
                        diag.help(format!(
                            "introduce a string-backed `{}` type and move its invariant and behavior onto that type",
                            finding.domain
                        ));
                    }
                }),
            );
        }
    }
}
