extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_span;

use std::borrow::Cow;

use rustc_ast::ast::{Inline, Item, ItemKind, ModKind, VisibilityKind};
use rustc_errors::DiagDecorator;
use rustc_lint::{EarlyContext, EarlyLintPass, LintContext};
use rustc_span::Span;
use rustc_span::symbol::kw;

use crate::utils::diagnostic::EarlyViolation;

// -----------------------------------------------------------------------------
// Violation: Invalid barrel item classification
// -----------------------------------------------------------------------------

/// The four useful classifications for code that does not belong in a barrel file.
#[derive(Clone, Copy)]
enum ViolationKind {
    /// An inline module hides implementation code inside a barrel file.
    InlineModule,
    /// A private import does not expose anything from this barrel.
    PrivateImport,
    /// Macro definitions and calls do not belong in a barrel file.
    Macro,
    /// Implementation code does not belong in a barrel file.
    Implementation,
}

impl ViolationKind {
    /// Classifies a top-level item, returning `None` for valid barrel entries.
    fn from_item(item: &Item) -> Option<Self> {
        match &item.kind {
            ItemKind::Mod(_, _, kind) => Self::from_module(kind),
            ItemKind::Use(_) if Self::is_outward_reexport(&item.vis.kind) => None,
            ItemKind::Use(_) => Some(Self::PrivateImport),
            ItemKind::MacCall(_) | ItemKind::MacroDef(..) | ItemKind::DelegationMac(_) => {
                Some(Self::Macro)
            }
            _ => Some(Self::Implementation),
        }
    }

    /// Classifies file and inline module declarations within a barrel file.
    const fn from_module(kind: &ModKind) -> Option<Self> {
        match kind {
            ModKind::Unloaded | ModKind::Loaded(_, Inline::No { .. }, _) => None,
            ModKind::Loaded(_, Inline::Yes, _) => Some(Self::InlineModule),
        }
    }

    /// Returns whether a `use` makes a name visible outside the current module.
    ///
    /// `pub(crate)`, `pub(super)`, and `pub(in ancestor)` all expose a useful barrel entry.
    /// `pub(self)` and `pub(in self)` are merely private imports written with longer syntax.
    fn is_outward_reexport(visibility: &VisibilityKind) -> bool {
        match visibility {
            VisibilityKind::Public => true,
            VisibilityKind::Restricted { path, .. } => **path != kw::SelfLower,
            VisibilityKind::Inherited => false,
        }
    }

    /// Explains why the particular item conflicts with the role of a barrel file.
    const fn message(self) -> &'static str {
        match self {
            Self::InlineModule => "an inline module hides implementation code inside a barrel file",
            Self::PrivateImport => "a private import does not expose anything from this barrel",
            Self::Macro => "macro definitions and calls do not belong in a barrel file",
            Self::Implementation => "implementation code does not belong in a barrel file",
        }
    }

    /// Describes the intended source organization in plain language.
    const fn help(self) -> &'static str {
        match self {
            Self::InlineModule => {
                "move the module body to child.rs or child/mod.rs and leave only `mod child;` here"
            }
            Self::PrivateImport => {
                "remove this import or expose it as a reexport that callers can actually use"
            }
            Self::Macro => {
                "move the macro elsewhere and spell this barrel's declarations and reexports explicitly"
            }
            Self::Implementation => {
                "move this code into a dedicated module and leave its declaration or reexport here"
            }
        }
    }
}

/// Forbidden barrel-file item with its authored source location.
struct Violation {
    /// Complete top-level item span used as the diagnostic location.
    span: Span,
    /// Concrete reason this item violates the barrel boundary.
    kind: ViolationKind,
}

impl EarlyViolation for Violation {
    fn primary_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.kind.message())
    }

    fn rationale_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(
            "a barrel file should describe module structure only, so readers and tools can locate implementation in named child modules",
        )
    }

    fn remediation_message(&self) -> Cow<'_, str> {
        Cow::Borrowed(self.kind.help())
    }

    fn emit(self, cx: &EarlyContext<'_>) {
        cx.emit_span_lint(
            INVALID_BARREL_FILE_ITEMS,
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
// InvalidBarrelFileItems: Lint state
// -----------------------------------------------------------------------------

/// Enforces the barrel-file boundary while remembering when traversal is inside an inline module.
///
/// The depth prevents one inline module from producing another warning for every item in its body.
/// The inline module itself is the single actionable mistake: moving its body fixes all children.
#[derive(Default)]
struct InvalidBarrelFileItems {
    /// Number of nested inline modules whose children should not be reported separately.
    ignored_inline_depth: usize,
}

dylint_linting::impl_pre_expansion_lint! {
    /// ### What it does
    ///
    /// Keeps every physical `mod.rs` and `lib.rs` file focused on describing the module tree. Such
    /// a file may declare child modules and expose their names, but it may not contain
    /// implementation code.
    ///
    /// ### Why is this bad?
    ///
    /// Predictable barrel files let a reader understand a crate's shape without separating useful
    /// declarations from unrelated behavior. They also give an agent one unambiguous place to look
    /// for each concern: barrels describe where code lives, and named files contain that code.
    ///
    /// For example, this barrel mixes module structure with implementation:
    ///
    /// ```rust
    /// // mod.rs or lib.rs
    /// mod parser;
    /// pub use parser::Parser;
    ///
    /// fn parse() {}
    /// ```
    ///
    /// Move the behavior into a named child file and leave the barrel as a map of the module:
    ///
    /// ```rust,ignore
    /// // mod.rs or lib.rs
    /// mod parser;
    /// pub use parser::{Parser, parse};
    ///
    /// // parser.rs
    /// pub struct Parser;
    /// pub fn parse() {}
    /// ```
    pub INVALID_BARREL_FILE_ITEMS,
    Warn,
    "keeps mod.rs and lib.rs limited to module declarations and reexports",
    InvalidBarrelFileItems::default()
}

impl EarlyLintPass for InvalidBarrelFileItems {
    /// Checks one explicitly written top-level item before macros can replace it with generated code.
    ///
    /// Running before expansion matters for code such as `include!("items.rs")`: the macro call is
    /// itself forbidden even when the included file happens to contain only module declarations.
    fn check_item(&mut self, cx: &EarlyContext<'_>, item: &Item) {
        // Ignore all items inside an inline module because the module itself is the actionable problem.
        if self.ignored_inline_depth > 0 {
            self.enter_inline_module(item);
            return;
        }

        // Ignore items that are not physically written in a barrel file because they cannot be moved.
        if !Self::is_written_in_barrel_file(cx, item) {
            return;
        }

        // Classifies the item and returns a violation if it is not a valid barrel entry.
        let Some(kind) = ViolationKind::from_item(item) else {
            return;
        };

        // Emit a single warning for the item and enter any inline module to avoid duplicate warnings.
        Violation {
            span: item.span,
            kind,
        }
        .emit(cx);
        self.enter_inline_module(item);
    }

    /// Leaves an ignored inline-module body after all of its children have been visited.
    fn check_item_post(&mut self, _: &EarlyContext<'_>, item: &Item) {
        if self.ignored_inline_depth == 0 || !Self::is_inline_module(item) {
            return;
        }
        self.ignored_inline_depth -= 1;
    }
}

impl InvalidBarrelFileItems {
    /// Returns whether an item is an inline `mod child { ... }` definition.
    const fn is_inline_module(item: &Item) -> bool {
        matches!(
            item.kind,
            ItemKind::Mod(_, _, ModKind::Loaded(_, Inline::Yes, _))
        )
    }

    /// Returns whether the item came from a real file named `mod.rs` or `lib.rs`.
    ///
    /// Compiler-created and virtual filenames are ignored because they do not represent a barrel
    /// file that a developer can open and reorganize.
    fn is_written_in_barrel_file(cx: &EarlyContext<'_>, item: &Item) -> bool {
        let source_map = cx.sess().source_map();
        let filename = source_map.span_to_filename(item.span);
        let local_path = filename.into_local_path();
        let basename = local_path.and_then(|path| path.file_name().map(ToOwned::to_owned));
        basename.is_some_and(|name| name == "mod.rs" || name == "lib.rs")
    }

    /// Records entry into an inline module so its contents do not receive duplicate warnings.
    const fn enter_inline_module(&mut self, item: &Item) {
        if !Self::is_inline_module(item) {
            return;
        }
        self.ignored_inline_depth += 1;
    }
}
