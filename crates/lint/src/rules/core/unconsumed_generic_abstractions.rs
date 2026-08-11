extern crate rustc_errors;
extern crate rustc_hir;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{AmbigArg, Expr, ImplItem, Item, Ty};
use rustc_lint::{LateContext, LateLintPass};

use crate::utils::callable_generic_analysis::CallableGenericAnalyzer;
use crate::utils::diagnostic::LateViolation;
use crate::utils::generic_abstraction_analysis::{
    GenericAbstractionAnalyzer, GenericAbstractionFinding,
};

// -----------------------------------------------------------------------------
// Violation: Unconsumed generic abstraction diagnostic
// -----------------------------------------------------------------------------

/// One generic parameter with only one observed concrete substitution.
struct Violation(
    /// Complete declaration and use evidence.
    GenericAbstractionFinding,
);

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Owned(format!(
            "type parameter `{}` on {} `{}` has only one concrete substitution",
            self.0.parameter.name, self.0.declaration.kind, self.0.declaration.name
        ))
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        if self.0.declaration.is_callable {
            return Cow::Owned(format!(
                "every active authored call resolves `{}` to `{}`; no generic forwarding, callable escape, unnameable type, or second concrete substitution demonstrates that this parameter is consumed polymorphically",
                self.0.parameter.name, self.0.concrete_type
            ));
        }
        Cow::Owned(format!(
            "every active authored use supplies `{}`; no generic forwarding, inferred argument, projection, opaque type, trait object, alias, or second concrete substitution demonstrates that this parameter is consumed polymorphically",
            self.0.concrete_type
        ))
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        if self.0.declaration.is_callable {
            return Cow::Owned(format!(
                "specialize `{}` as `{}` in the signature and remove the generic parameter until another substitution is required",
                self.0.parameter.name, self.0.concrete_type
            ));
        }
        Cow::Owned(format!(
            "replace `{}` with `{}` until a second real substitution is required",
            self.0.parameter.name, self.0.concrete_type
        ))
    }

    fn emit(self, cx: &LateContext<'_>) {
        // Render stable diagnostic layers before moving authored use evidence.
        let primary = self.primary_message().into_owned();
        let rationale = self.rationale_message().into_owned();
        let remediation = self.remediation_message().into_owned();

        // Keep representative substitutions adjacent to the parameter-level recommendation.
        cx.tcx.emit_node_span_lint(
            UNCONSUMED_GENERIC_ABSTRACTIONS,
            self.0.declaration.hir_id,
            self.0.parameter.span,
            DiagDecorator(|diag| {
                diag.primary_message(primary);
                for use_span in self.0.use_spans {
                    diag.span_label(
                        use_span,
                        if self.0.declaration.is_callable {
                            format!("`{}` resolves here", self.0.concrete_type)
                        } else {
                            format!("`{}` is supplied here", self.0.concrete_type)
                        },
                    );
                }
                diag.note(rationale);
                if self.0.is_closed_package_public {
                    diag.note(
                        "this public declaration is analyzed because the package declares `publish = false`",
                    );
                }
                diag.help(remediation);
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// UnconsumedGenericAbstractions: Nominal substitution policy
// -----------------------------------------------------------------------------

/// Late pass collecting generic nominal declarations and active authored substitutions.
#[derive(Default)]
struct UnconsumedGenericAbstractions {
    /// Crate-wide nominal substitution analyzer.
    analyzer: GenericAbstractionAnalyzer,
    /// Crate-wide resolved callable substitution analyzer.
    callable_analyzer: CallableGenericAnalyzer,
}

dylint_linting::impl_late_lint! {
    /// ### What it does
    ///
    /// Finds authored type parameters on local structs, enums, unions, type aliases, free functions,
    /// and inherent methods when every active use supplies the same fully concrete type. Parameters
    /// are analyzed independently, so one unused dimension can be specialized without rejecting
    /// meaningful variation in another. Direct calls are resolved through compiler type checking,
    /// including arguments inferred without an explicit turbofish.
    ///
    /// Evidence remains open when a parameter is inferred on a nominal use, forwarded from another
    /// generic context, escapes as a callable value, or involves an alias, projection, opaque type,
    /// trait object, closure, function item, function pointer, unnameable type, dependent bound, or
    /// higher-ranked bound. Trait methods, trait implementations, impl-level type parameters,
    /// generated declarations, unsafe functions, and non-Rust ABIs are excluded. Unrestricted public
    /// APIs in publishable libraries are preserved; binaries and packages marked `publish = false`
    /// are treated as closed. Active test uses count, while disabled `cfg` branches are not compiled.
    ///
    /// ### Why is this bad?
    ///
    /// A generic parameter with one concrete substitution introduces abstract vocabulary,
    /// monomorphized surface, and propagation through constructors and impls without supporting an
    /// observed variant. Specializing the declaration keeps the design concrete until another
    /// substitution is genuinely required.
    ///
    /// ```rust
    /// struct Input<T> {
    ///     value: T,
    /// }
    ///
    /// fn length<T: AsRef<str>>(input: T) -> usize {
    ///     input.as_ref().len()
    /// }
    ///
    /// fn inspect(value: String) -> usize {
    ///     let input = Input { value };
    ///     length(input.value)
    /// }
    /// ```
    ///
    /// When both parameters are only ever `String`, express the current design directly:
    ///
    /// ```rust
    /// struct Input {
    ///     value: String,
    /// }
    ///
    /// fn length(input: String) -> usize {
    ///     input.len()
    /// }
    ///
    /// fn inspect(value: String) -> usize {
    ///     let input = Input { value };
    ///     length(input.value)
    /// }
    /// ```
    ///
    /// The lint is deliberately compilation-local and structural: parameter names and comments
    /// about future variants do not suppress it, while an observable second substitution or open
    /// generic boundary does. It offers no automatic rewrite because specialization can propagate
    /// through fields, constructors, impl blocks, bounds, imports, and inferred call sites.
    pub UNCONSUMED_GENERIC_ABSTRACTIONS,
    Warn,
    "questions generic parameters with only one concrete substitution",
    UnconsumedGenericAbstractions::default()
}

impl<'tcx> LateLintPass<'tcx> for UnconsumedGenericAbstractions {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        self.analyzer.record_item(item);
        self.callable_analyzer.record_item(cx, item);
    }

    fn check_impl_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx ImplItem<'tcx>) {
        self.callable_analyzer.record_impl_item(cx, item);
    }

    fn check_ty(&mut self, cx: &LateContext<'tcx>, ty: &'tcx Ty<'tcx, AmbigArg>) {
        self.analyzer.record_ty(cx, ty);
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, expression: &'tcx Expr<'tcx>) {
        self.analyzer.record_expr(cx, expression);
        self.callable_analyzer.record_expression(cx, expression);
    }

    fn check_crate_post(&mut self, cx: &LateContext<'tcx>) {
        let mut findings = std::mem::take(&mut self.analyzer).findings(cx);
        findings.extend(std::mem::take(&mut self.callable_analyzer).findings(cx));
        findings.sort_by_key(|finding| finding.parameter.span.lo());
        for finding in findings {
            Violation(finding).emit(cx);
        }
    }
}
