extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Item, ItemKind, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;
use rustc_span::symbol::kw;

use crate::utils::diagnostic::EarlyViolation;

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
    fn from_item(item: &Item) -> Option<Self> {
        if !Self::is_outward_visibility(&item.vis.kind) {
            return None;
        }
        match item.kind {
            ItemKind::Use(_) => Some(Self::Use),
            ItemKind::ExternCrate(..) => Some(Self::ExternCrate),
            _ => None,
        }
    }

    /// Returns whether visibility escapes the module containing the import.
    fn is_outward_visibility(visibility: &VisibilityKind) -> bool {
        match visibility {
            VisibilityKind::Public => true,
            VisibilityKind::Restricted { path, .. } => **path != kw::SelfLower,
            VisibilityKind::Inherited => false,
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

impl EarlyViolation for Violation {
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

    fn emit(self, cx: &EarlyContext<'_>) {
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

/// Early lint pass that rejects authored and generated outward reexports.
struct NonDefiningModuleReexports;

dylint_linting::impl_early_lint! {
    /// ### What it does
    ///
    /// Rejects every `use` or `extern crate` declaration whose visibility escapes its containing
    /// module, including declarations produced by macros. This covers `pub`, `pub(crate)`,
    /// `pub(super)`, and `pub(in path)` forms for local items, dependency items, renamed imports,
    /// and glob imports. Ordinary private imports and the explicitly private `pub(self)` form are
    /// not reexports.
    ///
    /// ### Why is this bad?
    ///
    /// A reexport gives one declaration multiple plausible homes. Readers and tools can no longer
    /// infer ownership from an import path, module searches must follow façade layers, and an
    /// external dependency can appear to be an API owned by the exporting crate. A visible defining
    /// module provides one truthful canonical path. When a crate genuinely needs to own a boundary,
    /// a local trait, newtype, or wrapper makes that ownership explicit instead of borrowing a name.
    ///
    /// For example, this façade makes `Parser` appear to belong to the crate root:
    ///
    /// ```rust
    /// mod parser {
    ///     pub struct Parser;
    /// }
    ///
    /// pub use parser::Parser;
    /// ```
    ///
    /// Expose the defining module and use its canonical path:
    ///
    /// ```rust
    /// pub mod parser {
    ///     pub struct Parser;
    /// }
    ///
    /// use parser::Parser;
    /// ```
    pub NON_DEFINING_MODULE_REEXPORTS,
    Warn,
    "rejects items exported from modules that do not define them",
    NonDefiningModuleReexports
}

impl EarlyLintPass for NonDefiningModuleReexports {
    /// Checks every expanded import declaration regardless of its local or dependency origin.
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        let Some(kind) = ViolationKind::from_item(item) else {
            return;
        };
        Violation {
            span: item.span,
            kind,
        }
        .emit(cx);
    }
}
