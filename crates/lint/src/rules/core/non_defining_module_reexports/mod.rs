extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_middle;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_errors::DiagDecorator;
use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_middle::ty;
use rustc_span::Span;

use crate::utils::diagnostic::LateViolation;

// -----------------------------------------------------------------------------
// ViolationKind: Reexport syntax classification
// -----------------------------------------------------------------------------
/// Authored syntax that exports an item from somewhere other than its defining module.
#[derive(Clone, Copy)]
enum ViolationKind {
    /// Visibility-bearing `use` declaration for a local or external item.
    Use,
    /// Visibility-bearing `extern crate` declaration for a dependency.
    ExternCrate,
}

impl ViolationKind {
    /// Classifies outwardly visible import syntax while retaining private imports.
    const fn from_item(item: &Item<'_>) -> Option<Self> {
        match item.kind {
            ItemKind::Use(..) => Some(Self::Use),
            ItemKind::ExternCrate(..) => Some(Self::ExternCrate),
            _ => None,
        }
    }

    /// Describes the concrete non-defining export syntax.
    const fn primary_message(self) -> &'static str {
        match self {
            Self::Use => "this use declaration reexports an item from a non-defining module",
            Self::ExternCrate => {
                "this extern crate declaration reexports a dependency from a non-defining module"
            }
        }
    }

    /// Gives ownership-preserving remediation for the syntax category.
    const fn remediation_message(self) -> &'static str {
        match self {
            Self::Use => {
                "import the item through its defining module; expose that module at the required visibility, or define a local wrapper that owns the abstraction"
            }
            Self::ExternCrate => {
                "remove the outward visibility and have consumers depend on and import the external crate directly"
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Violation: Non defining module reexport diagnostic
// -----------------------------------------------------------------------------

/// Resolved reexport with its complete source location and remediation category.
struct Violation {
    /// Complete import declaration highlighted by the diagnostic.
    span: Span,
    /// Syntax category used to produce precise remediation guidance.
    kind: ViolationKind,
}

impl LateViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.kind.primary_message())
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "reexports obscure the item's canonical definition, create competing import paths, and make the exporting module appear to own an API defined elsewhere",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.kind.remediation_message())
    }

    fn emit(self, cx: &LateContext<'_>) {
        cx.emit_span_lint(
            NON_DEFINING_MODULE_REEXPORTS,
            self.span,
            DiagDecorator(|diag| {
                diag.primary_message(self.primary_message().into_owned());
                diag.note(self.rationale_message().into_owned());
                diag.help(self.remediation_message().into_owned());
            }),
        );
    }
}

// -----------------------------------------------------------------------------
// NonDefiningModuleReexports: Canonical item path policy
// -----------------------------------------------------------------------------

/// Late lint pass that rejects authored and generated outward reexports.
struct NonDefiningModuleReexports;

dylint_linting::impl_late_lint! {
    #[doc = include_str!("README.md")]
    pub NON_DEFINING_MODULE_REEXPORTS,
    Warn,
    "rejects items exported from modules that do not define them",
    NonDefiningModuleReexports
}

impl LateLintPass<'_> for NonDefiningModuleReexports {
    /// Checks every expanded import declaration regardless of its local or dependency origin.
    fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
        let Some(kind) = ViolationKind::from_item(item) else {
            return;
        };

        // Compare resolved visibility with the import's own module. Rust normalizes inherited,
        // `pub(self)`, and an explicit `pub(in path)` naming that module to the same restriction.
        let def_id = item.owner_id.def_id;
        let module = cx.tcx.parent_module_from_def_id(def_id).to_def_id();
        let visibility = cx.tcx.visibility(def_id);
        if matches!(visibility, ty::Visibility::Restricted(scope) if scope == module) {
            return;
        }
        Violation {
            span: item.span,
            kind,
        }
        .emit(cx);
    }
}
